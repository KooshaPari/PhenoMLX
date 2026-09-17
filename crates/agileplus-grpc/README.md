# agileplus-grpc

Tonic gRPC adapter layer for AgilePlus core and work-item services.

## Public API Index

- Server exports: `start_server`, `AgilePlusCoreServer`, `domain_error_to_status`.
- Modules: `conversions`, `event_bus`, `proxy`, `server`, `streaming`, `work_items`.
- Proto types come from `agileplus-proto`.

## Validation

```bash
cargo test -p agileplus-grpc
```

