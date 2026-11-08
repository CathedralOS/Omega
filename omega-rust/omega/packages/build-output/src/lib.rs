#![forbid(unsafe_code)]
#![cfg_attr(windows, feature(windows_by_handle))]

//! Retained build-output custody, canonical identity, and materialization.
//!
//! `BuildStagedOutputTree` captures what a build wrote, commits to a canonical
//! identity, materializes the tree on request, and verifies it against the
//! commitment; `captured_source` does the same for captured source inputs and
//! `tree_from_entries` constructs a canonical tree from explicit entries.
//!
//! Start at `staged_output_tree.rs`, the root: the tree, its commitment and its
//! sealed entries. `capture` builds one from the host filesystem,
//! `materialization` writes and verifies one, `portable_paths` holds
//! the path rules both sides share, and `captured_source` owns source snapshots.

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
