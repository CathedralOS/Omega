# Omega Package Source

This crate turns hostile local or Git input into an immutable, content-verified
source tree. It owns source identity, bounded tree capture, snapshot
publication, cache custody, and direct resolved-source custody.

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
├── git/            Git transport adapter, object verification, and resolution
│   ├── request.rs      validate transport, locator, revision, and endpoint
│   ├── cache/          create, verify, repair, and invalidate retained stores
│   ├── executable/     freeze operator-selected Git before package input
│   ├── commands/       construct and run bounded Git commands
│   ├── objects/        verify commit/tree/blob object graphs and identities
│   │   └── batch/         bounded transfer, exact protocol, and request custody
│   ├── resolution/     acquire, verify, materialize, and issue custody
│   ├── snapshot.rs     Git-specific verified tree materialization
│   └── workspace/      syntax-neutral workspace declaration exchange
├── observations/   direct resolved-source and retained-storage custody
├── limits.rs       compiler-owned acquisition ceilings
└── error.rs        fail-closed acquisition errors
```

Native process lifecycle, concrete resource limits, and bounded duplex capture
live in [`omega-bounded-process`](../../../tooling/omega-bounded-process/README.md).
The peer [`execution/`](../execution/README.md) crate owns resolver-specific
executable and phase/root preparation. Acquisition retains no executable identity,
command/completion provenance, platform-guarantee row, or fetch receipt.
Successful Git resolution exposes the authored canonical lineage,
requested revision, selected objects and materialized content, immutable
snapshot custody, and the concrete limits and retained-storage measurements
that were actually checked.
Package declarations, graph reconciliation, review, and admission remain
manager responsibilities. `SourceRelativePath` is lexical source navigation,
not an authored workspace-member declaration.

The crate root retains ordinary whole-storage entry points. Callers that
already hold one retained lane use the responsibility paths directly:
`git::resolution` for Git acquisition, `local::operations` and `local::model`
for exact snapshot work, and `storage::RetainedStorageLane` for lane custody.
These are deliberate public seams; cache machinery, native process assembly,
object verification, and publication internals remain private.

Whole-root and named-member acquisition both accept an operation-local
`GitAcquisitionPin` through their `from_pin_in_lane(s)` entrances. Reuse checks
the exact original request, retained cache, commit and root tree without
refetching; an absent or invalid pinned cache fails. These privately issued
pins prevent drift within one traversal, not persisted lock recovery.

`git::resolution::resolve_git_source_at_revision_in_lane` instead accepts
inert recorded `GitCommitId` and `GitTreeId` values for whole-root acquisition.
Its explicit `Offline` mode performs no transport or revision discovery;
`AllowFetch` reuses cached exact objects first and permits one exact-commit
fetch only after a successful, bounded object-absence probe. Neither mode
selects a mutable ref or `FETCH_HEAD`. Cache metadata and returned custody
retain the original authored request, while the expected IDs determine the
selected content. Cold creation uses their object format without `ls-remote`.

Absence of the recorded commit or root tree is distinct from malformed output,
failed commands, or invalid custody. An offline absence preserves a healthy
cache. Present objects still pass normal commit, tree, materialization, and
snapshot authentication before issuance; recorded IDs cannot mint custody.
Incomplete or corrupted descendant objects fail without automatic repair.
This whole-root entrance does not select workspace members or reconstruct a
manager lock's package graph.

The companion `resolve_git_workspace_member_at_revision_in_lanes` uses that
same recorded commit/root tree with the existing bounded workspace planner.
It authenticates root and declared-member declarations at that revision, then
materializes only the selected member. Returned custody distinguishes the
recorded repository root tree from the selected member tree and retains the
declarations that justified navigation. Both storage lanes remain checked;
unrelated repository payloads are not copied into the member snapshot. The
manager still owns interpreting declarations and matching the lock's package
identity and complete source graph.

Dependency direction is deliberate: the `local` and `git` adapters may use
`identity`, `tree`, `snapshot`, `custody`, and `storage`; those shared owners
must not depend back on either adapter. The local and Git adapters must not
import one another. This keeps transport details out of exact-tree identity and
makes the source lifecycle discoverable from the top down.

The current enforced floor and remaining platform gaps are maintained in
[`SOURCE_RESOLVER_SECURITY.md`](SOURCE_RESOLVER_SECURITY.md).
