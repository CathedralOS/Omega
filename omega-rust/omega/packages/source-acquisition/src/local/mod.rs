/*
Local-source capture, immutable snapshot publication, historical recovery, and
verification.

We resolve local package directories in two modes: in-memory invocation capture
(`capture_local_source_input`, which retains the exact bytes that produced the
source identity without writing a disk snapshot) and retained-lane snapshot
publication (`resolve_local_source_snapshot_in_lane`, which stages, verifies,
and atomically publishes an immutable read-only copy under an origin-bound
`source-v2-<digest>` custody address).
*/

/*
Why local snapshots bind both canonical origin and content digest:

Two distinct local checkouts may contain byte-identical trees at one moment and
diverge later. Addressing each published snapshot by `(canonical_live_root,
SourceContentDigest)` prevents cross-origin cache sharing while still allowing
`recover_cached_local_source_in_lane` to recover verified historical bytes for
a specific origin even after the live checkout changes or is deleted.
*/

/*
Provenance and candidate staging:

Successful publications issue a `LocalSourceResolutionObservation` binding the
caller's requested spelling, canonical live root, publication root, source
limits, custody identity, and exact-tree rehash. When package update workflows
propose a single-file edit (`stage_local_source_replacement_in_lane`), we
replace that file in the captured inventory, rehash the exact tree, and publish
a candidate snapshot without mutating the live project checkout.
*/

pub(crate) mod observation;
pub mod operations;
mod recovery;
pub mod resolution_observations;
pub(crate) mod snapshot;
pub mod staging;

#[cfg(test)]
mod tests;
