#![forbid(unsafe_code)]

//! Retained build-output custody, canonical identity, and materialization.
//!
//! `BuildStagedOutputTree` captures what a build wrote, commits to a canonical
//! identity, materializes the tree on request, and verifies it against the
//! commitment; `captured_source` does the same for captured source inputs and
//! `replayed_tree` rebuilds a tree from retained entries without rerunning.
//!
//! Start at `staged_output_tree.rs`, the root: the tree, its commitment and its
//! sealed entries. `capture` builds one from the host filesystem,
//! `materialization` writes and verifies one, `portable_paths` holds
//! the path rules both sides share, and `captured_source`,
//! `replayed_directories` and `replayed_tree` cover captured sources
//! and replayed trees.

mod capture;
mod captured_source;
mod materialization;
mod portable_paths;
mod replayed_directories;
mod replayed_tree;
mod staged_output_tree;
#[cfg(test)]
mod tests;

pub use capture::capture;
pub use captured_source::{
    CapturedBuildSourceInput, CapturedSourceEntry, CapturedSourceEntryKind, CapturedSourceEntryRef,
    CapturedSourceFile, CapturedSourceMaterializationError, discard_materialized_snapshot,
};
pub use replayed_directories::replayed_empty_directories;
pub use replayed_tree::{ReplayedBuildOutputEntry, replayed_output_tree};
pub use staged_output_tree::{
    BuildStagedOutputEntry, BuildStagedOutputEntryKind, BuildStagedOutputMaterializationError,
    BuildStagedOutputTree, BuildStagedOutputTreeCommitment, PackageGeneratedSource, empty,
    replayed_files, replayed_ordinary_files, replayed_single_ordinary_file,
    select_included_sources,
};
