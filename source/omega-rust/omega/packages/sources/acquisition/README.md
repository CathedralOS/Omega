# Omega Package Source

This crate turns hostile local or Git input into an immutable, authenticated
source tree. It owns source identity, bounded tree capture, snapshot
publication, cache custody, and successful source receipts.

Start at `src/lib.rs`, then follow the source lifecycle:

```text
src/
├── lib.rs          stable resolution entrance and public results
├── identity/       lineages, locators, immutable revisions, and digests
├── tree/           source-neutral tree model and bounded no-follow capture
│   ├── capture/       traversal policy and captured-entry observations
│   ├── filesystem.rs capability-relative filesystem primitives
│   └── identity.rs   canonical exact-tree identity
├── snapshot/       source-neutral construction, verification, and publication
├── custody/        locks, retained-tree validation, and cache publication
├── storage.rs      retained private storage and explicit acquisition lanes
├── local/          local-source adapter and issued local observations
├── git/            Git transport adapter, authentication, and resolution
│   ├── request.rs      validate transport, locator, revision, and endpoint
│   ├── cache/          create, verify, repair, and invalidate retained stores
│   ├── executable/     select and retain exact helper executables
│   ├── commands/       construct and reconcile bounded Git commands
│   ├── objects/        authenticate commit/tree/blob object graphs
│   │   └── batch/         bounded transfer, exact protocol, and request custody
│   ├── resolution/     acquire, authenticate, materialize, and issue custody
│   ├── snapshot.rs     Git-specific authenticated tree materialization
│   └── workspace/      syntax-neutral workspace declaration exchange
├── observations/   execution, accounting, retained-storage, and receipts
├── limits.rs       compiler-owned acquisition ceilings
└── error.rs        fail-closed acquisition errors
```

Native child-process confinement lives in the peer
[`execution/`](../execution/README.md) crate.
Package declarations, graph reconciliation, review, and admission remain
manager responsibilities. `SourceRelativePath` is lexical source navigation,
not an authored workspace-member declaration.

The crate root retains ordinary whole-storage entry points. Callers that
already hold one retained lane use the responsibility paths directly:
`git::resolution` for Git acquisition, `local::operations` and `local::model`
for exact snapshot work, and `storage::RetainedStorageLane` for lane custody.
These are deliberate public seams; cache machinery, native process assembly,
object authentication, and publication internals remain private.

Dependency direction is deliberate: the `local` and `git` adapters may use
`identity`, `tree`, `snapshot`, `custody`, and `storage`; those shared owners
must not depend back on either adapter. The local and Git adapters must not
import one another. This keeps transport details out of exact-tree identity and
makes the source lifecycle discoverable from the top down.

The current enforced floor and remaining platform gaps are maintained in
[`SOURCE_RESOLVER_SECURITY.md`](SOURCE_RESOLVER_SECURITY.md).
