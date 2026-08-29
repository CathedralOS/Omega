//! Restart-stable review baselines and bounded rooted-file custody.

use std::fmt;

mod capsule;
mod encoding;
mod operations;
mod storage;
mod validation;

pub use capsule::{ReviewOnlyBaselineCapsule, ReviewOnlyBaselinePackage};
pub use operations::{
    assemble_update_source_review_from_baseline, compare_review_only_capabilities_from_baseline,
    triage_review_update_from_baseline,
};
pub use storage::{
    ReviewOnlyBaselineDirectory, ReviewOnlyBaselineFileError, ReviewOnlyBaselineName,
    ReviewOnlyBaselineNameError,
};

pub(super) const MAGIC: &[u8] = b"OMEGA-PACKAGE-REVIEW-BASELINE\0";
pub(super) const CHECKSUM_DOMAIN: &[u8] = b"OMEGA-PACKAGE-REVIEW-BASELINE-CAPSULE\0";
pub(super) const REPLAY_PARENT_BINDING_DOMAIN: &[u8] =
    b"OMEGA-PACKAGE-REVIEW-REPLAY-PARENT-BINDING\0";
pub(super) const VERSION: u16 = 3;
pub(super) const REVIEW_ONLY_ARTIFACT_CLASS: u8 = 0;
pub(super) const CHECKSUM_BYTES: usize = 32;
pub(super) const BASELINE_NAME_MAXIMUM_BYTES: usize = 255;

/// Resource ceilings for a restart-stable review baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewOnlyBaselineLimits {
    pub(super) maximum_capsule_bytes: usize,
    pub(super) maximum_packages: usize,
    pub(super) maximum_dependencies: usize,
    pub(super) maximum_graph_depth: usize,
    pub(super) maximum_identity_bytes: usize,
    pub(super) maximum_target_bytes: usize,
    pub(super) maximum_rows: usize,
    pub(super) maximum_row_recovery_bytes: usize,
}

impl ReviewOnlyBaselineLimits {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        maximum_capsule_bytes: usize,
        maximum_packages: usize,
        maximum_dependencies: usize,
        maximum_graph_depth: usize,
        maximum_identity_bytes: usize,
        maximum_target_bytes: usize,
        maximum_rows: usize,
        maximum_row_recovery_bytes: usize,
    ) -> Self {
        Self {
            maximum_capsule_bytes,
            maximum_packages,
            maximum_dependencies,
            maximum_graph_depth,
            maximum_identity_bytes,
            maximum_target_bytes,
            maximum_rows,
            maximum_row_recovery_bytes,
        }
    }

    pub const fn maximum_capsule_bytes(self) -> usize {
        self.maximum_capsule_bytes
    }

    pub const fn maximum_packages(self) -> usize {
        self.maximum_packages
    }

    pub const fn maximum_rows(self) -> usize {
        self.maximum_rows
    }
}

impl Default for ReviewOnlyBaselineLimits {
    fn default() -> Self {
        Self::new(
            64 * 1024 * 1024,
            1_024,
            16_384,
            128,
            4 * 1024,
            256,
            65_536,
            32 * 1024 * 1024,
        )
    }
}

/// A bounded baseline-codec failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyBaselineError {
    message: &'static str,
}

impl ReviewOnlyBaselineError {
    pub(super) const fn new(message: &'static str) -> Self {
        Self { message }
    }

    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for ReviewOnlyBaselineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message)
    }
}

impl std::error::Error for ReviewOnlyBaselineError {}

#[cfg(test)]
#[path = "tests.rs"]
mod replay_record_tests;
