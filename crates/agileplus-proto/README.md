# agileplus-proto

Compiled or stubbed protobuf/tonic types for the gRPC layer.

## Public API Index

- `agileplus`: generated module when `protoc` is available, or compatible stubs when it is not.
- Consumers should import proto-facing types from this crate instead of including generated files directly.

## Validation

```bash
cargo test -p agileplus-proto
```

