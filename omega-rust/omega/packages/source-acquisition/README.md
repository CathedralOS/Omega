# Omega Package Source

This crate turns untrusted local or Git input into a read-only,
content-verified source tree. It owns source identity, bounded tree capture,
snapshot publication, the cache and resolved-source records.

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
├── source_resolver.rs session entrance: retain storage and select host Git once
├── storage.rs      retained root and lane directory custody
├── local/          local-source adapter and issued local observations
│   ├── recovery.rs     verified historical bytes from the existing snapshot cache
│   ├── staging.rs      proposed file edits with original live-source identity
│   └── snapshot/       shared captured-tree materialization
├── git/            Git transport adapter, object verification, and resolution
│   ├── request.rs      validate transport, locator, revision, and endpoint
│   ├── cache/          create, verify, repair, and invalidate retained stores
│   ├── executable/     freeze operator-selected Git before package input
│   ├── git_command.rs  run bounded Git commands; owns policy and capture
│   ├── objects/        verify commit/tree/blob object graphs and identities
│   │   └── batch/         bounded transfer, exact protocol, and request custody
│   ├── resolution/     acquire, verify, materialize, and issue custody
│   ├── snapshot.rs     Git-specific verified tree materialization
│   └── workspace/      syntax-neutral workspace declaration exchange
├── observations/   direct resolved-source and retained-storage custody
├── limits.rs       compiler-owned acquisition ceilings
└── error.rs        fail-closed acquisition errors
```

Native process handling, concrete resource limits and bounded duplex capture
live in [`bounded-process`](../../foundation/bounded-process/README.md).
`git/git_command.rs` launches every Git process: ordinary calls, blob batches
and exact-object probes. Callers supply arguments, stdin and stdout limits;
it applies command policy, counts the launch, limits capture and
checks the shared resolution budget. Batch callers still interpret the
protocol and own the cache. A failed Git exit never counts as object absence.
The sibling [`execution/`](../resolver-execution/README.md) crate prepares
the resolver's executable, phases and roots. Acquisition keeps no executable
identity, command/completion history, platform-guarantee row or fetch
receipt. Successful Git resolution returns the authored canonical lineage,
requested revision, selected objects and written content, read-only snapshot,
and the concrete limits and storage measurements checked. The
manager owns declarations, graph reconciliation, review and
admission. `SourceRelativePath` is a lexical source path, not an authored
workspace-member declaration.

New local snapshots are stored at an address built from their origin and
the strong content digest package locks record. `local/snapshot/address.rs`
owns that address and its lock name. Recovery looks it up directly and
verifies metadata, exact tree, origin and stored records. Only a missing
entry allows the bounded legacy search by raw tree key; a malformed entry is
an error even if a legacy copy exists. Recovery returns old source, never a
fresh live observation. Source content identity and snapshot payload format
are unchanged; the new addresses and live-observation records have their own
versions.

`source_resolver::SourceResolver` starts a session. Its constructors take
the private root, find host Git outside package-controlled directories, then
create and verify the acquisition lanes (storage directories). The session
holds at most one Git choice, passed explicitly to Git acquisition; local
resolution works without host Git. Root, lane and child-lane records hold
only filesystem state and never choose or keep an executable.

The crate root exposes the ordinary whole-session functions. Callers that
already hold a lane use the modules directly: `git::resolution` for Git
acquisition, `local::operations` and `local::model` for exact snapshot work,
and `storage::RetainedStorageLane` for the lane itself. These modules are
public on purpose; the cache, process setup, object verification and
publication internals stay private.

`local::operations::recover_cached_local_source_in_lane` finds old bytes for
a canonical local origin and accepted content digest that the caller knows.
It tries the origin/content address first and runs the bounded legacy
metadata search only if that is missing, then verifies the snapshot under its
entry lock. It never opens the origin: the result is old source, not proof
the live source still matches. A missing snapshot returns no source and an
invalid matching tree is rejected; neither creates an archive, index or
replacement baseline. The manager owns diagnostic fallback and policy
comparison when old source is unavailable.

`local::staging` replaces one existing regular file in a proposed snapshot
after checking the file's expected raw-byte SHA-256. It leaves the live tree
untouched and keeps the original requested/canonical root and pre-edit
source identity. The shared publisher checks proposed bytes separately from
unchanged live source. Landing those exact bytes gives the same source
identity and can reuse the staged snapshot. This is not a live resolution:
callers still check concurrent source/project edits and own file transaction
recovery. Excluded paths, symlink targets, stale input and proposals over the
source byte limit are rejected.

Capturing an editable local package skips `.git`, root `build/` and root
`omega.lock` (in any ASCII letter case). These are project control files, not
source identity inputs, so creating or updating them must not invalidate
local pins. Same-named files deeper in the tree are ordinary source. A
symlink into a skipped file is rejected, so no captured link depends on a
file missing from its snapshot. Exact written trees, Git snapshots included,
keep and hash all lock files; verification does not apply these skips to
published source.

Whole-root and named-member acquisition both accept a one-operation
`GitAcquisitionPin` through their `from_pin_in_lane(s)` functions. Reuse
checks the exact original request, cache, commit and root tree without
refetching; a missing or invalid pinned cache fails. These private pins stop
drift within one traversal; they are not persisted lock recovery.

`git::resolution::GitWorkspaceSession` separates reading a verified tree
from publishing a package. `open` reads the root declaration; `open_members`
batch-reads the requested member declarations; the resulting catalog's
`materialize` publishes one chosen subtree. Repository metadata has its own
limit, declarations keep theirs, and unrelated content does not count
against the chosen package's entry or byte allowance. Between steps the
session stays on one exact commit/tree, even if another operation advances
the same selector cache. Each resumed read re-checks the cache and verifies
the requested content. This supplies bytes; it neither runs Build nor admits.

The application's Build path still captures the whole root before named
selection. Using this staged resolver there needs a compiler-owned handoff
from declaration discovery to Product source that keeps one Build run, its
input list and negative observations, and the exact chosen tree. Do not pass
off declaration-only bytes as a complete capture, or rerun Build after
widening it. Cathedral's allocator acquisition waits on that work; this API
alone does not unblock it.

A failed transfer restores the canonical repository configuration and
keeps the cache only if post-command checks pass. Earlier exact objects stay
usable offline. A cache missing `source.identity` entries is discarded and
rebuilt only by a new acquisition that may fetch, under the existing entry
lock and cache limits. Offline use and one-operation pins never rebuild
silently; recorded revisions never fall back to selector refresh. See `git/cache/retry.rs`.

`git::resolution::resolve_git_source_at_revision_in_lane` instead takes
recorded `GitCommitId` and `GitTreeId` values for whole-root acquisition.
Its `Offline` mode does no network transfer or revision lookup;
`AllowFetch` reuses cached exact objects first and fetches one exact commit
only after a bounded probe confirms it is absent. Neither mode uses a
mutable ref or `FETCH_HEAD`. Cache metadata and the returned record keep the
original authored request; the expected IDs decide the content. A new cache
uses their object format without `ls-remote`.

A missing recorded commit or root tree is a different error from bad
output, a failed command or an invalid record. Offline absence leaves a
healthy cache intact. Present objects still pass normal commit, tree, write
and snapshot checks before being returned; recorded IDs alone cannot create
a valid record. Incomplete or corrupted child objects fail without automatic
repair. This function neither chooses workspace members nor rebuilds a
manager lock's package graph.

The companion `resolve_git_workspace_member_at_revision_in_lanes` uses the
same recorded commit/root tree with the existing bounded workspace planner.
It verifies root and declared-member declarations at that revision, then
writes only the chosen member. The returned record separates the repository
root tree from the member tree and keeps the declarations that led there.
Both storage lanes are checked; unrelated repository content stays out of
the member snapshot. The manager still interprets declarations and matches
the lock's package identity and complete source graph.

Workspace-member snapshots are stored under their repository acquisition,
then tree ID, so identical member bytes from unrelated repositories get
separate directories. This prefix is not part of package or content
identity; it stops deduplication from merging sources of different package
keys.

Dependencies point one way on purpose: the `local` and `git` adapters may
use `identity`, `tree`, `snapshot`, `custody` and `storage`, which must not
depend on either adapter, and the adapters must not import each other. This
keeps transport details out of exact-tree identity and lets a reader follow
the source lifecycle top down.

The [resolver security contract](../../../../wiki/spec/packages/resolver_security.md)
defines the required trust and transport behavior; the modules above implement it.
