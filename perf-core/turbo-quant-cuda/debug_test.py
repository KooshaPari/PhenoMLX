"""Debug TurboQuant port - check each step."""
import torch
import sys

# Replicate encode.rs exactly with small example
def encode_uniform_cpu(data, bits, group_size):
    """Pure Python port of Rust encode.rs for verification."""
    qmax = (1 << bits) - 1
    packed = []
    scales = []
    zeros = []
    bit_cursor = 0
    for chunk_start in range(0, len(data), group_size):
        chunk = data[chunk_start:chunk_start + group_size]
        mn = min(chunk)
        mx = max(chunk)
        scale = (mx - mn) / qmax
        zero = mn
        scales.append(max(scale, 1e-12))
        zeros.append(zero)
        for v in chunk:
            q = round((v - zero) / scale)
            q = max(0, min(qmax, q))
            # Write q in LSB-first
            bp = bits
            remaining = bp
            while remaining > 0:
                byte_idx = bit_cursor // 8
                bit_off = bit_cursor % 8
                room = 8 - bit_off
                take = min(remaining, room)
                mask = (1 << take) - 1
                while len(packed) <= byte_idx:
                    packed.append(0)
                packed[byte_idx] |= ((q & mask) << bit_off)
                q >>= take
                bit_cursor += take
                remaining -= take
    return bytes(packed), scales, zeros


def decode_uniform_cpu(packed, scales, zeros, n, bits, group_size):
    """Pure Python port of Rust decode.rs."""
    qmax = (1 << bits) - 1
    out = []
    for i in range(n):
        g = i // group_size
        s = scales[g]
        z = zeros[g]
        bp = bits
        bc = i * bp
        raw = 0
        shift = 0
        remaining = bp
        while remaining > 0:
            byte_idx = bc // 8
            bit_off = bc % 8
            room = 8 - bit_off
            take = min(remaining, room)
            mask = (1 << take) - 1
            byte = packed[byte_idx] if byte_idx < len(packed) else 0
            raw |= ((byte >> bit_off) & mask) << shift
            shift += take
            bc += take
            remaining -= take
        q = min(raw, qmax)
        out.append(q * s + z)
    return out


# Test on small data
test_data = [0.0, 0.5, 1.0, -0.5, 2.0, -2.0, 0.25, -0.25]
bits = 4
group_size = 4

packed, scales, zeros = encode_uniform_cpu(test_data, bits, group_size)
decoded = decode_uniform_cpu(packed, scales, zeros, len(test_data), bits, group_size)

print(f"Input:  {test_data}")
print(f"Packed: {packed.hex()}")
print(f"Scales: {scales}")
print(f"Zeros:  {zeros}")
print(f"Decoded: {[round(x, 4) for x in decoded]}")
print(f"Errors:  {[round(abs(a-b), 4) for a,b in zip(test_data, decoded)]}")

# Now compare against PyTorch port
data = torch.tensor(test_data, dtype=torch.float32, device="cuda")
print(f"\nPyTorch test:")
sys.path.insert(0, r'C:\bench')
from turbo_quant_cuda import encode_uniform_cuda, decode_uniform_cuda
packed_cuda, scales_cuda, zeros_cuda = encode_uniform_cuda(data, bits=bits, group_size=group_size)
packed_bytes = bytes(packed_cuda.cpu().numpy())
print(f"Packed CUDA: {packed_bytes.hex()}")
print(f"Match: {packed_bytes == packed}")

decoded_cuda = decode_uniform_cuda(packed_cuda, scales_cuda, zeros_cuda, len(test_data), bits=bits, group_size=group_size)
print(f"Decoded CUDA: {[round(x.item(), 4) for x in decoded_cuda]}")
print(f"Errors CUDA:  {[round(abs(a-b.item()), 4) for a,b in zip(data, decoded_cuda)]}")
