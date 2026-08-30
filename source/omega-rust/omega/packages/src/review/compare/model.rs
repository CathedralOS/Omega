//! Bounded conflict and error vocabulary.

use super::format::{RenderByteCounter, render_conflict_set, review_role_token};
use super::render_error::ReviewOnlyCapabilityConflictRenderError;
use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::resolution::graph::DependencyRequestPath;
use crate::review::candidate::ReviewOnlySourceConsumptionCommitment;
use omega_package_evidence::evidence::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
};
use omega_package_source::ImmutableSourceResolution;
use std::fmt;

/// Resource ceilings for exact review-row comparison.
///
/// Canonical row bytes have already been bounded by compiler input limits, but
/// comparison clones changed rows into review orchestration state. This
/// separate policy prevents a hostile graph from multiplying that memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewOnlyCapabilityConflictLimits {
    pub(super) maximum_packages: usize,
    pub(super) maximum_rows: usize,
    pub(super) maximum_row_key_bytes: usize,
    pub(super) maximum_encoded_row_bytes: usize,
    pub(super) maximum_source_locations: usize,
    pub(super) maximum_source_location_path_bytes: usize,
    pub(super) maximum_conflicts: usize,
    pub(super) maximum_changed_row_bytes: usize,
    pub(super) maximum_changed_source_location_bytes: usize,
    pub(super) maximum_dependency_path_steps: usize,
}

impl ReviewOnlyCapabilityConflictLimits {
    pub const fn new(
        maximum_packages: usize,
        maximum_rows: usize,
        maximum_row_key_bytes: usize,
        maximum_encoded_row_bytes: usize,
        maximum_source_locations: usize,
        maximum_source_location_path_bytes: usize,
        maximum_conflicts: usize,
        maximum_changed_row_bytes: usize,
        maximum_changed_source_location_bytes: usize,
        maximum_dependency_path_steps: usize,
    ) -> Self {
        Self {
            maximum_packages,
            maximum_rows,
            maximum_row_key_bytes,
            maximum_encoded_row_bytes,
            maximum_source_locations,
            maximum_source_location_path_bytes,
            maximum_conflicts,
            maximum_changed_row_bytes,
            maximum_changed_source_location_bytes,
            maximum_dependency_path_steps,
        }
    }

    pub const fn maximum_packages(self) -> usize {
        self.maximum_packages
    }

    pub const fn maximum_conflicts(self) -> usize {
        self.maximum_conflicts
    }

    pub const fn maximum_changed_row_bytes(self) -> usize {
        self.maximum_changed_row_bytes
    }

    pub const fn maximum_rows(self) -> usize {
        self.maximum_rows
    }

    pub const fn maximum_row_key_bytes(self) -> usize {
        self.maximum_row_key_bytes
    }

    pub const fn maximum_encoded_row_bytes(self) -> usize {
        self.maximum_encoded_row_bytes
    }

    pub const fn maximum_source_locations(self) -> usize {
        self.maximum_source_locations
    }

    pub const fn maximum_source_location_path_bytes(self) -> usize {
        self.maximum_source_location_path_bytes
    }

    pub const fn maximum_changed_source_location_bytes(self) -> usize {
        self.maximum_changed_source_location_bytes
    }

    pub const fn maximum_dependency_path_steps(self) -> usize {
        self.maximum_dependency_path_steps
    }
}

impl Default for ReviewOnlyCapabilityConflictLimits {
    fn default() -> Self {
        Self::new(
            4_096,
            131_072,
            16 * 1024 * 1024,
            32 * 1024 * 1024,
            262_144,
            16 * 1024 * 1024,
            65_536,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            1_024,
        )
    }
}

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

#[derive(Debug)]
pub enum ReviewOnlyCapabilityConflictError {
    DuplicateReview {
        role: ReviewSetRole,
        package: Box<PackageKey>,
    },
    ReviewIdentityMismatch {
        role: ReviewSetRole,
        package: Box<PackageKey>,
    },
    MissingCandidateCustody {
        package: Box<PackageKey>,
    },
    UnexpectedCandidateCustody {
        package: Box<PackageKey>,
    },
    CandidateResolutionMismatch {
        package: Box<PackageKey>,
    },
    MixedReviewTarget {
        role: ReviewSetRole,
        first: Box<PackageKey>,
        conflicting: Box<PackageKey>,
    },
    MissingDependencyPath {
        package: Box<PackageKey>,
    },
    TargetMismatch {
        package: Box<PackageKey>,
    },
    IncompleteRowProjection {
        package: Box<PackageKey>,
    },
    TooManyPackages {
        maximum: usize,
    },
    TooManyRows {
        maximum: usize,
    },
    RowKeyBytesExceeded {
        maximum_bytes: usize,
    },
    EncodedRowBytesExceeded {
        maximum_bytes: usize,
    },
    TooManySourceLocations {
        maximum: usize,
    },
    SourceLocationPathBytesExceeded {
        maximum_bytes: usize,
    },
    TooManyConflicts {
        maximum: usize,
    },
    ChangedRowBytesExceeded {
        maximum_bytes: usize,
    },
    ChangedSourceLocationBytesExceeded {
        maximum_bytes: usize,
    },
    DependencyPathTooLong {
        package: Box<PackageKey>,
        maximum_steps: usize,
    },
    InvalidCandidateSourceClosure,
    AllocationFailed,
}

impl fmt::Display for ReviewOnlyCapabilityConflictError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateReview { role, package } => write!(
                formatter,
                "{} review set repeats package `{}`",
                review_role_token(*role),
                package.name().as_str()
            ),
            Self::ReviewIdentityMismatch { role, package } => write!(
                formatter,
                "{} compiler review identity does not match package `{}`",
                review_role_token(*role),
                package.name().as_str()
            ),
            Self::MissingCandidateCustody { package } => write!(
                formatter,
                "candidate review `{}` has no resolver-owned source custody",
                package.name().as_str()
            ),
            Self::UnexpectedCandidateCustody { package } => write!(
                formatter,
                "candidate source custody `{}` has no compiler-issued review",
                package.name().as_str()
            ),
            Self::CandidateResolutionMismatch { package } => write!(
                formatter,
                "candidate source custody and compiler review disagree on `{}` resolution",
                package.name().as_str()
            ),
            Self::MixedReviewTarget {
                role,
                first,
                conflicting,
            } => write!(
                formatter,
                "{} review closure mixes targets between `{}` and `{}`",
                review_role_token(*role),
                first.name().as_str(),
                conflicting.name().as_str()
            ),
            Self::MissingDependencyPath { package } => write!(
                formatter,
                "validated candidate closure has no root path to `{}`",
                package.name().as_str()
            ),
            Self::TargetMismatch { package } => write!(
                formatter,
                "baseline and candidate review targets differ for `{}`",
                package.name().as_str()
            ),
            Self::IncompleteRowProjection { package } => write!(
                formatter,
                "canonical conflict rows do not completely represent review identity for `{}`",
                package.name().as_str()
            ),
            Self::TooManyPackages { maximum } => write!(
                formatter,
                "capability comparison exceeded its {maximum}-package conflict ceiling"
            ),
            Self::TooManyRows { maximum } => write!(
                formatter,
                "capability comparison exceeded its {maximum}-input-row ceiling"
            ),
            Self::RowKeyBytesExceeded { maximum_bytes } => write!(
                formatter,
                "capability comparison exceeded its {maximum_bytes}-byte row-key ceiling"
            ),
            Self::EncodedRowBytesExceeded { maximum_bytes } => write!(
                formatter,
                "capability comparison exceeded its {maximum_bytes}-byte encoded-row ceiling"
            ),
            Self::TooManySourceLocations { maximum } => write!(
                formatter,
                "capability comparison exceeded its {maximum}-source-location ceiling"
            ),
            Self::SourceLocationPathBytesExceeded { maximum_bytes } => write!(
                formatter,
                "capability comparison exceeded its {maximum_bytes}-byte source-location path ceiling"
            ),
            Self::TooManyConflicts { maximum } => write!(
                formatter,
                "capability comparison exceeded its {maximum}-row conflict ceiling"
            ),
            Self::ChangedRowBytesExceeded { maximum_bytes } => write!(
                formatter,
                "capability comparison exceeded its {maximum_bytes}-byte changed-row ceiling"
            ),
            Self::ChangedSourceLocationBytesExceeded { maximum_bytes } => write!(
                formatter,
                "capability comparison exceeded its {maximum_bytes}-byte changed-source-location ceiling"
            ),
            Self::DependencyPathTooLong {
                package,
                maximum_steps,
            } => write!(
                formatter,
                "candidate dependency path to `{}` exceeds its {maximum_steps}-step ceiling",
                package.name().as_str()
            ),
            Self::InvalidCandidateSourceClosure => formatter.write_str(
                "candidate source closure could not be canonically identified for review",
            ),
            Self::AllocationFailed => {
                formatter.write_str("capability conflict comparison allocation failed")
            }
        }
    }
}

impl std::error::Error for ReviewOnlyCapabilityConflictError {}
