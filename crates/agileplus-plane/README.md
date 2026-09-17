# agileplus-plane

Plane.so bidirectional sync adapter with webhook ingestion and outbound push support.

## Public API Index

- Plane port re-exports: `PlaneIssue`, `PlaneProject`, `PlaneSyncPort`.
- Client/sync: `PlaneClient`, `PlaneSyncAdapter`, `SyncState`.
- Mapping/conflict APIs: `PlaneStateMapper`, `PlaneStateMapperConfig`, `compute_content_hash`, `detect_conflict`, `ConflictStatus`.
- Queue/webhook/inbound/outbound/label APIs are re-exported from their modules.

## Validation

```bash
cargo test -p agileplus-plane
```

