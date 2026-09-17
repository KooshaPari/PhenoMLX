# agileplus-events

Event sourcing primitives: domain events, envelopes, hashing, snapshots, query, replay, and stores.

## Public API Index

- Domain event exports from `domain_event`.
- Hash chain: `compute_hash`, `verify_chain`, `HashError`.
- Query/replay/snapshot APIs: `EventQuery`, `replay_events`, `Aggregate`, snapshot types.
- Store API: `EventStore`, `InMemoryEventStore`, `EventError`.

## Validation

```bash
cargo test -p agileplus-events
```

