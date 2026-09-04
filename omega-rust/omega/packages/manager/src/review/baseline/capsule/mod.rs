//! In-memory baseline packages and restart-stable capsule operations.

mod capture;
mod persistence;
mod recovery;

use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedPackageClosure;
use crate::review::candidate::{
    PackageReviewEvidence, ReviewOnlyCanonicalRow, ReviewOnlySourceConsumptionCommitment,
};
use omega_build_evaluation::ReviewOnlyBuildFilesystemReplayRecord;
use omega_package_source::ImmutableSourceResolution;

/// One package's exact comparison evidence recovered from a review-only
/// baseline capsule.
#[derive(Debug, Clone)]
pub struct ReviewOnlyBaselinePackage {
    pub(super) key: PackageKey,
    pub(super) resolution: ImmutableSourceResolution,
    pub(super) target: String,
    pub(super) source_consumption_commitment: ReviewOnlySourceConsumptionCommitment,
    pub(super) build_observation_commitment: Option<[u8; 32]>,
    pub(super) filesystem_replay_record: Option<ReviewOnlyBuildFilesystemReplayRecord>,
    pub(super) replay_record_parent_binding: Option<[u8; 32]>,
    pub(super) whole_review_commitment: [u8; 32],
    pub(super) canonical_rows: Vec<ReviewOnlyCanonicalRow>,
}

impl ReviewOnlyBaselinePackage {
    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub const fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    pub const fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_commitment
    }

    pub const fn filesystem_replay_record(&self) -> Option<&ReviewOnlyBuildFilesystemReplayRecord> {
        self.filesystem_replay_record.as_ref()
    }

    pub const fn whole_review_commitment(&self) -> [u8; 32] {
        self.whole_review_commitment
    }

    pub fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        &self.canonical_rows
    }
}

impl PackageReviewEvidence for ReviewOnlyBaselinePackage {
    fn key(&self) -> &PackageKey {
        &self.key
    }

    fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    fn projection_identity_matches(&self) -> bool {
        true
    }

    fn target_name(&self) -> &str {
        &self.target
    }

    fn source_consumption_commitment(&self) -> ReviewOnlySourceConsumptionCommitment {
        self.source_consumption_commitment
    }

    fn build_observation_commitment(&self) -> Option<[u8; 32]> {
        self.build_observation_commitment
    }

    fn whole_review_commitment(&self) -> [u8; 32] {
        self.whole_review_commitment
    }

    fn canonical_rows(&self) -> &[ReviewOnlyCanonicalRow] {
        &self.canonical_rows
    }
}

/// A restart-stable source graph and normalized review baseline.
///
/// This is intentionally review-only. It cannot construct `PackageInstance`,
/// authorize a conflict, mutate a project, or stand in for `omega.lock`.
#[derive(Debug, Clone)]
pub struct ReviewOnlyBaselineCapsule {
    pub(super) graph: ResolvedPackageClosure,
    pub(super) packages: Vec<ReviewOnlyBaselinePackage>,
}

impl ReviewOnlyBaselineCapsule {
    pub fn graph(&self) -> &ResolvedPackageClosure {
        &self.graph
    }

    pub fn packages(&self) -> &[ReviewOnlyBaselinePackage] {
        &self.packages
    }
}
