//! Bounded conflict and error vocabulary.

use super::format::{RenderByteCounter, render_conflict_set};
use super::render_error::ReviewOnlyCapabilityConflictRenderError;
use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::resolution::graph::DependencyRequestPath;
use crate::review::candidate::ReviewOnlySourceConsumptionCommitment;
use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
};
use omega_package_source::ImmutableSourceResolution;
use std::fmt;

pub use super::error::ReviewOnlyCapabilityConflictError;
pub use super::limits::ReviewOnlyCapabilityConflictLimits;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewOnlyCapabilityConflictChange {
    Added,
    Removed,
    Changed,
}

/// Exact compatibility contract broken by changing the selected project root's
/// authored role without changing its package identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReviewOnlyRootRoleContract {
    DependencyCompatibility,
    ApplicationActivation,
}

impl ReviewOnlyRootRoleContract {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DependencyCompatibility => "dependency-compatibility",
            Self::ApplicationActivation => "application-activation",
        }
    }
}

/// Directional review result for one stable root key whose authored role
/// changed. This is blocking review evidence, not an admission decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyRootRoleChange {
    pub(super) root: PackageKey,
    pub(super) baseline_role: BuildDeclarationKind,
    pub(super) candidate_role: BuildDeclarationKind,
    pub(super) broken_contract: ReviewOnlyRootRoleContract,
}

impl ReviewOnlyRootRoleChange {
    pub fn root(&self) -> &PackageKey {
        &self.root
    }

    pub const fn baseline_role(&self) -> BuildDeclarationKind {
        self.baseline_role
    }

    pub const fn candidate_role(&self) -> BuildDeclarationKind {
        self.candidate_role
    }

    pub const fn broken_contract(&self) -> ReviewOnlyRootRoleContract {
        self.broken_contract
    }

    pub const fn is_blocking(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOnlyRootRoleComparisonError {
    RootIdentityMismatch {
        baseline: Box<PackageKey>,
        candidate: Box<PackageKey>,
    },
}

impl fmt::Display for ReviewOnlyRootRoleComparisonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootIdentityMismatch { .. } => formatter.write_str(
                "root-role comparison requires one stable package key across baseline and candidate",
            ),
        }
    }
}

impl std::error::Error for ReviewOnlyRootRoleComparisonError {}

/// Domain-separated identity for one exact review-time row conflict.
///
/// This is suitable for joining a future root-policy decision to the conflict
/// it resolved. It is not itself a resolution, admission artifact, or proof of
/// review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyCapabilityConflictFingerprint(pub(super) [u8; 32]);

impl ReviewOnlyCapabilityConflictFingerprint {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

/// Review-only identity of exact candidate source topology and every package's
/// target, compiler, source-consumption, build-observation, and whole-review
/// evidence. It does not admit the candidate or certify any observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyCandidateClosureCommitment(pub(super) [u8; 32]);

impl ReviewOnlyCandidateClosureCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

/// One exact compiler-owned row difference.
///
/// The package layer deliberately does not parse either canonical row. The
/// compiler remains the sole owner of their semantic schema and matching key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyCapabilityConflict {
    pub(super) kind: PackageReviewCanonicalRowKind,
    pub(super) risk: PackageReviewCanonicalRowRisk,
    pub(super) change: ReviewOnlyCapabilityConflictChange,
    pub(super) row_key: Vec<u8>,
    pub(super) baseline_row: Option<Vec<u8>>,
    pub(super) candidate_row: Option<Vec<u8>>,
    pub(super) baseline_source: Option<PackageReviewCanonicalRowSource>,
    pub(super) candidate_source: Option<PackageReviewCanonicalRowSource>,
    pub(super) fingerprint: ReviewOnlyCapabilityConflictFingerprint,
}

impl ReviewOnlyCapabilityConflict {
    pub const fn kind(&self) -> PackageReviewCanonicalRowKind {
        self.kind
    }

    pub const fn risk(&self) -> PackageReviewCanonicalRowRisk {
        self.risk
    }

    pub const fn change(&self) -> ReviewOnlyCapabilityConflictChange {
        self.change
    }

    pub fn row_key(&self) -> &[u8] {
        &self.row_key
    }

    pub fn baseline_row(&self) -> Option<&[u8]> {
        self.baseline_row.as_deref()
    }

    pub fn candidate_row(&self) -> Option<&[u8]> {
        self.candidate_row.as_deref()
    }

    pub const fn baseline_source(&self) -> Option<&PackageReviewCanonicalRowSource> {
        self.baseline_source.as_ref()
    }

    pub const fn candidate_source(&self) -> Option<&PackageReviewCanonicalRowSource> {
        self.candidate_source.as_ref()
    }

    pub const fn fingerprint(&self) -> ReviewOnlyCapabilityConflictFingerprint {
        self.fingerprint
    }

    /// Whether root policy must resolve this row before update. Representation-
    /// TCB opacity alone recommends audit; blocking and opaque-blocking rows do
    /// not become implicit admissions.
    pub const fn is_blocking(&self) -> bool {
        matches!(
            self.risk,
            PackageReviewCanonicalRowRisk::Blocking | PackageReviewCanonicalRowRisk::OpaqueBlocking
        )
    }
}

/// Conflicts for one exact package identity and candidate dependency path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyPackageCapabilityConflicts {
    pub(super) key: PackageKey,
    pub(super) baseline_resolution: ImmutableSourceResolution,
    pub(super) candidate_resolution: ImmutableSourceResolution,
    pub(super) dependency_path: DependencyRequestPath,
    pub(super) baseline_source_consumption: ReviewOnlySourceConsumptionCommitment,
    pub(super) candidate_source_consumption: ReviewOnlySourceConsumptionCommitment,
    pub(super) candidate_closure: ReviewOnlyCandidateClosureCommitment,
    pub(super) conflicts: Vec<ReviewOnlyCapabilityConflict>,
}

impl ReviewOnlyPackageCapabilityConflicts {
    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn baseline_resolution(&self) -> &ImmutableSourceResolution {
        &self.baseline_resolution
    }

    pub fn candidate_resolution(&self) -> &ImmutableSourceResolution {
        &self.candidate_resolution
    }

    pub const fn dependency_path(&self) -> &DependencyRequestPath {
        &self.dependency_path
    }

    pub const fn baseline_source_consumption(&self) -> ReviewOnlySourceConsumptionCommitment {
        self.baseline_source_consumption
    }

    pub const fn candidate_source_consumption(&self) -> ReviewOnlySourceConsumptionCommitment {
        self.candidate_source_consumption
    }

    pub const fn candidate_closure(&self) -> ReviewOnlyCandidateClosureCommitment {
        self.candidate_closure
    }

    pub fn conflicts(&self) -> &[ReviewOnlyCapabilityConflict] {
        &self.conflicts
    }
}

/// Exact row conflicts for ordinary same-lineage updates.
///
/// New packages, removed packages, and source-lineage replacements remain in
/// source/provenance triage. Durable resolution is intentionally absent until
/// accepted lock and toolchain evidence are sealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewOnlyCapabilityConflictSet {
    pub(super) packages: Vec<ReviewOnlyPackageCapabilityConflicts>,
}

impl ReviewOnlyCapabilityConflictSet {
    pub fn packages(&self) -> &[ReviewOnlyPackageCapabilityConflicts] {
        &self.packages
    }

    pub fn conflict_count(&self) -> usize {
        self.packages
            .iter()
            .map(|package| package.conflicts.len())
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// Render a fixed-vocabulary, injection-resistant exact conflict view.
    ///
    /// Compiler rows are hexadecimal rather than decoded by package code. The
    /// companion source patch supplies the human-readable code delta.
    pub fn render_bounded(
        &self,
        maximum_bytes: usize,
    ) -> Result<String, ReviewOnlyCapabilityConflictRenderError> {
        let mut counter = RenderByteCounter::default();
        render_conflict_set(&mut counter, self);
        if counter.bytes > maximum_bytes {
            return Err(ReviewOnlyCapabilityConflictRenderError::LimitExceeded {
                maximum_bytes,
                required_bytes: counter.bytes,
            });
        }
        let mut rendered = String::new();
        rendered
            .try_reserve_exact(counter.bytes)
            .map_err(|_| ReviewOnlyCapabilityConflictRenderError::AllocationFailed)?;
        render_conflict_set(&mut rendered, self);
        debug_assert_eq!(rendered.len(), counter.bytes);
        Ok(rendered)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewSetRole {
    Baseline,
    Candidate,
}
