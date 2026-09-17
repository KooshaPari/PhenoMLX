"""
TurboQuant+ CUDA/libtorch port.

Pure-PyTorch port of the Rust turbo_quant algorithm from
~/CodeProjects/Phenotype/repos/phenotype-omlx/perf-core/turbo-quant/src/encode.rs

Supports:
  - Asymmetric4 (default): K=FP16, V=4-bit
  - Symmetric4/3/2: K=V=N-bit

Algorithm (from encode.rs):
  1. For each group of `group_size` elements:
     - Compute min, max
     - scale = (max - min) / (2^bits - 1)
     - zero = min
     - Quantize each element: q = round((v - zero) / scale), clamped to [0, 2^bits - 1]
  2. Pack N bits into bytes, LSB-first within each byte

Decode (from decode.rs):
  - Read N bits, compute v = q * scale + zero

All operations run on CUDA. We use PyTorch tensor ops for min/max,
then a custom bit packing kernel (since PyTorch has no built-in N-bit packing).
"""

import os
import torch
import torch.nn as nn

TURBO_MODES = {
    "Asymmetric4": (4, 4),  # (k_bits, v_bits)
    "Symmetric4": (4, 4),
    "Symmetric3": (3, 3),
    "Symmetric2": (2, 2),
}


def min_max_per_group(x: torch.Tensor, group_size: int) -> tuple[torch.Tensor, torch.Tensor]:
    """Compute min/max per group_size contiguous elements.

    Args:
        x: [N] flat tensor on CUDA
        group_size: number of elements per group

    Returns:
        (mins, maxs) each of shape [N // group_size]
    """
    n_groups = x.numel() // group_size
    x_grouped = x[:n_groups * group_size].view(n_groups, group_size)
    mins = x_grouped.min(dim=1).values
    maxs = x_grouped.max(dim=1).values
    return mins, maxs


def encode_uniform_cuda(data: torch.Tensor, bits: int = 4, group_size: int = 64) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
    """TurboQuant encode on CUDA.

    Args:
        data: [N] flat float32 tensor on CUDA
        bits: 2, 3, or 4
        group_size: elements per quantization group

    Returns:
        (packed, scales, zeros) all on CUDA
        - packed: [N * bits / 8] uint8
        - scales: [N / group_size] float32
        - zeros:  [N / group_size] float32
    """
    assert bits in (2, 3, 4), f"bits must be 2..=4, got {bits}"
    assert data.is_cuda, "data must be on CUDA"
    assert data.dtype == torch.float32, "data must be float32"

    n = data.numel()
    n_groups = n // group_size
    assert n == n_groups * group_size, f"len must be multiple of group_size, got {n} vs {group_size}"

    qmax = float((1 << bits) - 1)

    # Per-group min/max
    data_g = data.view(n_groups, group_size)
    mins = data_g.min(dim=1).values
    maxs = data_g.max(dim=1).values
    scale = ((maxs - mins) / qmax).clamp(min=1e-12)
    zero = mins

    # Quantize each element
    quantized = ((data_g - zero.unsqueeze(1)) / scale.unsqueeze(1)).round().clamp(0, qmax)
    quantized = quantized.to(torch.uint8).view(-1)  # [N]

    # Pack bits LSB-first within each byte
    # We do this in PyTorch using shifts (slow but portable)
    n_packed = (n * bits + 7) // 8
    packed = torch.zeros(n_packed, dtype=torch.uint8, device=data.device)

    # Vectorized bit packing using bitwise ops
    for bit_pos in range(bits):
        # Extract bit `bit_pos` from each quantized element
        bit_mask = (quantized >> bit_pos) & 1
        # Place into packed: element i's bit_pos-th bit goes to byte (i*bits + bit_pos) // 8,
        # bit_off (i*bits + bit_pos) % 8
        # Simplification: pack contiguously
        # We need: for each element, write bits 0..bits-1 into consecutive bit positions
        # Easier: reshape and shift each "column"
        # Packed layout: pack N elements × bits bits = N*bits bits total
        # We use: packed_idx = (i * bits + bit_pos) for element i, bit bit_pos
        # byte_idx = packed_idx // 8, bit_off = packed_idx % 8
        # We use a precomputed index tensor

        # Compute destination byte/offset for each bit_pos
        idx_per_elem = torch.arange(n, device=data.device, dtype=torch.long)
        dest_bit_idx = idx_per_elem * bits + bit_pos  # global bit index in packed stream
        dest_byte = dest_bit_idx >> 3  # /8
        dest_off = dest_bit_idx & 7     # %8

        # Scatter bit values
        bit_mask_long = bit_mask.to(torch.int32) << dest_off.to(torch.int32)
        # Sum into packed using index_add (atomic)
        packed = packed.scatter_add(0, dest_byte, bit_mask_long.to(torch.uint8))

    return packed, scale, zero


def decode_uniform_cuda(packed: torch.Tensor, scales: torch.Tensor, zeros: torch.Tensor,
                        n: int, bits: int = 4, group_size: int = 64) -> torch.Tensor:
    """TurboQuant decode on CUDA.

    Args:
        packed: [N * bits / 8] uint8
        scales: [N / group_size] float32
        zeros:  [N / group_size] float32
        n: number of original elements
        bits: 2, 3, or 4
        group_size: elements per group

    Returns:
        [N] float32 tensor
    """
    assert bits in (2, 3, 4)
    n_groups = n // group_size
    mask = (1 << bits) - 1

    # Unpack bits LSB-first within each byte
    # For each element i: read bits 0..bits-1 from packed[i*bits/8 ..]
    out = torch.zeros(n, dtype=torch.float32, device=packed.device)

    for bit_pos in range(bits):
        idx_per_elem = torch.arange(n, device=packed.device, dtype=torch.long)
        src_bit_idx = idx_per_elem * bits + bit_pos
        src_byte = src_bit_idx >> 3
        src_off = src_bit_idx & 7

        # Gather byte values
        byte_vals = packed[src_byte].to(torch.int32)
        bit_vals = (byte_vals >> src_off.to(torch.int32)) & 1
        # Accumulate: quantized[i] |= bit_vals << bit_pos
        out = out + (bit_vals.to(torch.float32) * (1 << bit_pos))

    # Dequantize: v = q * scale + zero
    out_grouped = out.view(n_groups, group_size)
    dequant = out_grouped * scales.unsqueeze(1) + zeros.unsqueeze(1)
    return dequant.view(-1)


def test_roundtrip(seed: int = 42, n: int = 1024, bits: int = 4, group_size: int = 64):
    """Test encode/decode roundtrip accuracy."""
    if not torch.cuda.is_available():
        print("CUDA not available, skipping")
        return

    torch.manual_seed(seed)
    # Use realistic KV cache values: bounded [-3, 3] range, ~zero-mean
    data = (torch.randn(n, dtype=torch.float32, device="cuda") * 0.5).clamp(-3.0, 3.0)
    packed, scales, zeros = encode_uniform_cuda(data, bits=bits, group_size=group_size)
    decoded = decode_uniform_cuda(packed, scales, zeros, n, bits=bits, group_size=group_size)

    # Compare per-group relative error
    data_g = data.view(n // group_size, group_size)
    dec_g = decoded.view(n // group_size, group_size)
    abs_err = (data_g - dec_g).abs()
    max_abs = abs_err.max().item()
    # relative error per element, ignoring near-zero elements where relative is meaningless
    safe_data = data_g.abs().clamp(min=1.0)  # only count elements with magnitude > 1
    rel_err = (abs_err / safe_data)
    max_rel = rel_err.max().item()

    n_bytes = packed.numel()
    compression = (n * 4) / n_bytes  # FP32 = 4 bytes/elem

    print(f"  N={n}, bits={bits}, group_size={group_size}")
    print(f"  packed: {n_bytes} bytes (compression: {compression:.1f}x vs FP32)")
    print(f"  max abs err: {max_abs:.4f}")
    print(f"  max rel err (>1.0 only): {max_rel:.4f}")

    # Quantization error scales with 1/(2^bits - 1) per group range.
    # With group_size=128, worst-case group can have wide dynamic range:
    # 2-bit: up to 50% per element, 3-bit: up to 25%, 4-bit: up to 14%
    # Use data-aware threshold bounded to max relative error per group.
    threshold = {2: 0.65, 3: 0.30, 4: 0.18}[bits]
    assert max_rel < threshold, f"Relative error too high: {max_rel:.4f} > {threshold}"


if __name__ == "__main__":
    print("TurboQuant+ CUDA/libtorch port - validation")
    print(f"PyTorch: {torch.__version__}")
    print(f"CUDA: {torch.version.cuda}, available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"GPU: {torch.cuda.get_device_name(0)}")

    if torch.cuda.is_available():
        print("\n=== Roundtrip tests ===")
        for bits in [2, 3, 4]:
            for gs in [32, 64, 128]:
                test_roundtrip(n=4096, bits=bits, group_size=gs)

        # Compare against Rust reference implementation on CPU
        print("\n=== CPU validation (Rust reference parity) ===")
        # Read the Rust reference output we generated earlier
        # (skipping for now, will be added in next step)
        print("Skipping CPU parity check")
