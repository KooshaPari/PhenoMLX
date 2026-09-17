# agileplus-nats

NATS infrastructure adapter and in-process event bus for tests.

## Public API Index

- Bus API: `EventBus`, `EventBusStore`, `EventBusError`, `InMemoryBus`.
- NATS API: `NatsEventBus`, `NatsEventBusError`, `derive_subject`.
- Supporting types: `NatsConfig`, `Envelope`, `FnHandler`, `Handler`, `BusHealth`, `Subject`.

## Validation

```bash
cargo test -p agileplus-nats
```

