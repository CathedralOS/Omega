/*
Retained build-output custody, canonical tree identity, and immutable source
snapshots.

We capture what a sponsored build wrote into its private output directory,
commit to a canonical tree identity (`BuildStagedOutputTreeCommitment`),
select completed or generated-source subsets, and materialize retained trees
onto an empty host directory on request. `captured_source` applies the same
retained-custody discipline to immutable source trees before a build runs, bound
to the canonical source metadata index that narrows the build's filesystem
reads.

Why keep `BuildStagedOutputTree` fields private and recompute commitments on
reconstruction? Package-controlled data or decoded persistence rows must never
impersonate compiler capture or substitute a stored commitment without
reproducing its exact digest, entry count, and unique file-byte count.
*/

/*
Module layout:
- `staged_output_tree`: the canonical tree carrier, domain-separated commitment,
  generated-source selection (`select_included_sources`), and completed-output
  subset selection (`select_files`).
- `capture`: physical host-tree acquisition with open-file identity and
  link-count custody checks.
- `materialization`: deterministic host-directory materialization and retained
  tree validation.
- `portable_paths`: portable path component, device-name, and relative symlink
  rules shared by capture and materialization.
- `captured_source`: immutable `CapturedBuildSourceInput` inventory verified
  against `CanonicalFilesystemMetadataIndex`.
- `tree_from_entries`: test-only constructor for canonical output trees without
  touching the host filesystem.
*/

#![forbid(unsafe_code)]
#![cfg_attr(windows, feature(windows_by_handle))]

pub(crate) use optimization_core::digest_encoder;
mod capture;
mod captured_source;
mod materialization;
mod portable_paths;
mod staged_output_tree;
#[cfg(test)]
mod tree_from_entries;

pub use capture::capture;
pub use captured_source::{CapturedBuildSourceInput, CapturedSourceEntry, CapturedSourceEntryKind};
pub(crate) use staged_output_tree::BuildStagedOutputMaterializationError;
pub use staged_output_tree::{
    BuildStagedOutputEntryInput, BuildStagedOutputEntryKind, BuildStagedOutputTree,
    BuildStagedOutputTreeCommitment, PackageGeneratedSource, empty, select_included_sources,
};
