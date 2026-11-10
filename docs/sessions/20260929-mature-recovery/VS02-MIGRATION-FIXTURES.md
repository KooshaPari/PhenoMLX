# VS-02 migration/compatibility fixtures

F1 profile schema migration preserves historical profile/support references.
F2 capability source declaration remains source_declaration after roundtrip.
F3 verified qualification remains distinct from source declaration.
F4 tokenizer mismatch invalidates prior profile qualification reuse.
F5 chat-template mismatch explicit.
F6 unsupported operation returns structured unsupported.
F7 requested engine fallback produces explicit new RealizedProfile.
F8 engine version unknown remains unknown.
F9 model-scoped capability cannot be generalized to all models.
F10 NOT_APPLICABLE remains distinct from unsupported/unknown.

Schema/read-only only; no cross-engine qualification claim.
