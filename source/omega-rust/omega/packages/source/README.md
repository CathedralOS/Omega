# Omega Package Source

This crate owns immutable source identity, local snapshot capture, Git
acquisition, cache custody, and successful non-admitting resolution
observations.

Start at `src/lib.rs`, then follow the source lifecycle:

```text
src/
├── lib.rs          stable whole-storage source-resolution entrance
├── identity/       lineages, locators, validated paths, and immutable revisions
├── local/          local snapshot responsibility path
│   ├── operations.rs  resolve, capture, and verify immutable snapshots
│   └── model.rs       resolved snapshots and verified captured entries
├── git/            validated Git request through immutable publication
│   ├── request.rs      validate transport, locator, revision, and endpoint
│   ├── cache/          create, verify, repair, and invalidate retained stores
│   ├── executable/     select and retain exact helper executables
│   ├── commands/       construct and reconcile bounded Git commands
│   ├── objects/        authenticate commit/tree/blob object graphs
│   ├── resolution/     acquire, authenticate, materialize, and issue custody
│   ├── snapshot/       build and atomically publish immutable source trees
│   └── workspace/      syntax-neutral workspace declaration exchange
├── custody/        locks, tree validation, and atomic publication
├── observations/   execution, accounting, retained-storage, and receipt evidence
├── storage.rs      retained private storage and explicit acquisition lanes
├── limits.rs       compiler-owned acquisition ceilings
└── error.rs        fail-closed acquisition errors
```

Native child-process confinement lives one level down in
[`resolver-execution/`](resolver-execution/README.md).
Package declarations, graph reconciliation, review, and admission remain
manager responsibilities. `SourceRelativePath` is lexical source navigation,
not an authored workspace-member declaration.

The crate root retains ordinary whole-storage entry points. Callers that
already hold one retained lane use the responsibility paths directly:
`git::resolution` for Git acquisition, `local::operations` and `local::model`
for exact snapshot work, and `storage::RetainedStorageLane` for lane custody.
These are deliberate public seams; cache machinery, native process assembly,
object authentication, and publication internals remain private.

The current enforced floor and remaining platform gaps are maintained in
[`SOURCE_RESOLVER_SECURITY.md`](SOURCE_RESOLVER_SECURITY.md).
