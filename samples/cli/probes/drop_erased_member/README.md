# drop_erased_member

`use omega::language::core::drop` + `drop(carrier)` on a record that owns NO `Carrier::drop` hook: the checked cleanup edge selects member hooks for live storage (`Guard::drop` on `guard`), while `proof [erased]: Evidence` carries no runtime representation — it needs no hook of its own and triggers no cleanup.

## Learnings

- `[erased]` fields are proof payload only: semantically present, representation-free, exempt from cleanup selection.
- Owner hooks are opt-in per type; containing a hooked member does not force the record to declare one.

Compile-checked only: run a check target, not native.
