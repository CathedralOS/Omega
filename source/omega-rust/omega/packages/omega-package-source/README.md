# Omega Package Source

This crate owns immutable source identity, local snapshot capture, Git
acquisition, cache custody, and successful non-admitting resolution
observations.

Start at `src/lib.rs`, then follow the source lifecycle:

```text
src/
├── lib.rs          public identity and acquisition entrance
├── identity/       names, lineages, locators, and immutable revisions
├── local/          capture and publish local immutable snapshots
├── git/            fetch, authenticate, materialize, and retain Git trees
├── custody/        locks, tree validation, and atomic publication
├── observations/   bounded successful-resolution observations
├── storage.rs      retained private storage and acquisition lanes
├── limits.rs       compiler-owned acquisition ceilings
└── error.rs        fail-closed acquisition errors
```

Native child-process confinement lives in `../omega-resolver-execution/`.
Package declarations, graph reconciliation, review, and admission remain
manager responsibilities.

The current enforced floor and remaining platform gaps are maintained in
[`SOURCE_RESOLVER_SECURITY.md`](SOURCE_RESOLVER_SECURITY.md).
