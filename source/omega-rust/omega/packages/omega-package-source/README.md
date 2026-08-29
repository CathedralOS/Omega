# Omega Package Source

This crate owns immutable source identity, local snapshot capture, Git
acquisition, cache custody, and successful non-admitting resolution
observations.

Start at `src/lib.rs`, then follow the source lifecycle:

```text
src/
├── lib.rs          public source identity and acquisition entrance
├── identity/       lineages, locators, validated paths, and immutable revisions
├── local/          capture and publish local immutable snapshots
├── git/            validated Git request through immutable publication
│   ├── request.rs      validate transport, locator, revision, and endpoint
│   ├── cache/          create, verify, repair, and invalidate retained stores
│   ├── executable/     select and retain exact helper executables
│   ├── process/        construct and reconcile bounded Git commands
│   ├── objects/        authenticate commit/tree/blob object graphs
│   ├── resolution/     acquire, authenticate, materialize, and issue custody
│   ├── snapshot/       build and atomically publish immutable source trees
│   └── workspace/      syntax-neutral workspace declaration exchange
├── custody/        locks, tree validation, and atomic publication
├── observations/   bounded successful-resolution observations
├── storage.rs      retained private storage and acquisition lanes
├── limits.rs       compiler-owned acquisition ceilings
└── error.rs        fail-closed acquisition errors
```

Native child-process confinement lives in `../omega-resolver-execution/`.
Package declarations, graph reconciliation, review, and admission remain
manager responsibilities. `SourceRelativePath` is lexical source navigation,
not an authored workspace-member declaration.

The current enforced floor and remaining platform gaps are maintained in
[`SOURCE_RESOLVER_SECURITY.md`](SOURCE_RESOLVER_SECURITY.md).
