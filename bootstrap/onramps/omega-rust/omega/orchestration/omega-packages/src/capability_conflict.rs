use crate::review_closure::{
    ReviewOnlyClosureValidationError, ReviewOnlySetValidationError, validate_review_only_closure,
    validate_review_only_records,
};
use crate::review_evidence::{
    PackageReviewEvidence, ReviewOnlyCanonicalRow, ReviewOnlyCompilerExecutableCommitment,
    ReviewOnlySourceConsumptionCommitment,
};
use crate::{
    CompilerIssuedPackageReviewSet, DependencyRequestPath, ImmutableSourceResolution, PackageKey,
    ResolvedPackageSourceClosure,
};
use omega_compiler::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocationOwner, PackageReviewSourceLocationRole,
    PackageReviewSyntheticSourceKind,
};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fmt;

const CONFLICT_FINGERPRINT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-CAPABILITY-CONFLICT\0";
const CONFLICT_FINGERPRINT_VERSION: u16 = 4;
const CANDIDATE_CLOSURE_DOMAIN: &[u8] = b"OMEGA-PACKAGE-CANDIDATE-CLOSURE\0";
const CANDIDATE_CLOSURE_VERSION: u16 = 1;
const CONFLICT_RENDER_SCHEMA: &str = "OMEGA_PACKAGE_CAPABILITY_CONFLICTS_V3\n";

/// Resource ceilings for exact review-row comparison.
///
/// Canonical row bytes have already been bounded by compiler input limits, but
/// comparison clones changed rows into review orchestration state. This
/// separate policy prevents a hostile graph from multiplying that memory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReviewOnlyCapabilityConflictLimits {
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

/// Domain-separated identity for one exact review-time row conflict.
///
/// This is suitable for joining a future root-policy decision to the conflict
/// it resolved. It is not itself a resolution, admission artifact, or proof of
/// review.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyCapabilityConflictFingerprint([u8; 32]);

impl ReviewOnlyCapabilityConflictFingerprint {
    pub const fn digest(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyCandidateClosureCommitment([u8; 32]);

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
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    change: ReviewOnlyCapabilityConflictChange,
    row_key: Vec<u8>,
    baseline_row: Option<Vec<u8>>,
    candidate_row: Option<Vec<u8>>,
    baseline_source: Option<PackageReviewCanonicalRowSource>,
    candidate_source: Option<PackageReviewCanonicalRowSource>,
    fingerprint: ReviewOnlyCapabilityConflictFingerprint,
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
    key: PackageKey,
    baseline_resolution: ImmutableSourceResolution,
    candidate_resolution: ImmutableSourceResolution,
    dependency_path: DependencyRequestPath,
    baseline_compiler: ReviewOnlyCompilerExecutableCommitment,
    candidate_compiler: ReviewOnlyCompilerExecutableCommitment,
    baseline_source_consumption: ReviewOnlySourceConsumptionCommitment,
    candidate_source_consumption: ReviewOnlySourceConsumptionCommitment,
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    conflicts: Vec<ReviewOnlyCapabilityConflict>,
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

    pub const fn baseline_compiler(&self) -> ReviewOnlyCompilerExecutableCommitment {
        self.baseline_compiler
    }

    pub const fn candidate_compiler(&self) -> ReviewOnlyCompilerExecutableCommitment {
        self.candidate_compiler
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
    packages: Vec<ReviewOnlyPackageCapabilityConflicts>,
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
    /// companion source patch supplies the human/LLM-readable code delta.
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
    MixedCompilerExecutableCommitment {
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
            Self::MixedCompilerExecutableCommitment {
                role,
                first,
                conflicting,
            } => write!(
                formatter,
                "{} review closure mixes compiler executable commitments between `{}` and `{}`",
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
            Self::AllocationFailed => {
                formatter.write_str("capability conflict comparison allocation failed")
            }
        }
    }
}

impl std::error::Error for ReviewOnlyCapabilityConflictError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOnlyCapabilityConflictRenderError {
    LimitExceeded {
        maximum_bytes: usize,
        required_bytes: usize,
    },
    AllocationFailed,
}

impl ReviewOnlyCapabilityConflictRenderError {
    pub const fn maximum_bytes(self) -> Option<usize> {
        match self {
            Self::LimitExceeded { maximum_bytes, .. } => Some(maximum_bytes),
            Self::AllocationFailed => None,
        }
    }

    pub const fn required_bytes(self) -> Option<usize> {
        match self {
            Self::LimitExceeded { required_bytes, .. } => Some(required_bytes),
            Self::AllocationFailed => None,
        }
    }
}

impl fmt::Display for ReviewOnlyCapabilityConflictRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExceeded {
                maximum_bytes,
                required_bytes,
            } => write!(
                formatter,
                "capability conflict view requires {required_bytes} bytes, exceeding the {maximum_bytes}-byte ceiling"
            ),
            Self::AllocationFailed => {
                formatter.write_str("capability conflict view allocation failed")
            }
        }
    }
}

impl std::error::Error for ReviewOnlyCapabilityConflictRenderError {}

/// Compare exact compiler-owned rows for every package retained under the same
/// source identity in baseline and candidate review sets.
pub fn compare_review_only_capabilities(
    baseline: &CompilerIssuedPackageReviewSet,
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: ReviewOnlyCapabilityConflictLimits,
) -> Result<ReviewOnlyCapabilityConflictSet, ReviewOnlyCapabilityConflictError> {
    compare_review_only_capability_records(baseline.reviews(), candidate, candidate_sources, limits)
}

pub(crate) fn compare_review_only_capability_records<B: PackageReviewEvidence>(
    baseline: &[B],
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: ReviewOnlyCapabilityConflictLimits,
) -> Result<ReviewOnlyCapabilityConflictSet, ReviewOnlyCapabilityConflictError> {
    let mut input_budget = ComparisonInputBudget::default();
    account_review_resources(baseline, limits, &mut input_budget)?;
    let baseline_by_key = validate_review_only_records(baseline)
        .map_err(|error| map_set_validation_error(ReviewSetRole::Baseline, error))?
        .into_reviews_by_key();
    account_review_resources(candidate.reviews(), limits, &mut input_budget)?;
    let candidate_by_key = validate_review_only_closure(candidate_sources, candidate)
        .map_err(map_candidate_closure_validation_error)?
        .into_reviews_by_key();
    let candidate_closure = derive_candidate_closure_commitment(candidate_sources)?;

    let mut packages = Vec::new();
    packages
        .try_reserve(baseline_by_key.len().min(candidate_by_key.len()))
        .map_err(|_| ReviewOnlyCapabilityConflictError::AllocationFailed)?;
    let mut owned_budget = OwnedConflictBudget::default();
    for baseline_review in baseline_by_key {
        let key = baseline_review.key();
        let Ok(candidate_index) = candidate_by_key.binary_search_by(|review| review.key().cmp(key))
        else {
            continue;
        };
        let candidate_review = candidate_by_key[candidate_index];
        if baseline_review.target_name() != candidate_review.target_name() {
            return Err(ReviewOnlyCapabilityConflictError::TargetMismatch {
                package: Box::new(key.clone()),
            });
        }
        let dependency_path = candidate_sources.dependency_path(key).ok_or_else(|| {
            ReviewOnlyCapabilityConflictError::MissingDependencyPath {
                package: Box::new(key.clone()),
            }
        })?;
        if dependency_path.steps().len() > limits.maximum_dependency_path_steps {
            return Err(ReviewOnlyCapabilityConflictError::DependencyPathTooLong {
                package: Box::new(key.clone()),
                maximum_steps: limits.maximum_dependency_path_steps,
            });
        }
        let row_conflicts = compare_rows(
            key,
            baseline_review,
            candidate_review,
            &dependency_path,
            candidate_closure,
            limits,
            &mut owned_budget,
        )?;
        let whole_review_changed =
            baseline_review.whole_review_commitment() != candidate_review.whole_review_commitment();
        if whole_review_changed == row_conflicts.is_empty() {
            return Err(ReviewOnlyCapabilityConflictError::IncompleteRowProjection {
                package: Box::new(key.clone()),
            });
        }
        if row_conflicts.is_empty() {
            continue;
        }

        if packages.len() >= limits.maximum_packages {
            return Err(ReviewOnlyCapabilityConflictError::TooManyPackages {
                maximum: limits.maximum_packages,
            });
        }
        packages.push(ReviewOnlyPackageCapabilityConflicts {
            key: key.clone(),
            baseline_resolution: baseline_review.resolution().clone(),
            candidate_resolution: candidate_review.resolution().clone(),
            dependency_path,
            baseline_compiler: PackageReviewEvidence::compiler_executable_commitment(
                baseline_review,
            ),
            candidate_compiler: PackageReviewEvidence::compiler_executable_commitment(
                candidate_review,
            ),
            baseline_source_consumption: PackageReviewEvidence::source_consumption_commitment(
                baseline_review,
            ),
            candidate_source_consumption: PackageReviewEvidence::source_consumption_commitment(
                candidate_review,
            ),
            candidate_closure,
            conflicts: row_conflicts,
        });
    }
    Ok(ReviewOnlyCapabilityConflictSet { packages })
}

#[derive(Default)]
struct ComparisonInputBudget {
    packages: usize,
    rows: usize,
    row_key_bytes: usize,
    encoded_row_bytes: usize,
    source_locations: usize,
    source_location_path_bytes: usize,
}

fn account_review_resources(
    reviews: &[impl PackageReviewEvidence],
    limits: ReviewOnlyCapabilityConflictLimits,
    budget: &mut ComparisonInputBudget,
) -> Result<(), ReviewOnlyCapabilityConflictError> {
    budget.packages = budget.packages.saturating_add(reviews.len());
    if budget.packages > limits.maximum_packages {
        return Err(ReviewOnlyCapabilityConflictError::TooManyPackages {
            maximum: limits.maximum_packages,
        });
    }
    for review in reviews {
        budget.rows = budget.rows.saturating_add(review.canonical_rows().len());
        budget.row_key_bytes = review
            .canonical_rows()
            .iter()
            .fold(budget.row_key_bytes, |bytes, row| {
                bytes.saturating_add(row.key_bytes().len())
            });
        budget.encoded_row_bytes = review
            .canonical_rows()
            .iter()
            .fold(budget.encoded_row_bytes, |bytes, row| {
                bytes.saturating_add(row.canonical_bytes().len())
            });
        for row in review.canonical_rows() {
            let (locations, path_bytes) = source_metrics(row.source());
            budget.source_locations = budget.source_locations.saturating_add(locations);
            budget.source_location_path_bytes =
                budget.source_location_path_bytes.saturating_add(path_bytes);
        }
        if budget.rows > limits.maximum_rows {
            return Err(ReviewOnlyCapabilityConflictError::TooManyRows {
                maximum: limits.maximum_rows,
            });
        }
        if budget.row_key_bytes > limits.maximum_row_key_bytes {
            return Err(ReviewOnlyCapabilityConflictError::RowKeyBytesExceeded {
                maximum_bytes: limits.maximum_row_key_bytes,
            });
        }
        if budget.encoded_row_bytes > limits.maximum_encoded_row_bytes {
            return Err(ReviewOnlyCapabilityConflictError::EncodedRowBytesExceeded {
                maximum_bytes: limits.maximum_encoded_row_bytes,
            });
        }
        if budget.source_locations > limits.maximum_source_locations {
            return Err(ReviewOnlyCapabilityConflictError::TooManySourceLocations {
                maximum: limits.maximum_source_locations,
            });
        }
        if budget.source_location_path_bytes > limits.maximum_source_location_path_bytes {
            return Err(
                ReviewOnlyCapabilityConflictError::SourceLocationPathBytesExceeded {
                    maximum_bytes: limits.maximum_source_location_path_bytes,
                },
            );
        }
    }
    Ok(())
}

fn map_candidate_closure_validation_error(
    error: ReviewOnlyClosureValidationError,
) -> ReviewOnlyCapabilityConflictError {
    match error {
        ReviewOnlyClosureValidationError::ReviewSet(error) => {
            map_set_validation_error(ReviewSetRole::Candidate, error)
        }
        ReviewOnlyClosureValidationError::MissingReview { package } => {
            ReviewOnlyCapabilityConflictError::UnexpectedCandidateCustody {
                package: Box::new(package),
            }
        }
        ReviewOnlyClosureValidationError::UnexpectedReview { package } => {
            ReviewOnlyCapabilityConflictError::MissingCandidateCustody {
                package: Box::new(package),
            }
        }
        ReviewOnlyClosureValidationError::ResolutionMismatch { package } => {
            ReviewOnlyCapabilityConflictError::CandidateResolutionMismatch {
                package: Box::new(package),
            }
        }
        ReviewOnlyClosureValidationError::AllocationFailed => {
            ReviewOnlyCapabilityConflictError::AllocationFailed
        }
    }
}

fn map_set_validation_error(
    role: ReviewSetRole,
    error: ReviewOnlySetValidationError,
) -> ReviewOnlyCapabilityConflictError {
    match error {
        ReviewOnlySetValidationError::DuplicateReview { package } => {
            ReviewOnlyCapabilityConflictError::DuplicateReview {
                role,
                package: Box::new(package),
            }
        }
        ReviewOnlySetValidationError::ProjectionIdentityMismatch { package } => {
            ReviewOnlyCapabilityConflictError::ReviewIdentityMismatch {
                role,
                package: Box::new(package),
            }
        }
        ReviewOnlySetValidationError::MixedTarget { first, conflicting } => {
            ReviewOnlyCapabilityConflictError::MixedReviewTarget {
                role,
                first: Box::new(first),
                conflicting: Box::new(conflicting),
            }
        }
        ReviewOnlySetValidationError::MixedCompilerExecutableCommitment { first, conflicting } => {
            ReviewOnlyCapabilityConflictError::MixedCompilerExecutableCommitment {
                role,
                first: Box::new(first),
                conflicting: Box::new(conflicting),
            }
        }
        ReviewOnlySetValidationError::AllocationFailed => {
            ReviewOnlyCapabilityConflictError::AllocationFailed
        }
    }
}

#[derive(Default)]
struct OwnedConflictBudget {
    conflicts: usize,
    bytes: usize,
    source_location_bytes: usize,
}

fn compare_rows<B: PackageReviewEvidence, C: PackageReviewEvidence>(
    key: &PackageKey,
    baseline_review: &B,
    candidate_review: &C,
    dependency_path: &DependencyRequestPath,
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    limits: ReviewOnlyCapabilityConflictLimits,
    budget: &mut OwnedConflictBudget,
) -> Result<Vec<ReviewOnlyCapabilityConflict>, ReviewOnlyCapabilityConflictError> {
    let baseline = baseline_review.canonical_rows();
    let candidate = candidate_review.canonical_rows();
    let mut conflicts = Vec::new();
    conflicts
        .try_reserve(baseline.len().saturating_add(candidate.len()))
        .map_err(|_| ReviewOnlyCapabilityConflictError::AllocationFailed)?;
    let mut baseline_index = 0usize;
    let mut candidate_index = 0usize;
    while baseline_index < baseline.len() || candidate_index < candidate.len() {
        let baseline_row = baseline.get(baseline_index);
        let candidate_row = candidate.get(candidate_index);
        let ordering = match (baseline_row, candidate_row) {
            (Some(left), Some(right)) => row_coordinate(left).cmp(&row_coordinate(right)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => break,
        };
        let (baseline_row, candidate_row) = match ordering {
            Ordering::Less => {
                baseline_index += 1;
                (baseline_row, None)
            }
            Ordering::Greater => {
                candidate_index += 1;
                (None, candidate_row)
            }
            Ordering::Equal => {
                baseline_index += 1;
                candidate_index += 1;
                (baseline_row, candidate_row)
            }
        };
        if matches!((baseline_row, candidate_row), (Some(left), Some(right)) if left.canonical_bytes() == right.canonical_bytes())
        {
            continue;
        }
        let exemplar = candidate_row
            .or(baseline_row)
            .expect("merged row has one side");
        let kind = exemplar.kind();
        let row_key = exemplar.key_bytes();
        let change = match (baseline_row, candidate_row) {
            (None, Some(_)) => ReviewOnlyCapabilityConflictChange::Added,
            (Some(_), None) => ReviewOnlyCapabilityConflictChange::Removed,
            (Some(_), Some(_)) => ReviewOnlyCapabilityConflictChange::Changed,
            (None, None) => unreachable!("row-key union contains at least one row"),
        };
        let risk = merge_risk(
            baseline_row.map(ReviewOnlyCanonicalRow::risk),
            candidate_row.map(ReviewOnlyCanonicalRow::risk),
        );
        let required_bytes = row_key
            .len()
            .saturating_add(baseline_row.map_or(0, |row| row.canonical_bytes().len()))
            .saturating_add(candidate_row.map_or(0, |row| row.canonical_bytes().len()));
        budget.conflicts = budget.conflicts.saturating_add(1);
        budget.bytes = budget.bytes.saturating_add(required_bytes);
        if budget.conflicts > limits.maximum_conflicts {
            return Err(ReviewOnlyCapabilityConflictError::TooManyConflicts {
                maximum: limits.maximum_conflicts,
            });
        }
        if budget.bytes > limits.maximum_changed_row_bytes {
            return Err(ReviewOnlyCapabilityConflictError::ChangedRowBytesExceeded {
                maximum_bytes: limits.maximum_changed_row_bytes,
            });
        }
        let changed_source_location_bytes = baseline_row
            .map_or(0, |row| source_metrics(row.source()).1)
            .saturating_add(candidate_row.map_or(0, |row| source_metrics(row.source()).1));
        budget.source_location_bytes = budget
            .source_location_bytes
            .saturating_add(changed_source_location_bytes);
        if budget.source_location_bytes > limits.maximum_changed_source_location_bytes {
            return Err(
                ReviewOnlyCapabilityConflictError::ChangedSourceLocationBytesExceeded {
                    maximum_bytes: limits.maximum_changed_source_location_bytes,
                },
            );
        }
        let baseline_bytes = baseline_row
            .map(|row| clone_bytes(row.canonical_bytes()))
            .transpose()?;
        let candidate_bytes = candidate_row
            .map(|row| clone_bytes(row.canonical_bytes()))
            .transpose()?;
        let owned_row_key = clone_bytes(row_key)?;
        let baseline_source = baseline_row.map(|row| row.source().clone());
        let candidate_source = candidate_row.map(|row| row.source().clone());
        let fingerprint = derive_conflict_fingerprint(
            key,
            baseline_review,
            candidate_review,
            dependency_path,
            candidate_closure,
            kind,
            risk,
            change,
            row_key,
            baseline_bytes.as_deref(),
            candidate_bytes.as_deref(),
            baseline_source.as_ref(),
            candidate_source.as_ref(),
        );
        conflicts.push(ReviewOnlyCapabilityConflict {
            kind,
            risk,
            change,
            row_key: owned_row_key,
            baseline_row: baseline_bytes,
            candidate_row: candidate_bytes,
            baseline_source,
            candidate_source,
            fingerprint,
        });
    }
    Ok(conflicts)
}

fn clone_bytes(bytes: &[u8]) -> Result<Vec<u8>, ReviewOnlyCapabilityConflictError> {
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(bytes.len())
        .map_err(|_| ReviewOnlyCapabilityConflictError::AllocationFailed)?;
    owned.extend_from_slice(bytes);
    Ok(owned)
}

fn row_coordinate(row: &ReviewOnlyCanonicalRow) -> (PackageReviewCanonicalRowKind, &[u8]) {
    (row.kind(), row.key_bytes())
}

fn merge_risk(
    baseline: Option<PackageReviewCanonicalRowRisk>,
    candidate: Option<PackageReviewCanonicalRowRisk>,
) -> PackageReviewCanonicalRowRisk {
    if baseline == Some(PackageReviewCanonicalRowRisk::OpaqueBlocking)
        || candidate == Some(PackageReviewCanonicalRowRisk::OpaqueBlocking)
    {
        PackageReviewCanonicalRowRisk::OpaqueBlocking
    } else if baseline == Some(PackageReviewCanonicalRowRisk::Blocking)
        || candidate == Some(PackageReviewCanonicalRowRisk::Blocking)
    {
        PackageReviewCanonicalRowRisk::Blocking
    } else {
        PackageReviewCanonicalRowRisk::AuditRecommended
    }
}

/// Highest policy risk among changed canonical rows. A whole-review change
/// not represented by rows fails closed as opaque blocking here; the explicit
/// comparator reports the stronger structural error.
pub(crate) fn changed_review_risk(
    baseline: &impl PackageReviewEvidence,
    candidate: &impl PackageReviewEvidence,
) -> Option<PackageReviewCanonicalRowRisk> {
    let baseline_rows = baseline.canonical_rows();
    let candidate_rows = candidate.canonical_rows();
    let mut baseline_index = 0usize;
    let mut candidate_index = 0usize;
    let mut changed = None;
    while baseline_index < baseline_rows.len() || candidate_index < candidate_rows.len() {
        let baseline_row = baseline_rows.get(baseline_index);
        let candidate_row = candidate_rows.get(candidate_index);
        let ordering = match (baseline_row, candidate_row) {
            (Some(left), Some(right)) => row_coordinate(left).cmp(&row_coordinate(right)),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => break,
        };
        let (baseline_row, candidate_row) = match ordering {
            Ordering::Less => {
                baseline_index += 1;
                (baseline_row, None)
            }
            Ordering::Greater => {
                candidate_index += 1;
                (None, candidate_row)
            }
            Ordering::Equal => {
                baseline_index += 1;
                candidate_index += 1;
                (baseline_row, candidate_row)
            }
        };
        if matches!((baseline_row, candidate_row), (Some(left), Some(right)) if left.canonical_bytes() == right.canonical_bytes())
        {
            continue;
        }
        let row_risk = merge_risk(
            baseline_row.map(ReviewOnlyCanonicalRow::risk),
            candidate_row.map(ReviewOnlyCanonicalRow::risk),
        );
        changed = Some(merge_risk(changed, Some(row_risk)));
    }
    if changed.is_none()
        && baseline.whole_review_commitment() != candidate.whole_review_commitment()
    {
        Some(PackageReviewCanonicalRowRisk::OpaqueBlocking)
    } else {
        changed
    }
}

#[allow(clippy::too_many_arguments)]
fn derive_conflict_fingerprint<B: PackageReviewEvidence, C: PackageReviewEvidence>(
    key: &PackageKey,
    baseline_review: &B,
    candidate_review: &C,
    dependency_path: &DependencyRequestPath,
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    kind: PackageReviewCanonicalRowKind,
    risk: PackageReviewCanonicalRowRisk,
    change: ReviewOnlyCapabilityConflictChange,
    row_key: &[u8],
    baseline_row: Option<&[u8]>,
    candidate_row: Option<&[u8]>,
    baseline_source: Option<&PackageReviewCanonicalRowSource>,
    candidate_source: Option<&PackageReviewCanonicalRowSource>,
) -> ReviewOnlyCapabilityConflictFingerprint {
    let mut digest = Sha256::new();
    hash_field(&mut digest, CONFLICT_FINGERPRINT_DOMAIN);
    digest.update(CONFLICT_FINGERPRINT_VERSION.to_le_bytes());
    hash_field(&mut digest, &key.identity().digest());
    hash_resolution(&mut digest, baseline_review.resolution());
    hash_resolution(&mut digest, candidate_review.resolution());
    hash_field(
        &mut digest,
        &PackageReviewEvidence::compiler_executable_commitment(baseline_review).digest(),
    );
    hash_field(
        &mut digest,
        &PackageReviewEvidence::compiler_executable_commitment(candidate_review).digest(),
    );
    hash_field(
        &mut digest,
        &PackageReviewEvidence::source_consumption_commitment(baseline_review).digest(),
    );
    hash_field(
        &mut digest,
        &PackageReviewEvidence::source_consumption_commitment(candidate_review).digest(),
    );
    hash_field(&mut digest, &baseline_review.whole_review_commitment());
    hash_field(&mut digest, &candidate_review.whole_review_commitment());
    hash_field(&mut digest, &candidate_closure.digest());
    hash_dependency_path(&mut digest, dependency_path);
    digest.update([row_kind_tag(kind), row_risk_tag(risk), change_tag(change)]);
    hash_field(&mut digest, row_key);
    hash_optional_field(&mut digest, baseline_row);
    hash_optional_field(&mut digest, candidate_row);
    hash_optional_row_source(&mut digest, baseline_source);
    hash_optional_row_source(&mut digest, candidate_source);
    ReviewOnlyCapabilityConflictFingerprint(digest.finalize().into())
}

fn derive_candidate_closure_commitment(
    closure: &ResolvedPackageSourceClosure,
) -> Result<ReviewOnlyCandidateClosureCommitment, ReviewOnlyCapabilityConflictError> {
    let mut digest = Sha256::new();
    hash_field(&mut digest, CANDIDATE_CLOSURE_DOMAIN);
    digest.update(CANDIDATE_CLOSURE_VERSION.to_le_bytes());
    hash_field(&mut digest, &closure.graph().root().identity().digest());
    let mut packages = Vec::new();
    packages
        .try_reserve(closure.graph().packages().len())
        .map_err(|_| ReviewOnlyCapabilityConflictError::AllocationFailed)?;
    packages.extend(closure.graph().packages());
    packages.sort_by(|left, right| left.source().key().cmp(right.source().key()));
    digest.update(
        u64::try_from(packages.len())
            .expect("bounded package count fits u64")
            .to_le_bytes(),
    );
    for package in packages {
        hash_field(&mut digest, &package.source().key().identity().digest());
        hash_resolution(&mut digest, package.source().resolution());
        digest.update(
            u64::try_from(package.dependencies().len())
                .expect("bounded dependency count fits u64")
                .to_le_bytes(),
        );
        for (dependency_index, dependency) in package.dependencies().iter().enumerate() {
            digest.update(
                u64::try_from(dependency_index)
                    .expect("bounded dependency index fits u64")
                    .to_le_bytes(),
            );
            hash_field(&mut digest, dependency.alias().as_str().as_bytes());
            hash_field(&mut digest, &dependency.target().identity().digest());
        }
    }
    Ok(ReviewOnlyCandidateClosureCommitment(
        digest.finalize().into(),
    ))
}

fn hash_resolution(digest: &mut Sha256, resolution: &ImmutableSourceResolution) {
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            digest.update([0]);
            hash_field(digest, commit.to_hex().as_bytes());
            hash_field(digest, tree.to_hex().as_bytes());
            hash_field(digest, content.to_hex().as_bytes());
        }
        ImmutableSourceResolution::Workspace { content } => {
            digest.update([1]);
            hash_field(digest, content.to_hex().as_bytes());
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            digest.update([2]);
            hash_field(digest, content.to_hex().as_bytes());
        }
    }
}

fn hash_dependency_path(digest: &mut Sha256, path: &DependencyRequestPath) {
    hash_field(digest, &path.root().identity().digest());
    digest.update(
        u64::try_from(path.steps().len())
            .expect("bounded dependency path length fits u64")
            .to_le_bytes(),
    );
    for step in path.steps() {
        hash_field(digest, &step.requester().identity().digest());
        digest.update(
            u64::try_from(step.dependency_index())
                .expect("dependency index fits u64")
                .to_le_bytes(),
        );
        hash_field(digest, step.alias().as_str().as_bytes());
        hash_field(digest, &step.target().identity().digest());
    }
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("canonical conflict field length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

fn hash_optional_field(digest: &mut Sha256, bytes: Option<&[u8]>) {
    match bytes {
        Some(bytes) => {
            digest.update([1]);
            hash_field(digest, bytes);
        }
        None => digest.update([0]),
    }
}

fn hash_optional_row_source(digest: &mut Sha256, source: Option<&PackageReviewCanonicalRowSource>) {
    match source {
        None => digest.update([0]),
        Some(source) => {
            digest.update([1]);
            let locations = source.authored_locations().unwrap_or_default();
            digest.update(
                u64::try_from(locations.len())
                    .expect("bounded source-location count fits u64")
                    .to_le_bytes(),
            );
            for location in locations {
                match location.owner() {
                    PackageReviewSourceLocationOwner::Package(package) => {
                        digest.update([0]);
                        hash_field(digest, &package.digest());
                    }
                    PackageReviewSourceLocationOwner::Toolchain(source) => {
                        digest.update([1]);
                        hash_field(digest, &source.digest());
                    }
                }
                hash_field(digest, location.relative_path().as_bytes());
                digest.update(location.start_byte().to_le_bytes());
                digest.update(location.end_byte().to_le_bytes());
                digest.update([source_location_role_tag(location.role())]);
            }
            digest.update(
                u64::try_from(source.compiler_derivations().len())
                    .expect("bounded compiler-derivation count fits u64")
                    .to_le_bytes(),
            );
            for kind in source.compiler_derivations() {
                digest.update([synthetic_source_kind_tag(*kind)]);
            }
        }
    }
}

fn source_metrics(source: &PackageReviewCanonicalRowSource) -> (usize, usize) {
    let locations = source.authored_locations().unwrap_or_default();
    (
        locations.len(),
        locations.iter().fold(0usize, |bytes, location| {
            bytes.saturating_add(location.relative_path().len())
        }),
    )
}

trait ConflictRenderOutput {
    fn push_str(&mut self, value: &str);

    fn push(&mut self, value: char) {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes));
    }
}

impl ConflictRenderOutput for String {
    fn push_str(&mut self, value: &str) {
        String::push_str(self, value);
    }
}

#[derive(Default)]
struct RenderByteCounter {
    bytes: usize,
}

impl ConflictRenderOutput for RenderByteCounter {
    fn push_str(&mut self, value: &str) {
        self.bytes = self.bytes.saturating_add(value.len());
    }
}

fn render_conflict_set(
    output: &mut impl ConflictRenderOutput,
    set: &ReviewOnlyCapabilityConflictSet,
) {
    output.push_str(CONFLICT_RENDER_SCHEMA);
    output.push_str("package_count ");
    output.push_str(&set.packages.len().to_string());
    output.push('\n');
    for package in &set.packages {
        render_package(output, package);
    }
    output.push_str("end_capability_conflicts\n");
}

fn render_package(
    output: &mut impl ConflictRenderOutput,
    package: &ReviewOnlyPackageCapabilityConflicts,
) {
    output.push_str("package_begin\npackage_name ");
    output.push_str(package.key.name().as_str());
    output.push_str("\npackage_key ");
    push_hex(output, &package.key.identity().digest());
    output.push('\n');
    render_resolution(output, "baseline_resolution", &package.baseline_resolution);
    render_resolution(
        output,
        "candidate_resolution",
        &package.candidate_resolution,
    );
    render_digest(
        output,
        "baseline_compiler",
        &package.baseline_compiler.digest(),
    );
    render_digest(
        output,
        "candidate_compiler",
        &package.candidate_compiler.digest(),
    );
    render_digest(
        output,
        "baseline_source_consumption",
        &package.baseline_source_consumption.digest(),
    );
    render_digest(
        output,
        "candidate_source_consumption",
        &package.candidate_source_consumption.digest(),
    );
    render_digest(
        output,
        "candidate_closure",
        &package.candidate_closure.digest(),
    );
    output.push_str("dependency_root ");
    push_hex(output, &package.dependency_path.root().identity().digest());
    output.push('\n');
    for step in package.dependency_path.steps() {
        output.push_str("dependency_step ");
        output.push_str(&step.dependency_index().to_string());
        output.push(' ');
        output.push_str(step.alias().as_str());
        output.push(' ');
        push_hex(output, &step.requester().identity().digest());
        output.push(' ');
        push_hex(output, &step.target().identity().digest());
        output.push('\n');
    }
    output.push_str("conflict_count ");
    output.push_str(&package.conflicts.len().to_string());
    output.push('\n');
    for conflict in &package.conflicts {
        output.push_str("conflict_begin\nfingerprint ");
        push_hex(output, &conflict.fingerprint.digest());
        output.push_str("\nchange ");
        output.push_str(change_token(conflict.change));
        output.push_str("\nkind ");
        output.push_str(row_kind_token(conflict.kind));
        output.push_str("\nrisk ");
        output.push_str(row_risk_token(conflict.risk));
        output.push_str("\nrow_key ");
        render_bytes_summary(output, &conflict.row_key);
        output.push('\n');
        render_optional_bytes_summary(output, "baseline_row", conflict.baseline_row.as_deref());
        render_optional_bytes_summary(output, "candidate_row", conflict.candidate_row.as_deref());
        render_optional_row_source(output, "baseline", conflict.baseline_source.as_ref());
        render_optional_row_source(output, "candidate", conflict.candidate_source.as_ref());
        output.push_str("conflict_end\n");
    }
    output.push_str("package_end\n");
}

fn render_resolution(
    output: &mut impl ConflictRenderOutput,
    label: &str,
    resolution: &ImmutableSourceResolution,
) {
    output.push_str(label);
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => {
            output.push_str(" git ");
            output.push_str(&commit.to_hex());
            output.push(' ');
            output.push_str(&tree.to_hex());
            output.push(' ');
            output.push_str(&content.to_hex());
        }
        ImmutableSourceResolution::Workspace { content } => {
            output.push_str(" workspace ");
            output.push_str(&content.to_hex());
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            output.push_str(" external_local ");
            output.push_str(&content.to_hex());
        }
    }
    output.push('\n');
}

fn render_digest(output: &mut impl ConflictRenderOutput, label: &str, digest: &[u8; 32]) {
    output.push_str(label);
    output.push(' ');
    push_hex(output, digest);
    output.push('\n');
}

fn render_optional_bytes_summary(
    output: &mut impl ConflictRenderOutput,
    label: &str,
    bytes: Option<&[u8]>,
) {
    output.push_str(label);
    output.push(' ');
    if let Some(bytes) = bytes {
        render_bytes_summary(output, bytes);
    } else {
        output.push_str("none");
    }
    output.push('\n');
}

fn render_optional_row_source(
    output: &mut impl ConflictRenderOutput,
    label: &str,
    source: Option<&PackageReviewCanonicalRowSource>,
) {
    output.push_str(label);
    output.push_str("_source ");
    match source {
        None => output.push_str("absent_row\n"),
        Some(source) => {
            let locations = source.authored_locations().unwrap_or_default();
            output.push_str("present authored ");
            output.push_str(&locations.len().to_string());
            output.push_str(" compiler_derived ");
            output.push_str(&source.compiler_derivations().len().to_string());
            output.push('\n');
            for kind in source.compiler_derivations() {
                output.push_str(label);
                output.push_str("_derivation ");
                output.push_str(synthetic_source_kind_token(*kind));
                output.push('\n');
            }
            for location in locations {
                output.push_str(label);
                output.push_str("_location ");
                output.push_str(source_location_role_token(location.role()));
                output.push(' ');
                match location.owner() {
                    PackageReviewSourceLocationOwner::Package(package) => {
                        output.push_str("package ");
                        push_hex(output, &package.digest());
                    }
                    PackageReviewSourceLocationOwner::Toolchain(source) => {
                        output.push_str("toolchain ");
                        push_hex(output, &source.digest());
                    }
                }
                output.push(' ');
                output.push_str(&location.start_byte().to_string());
                output.push(' ');
                output.push_str(&location.end_byte().to_string());
                output.push(' ');
                push_escaped_path(output, location.relative_path().as_bytes());
                output.push('\n');
            }
        }
    }
}

const fn synthetic_source_kind_tag(kind: PackageReviewSyntheticSourceKind) -> u8 {
    match kind {
        PackageReviewSyntheticSourceKind::ProjectionHeader => 0,
        PackageReviewSyntheticSourceKind::EmptySelectedProviderSet => 1,
        PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection => 2,
        PackageReviewSyntheticSourceKind::FreeExternalProviderType => 3,
    }
}

const fn synthetic_source_kind_token(kind: PackageReviewSyntheticSourceKind) -> &'static str {
    match kind {
        PackageReviewSyntheticSourceKind::ProjectionHeader => "projection_header",
        PackageReviewSyntheticSourceKind::EmptySelectedProviderSet => "empty_selected_provider_set",
        PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection => {
            "unique_covering_provider_selection"
        }
        PackageReviewSyntheticSourceKind::FreeExternalProviderType => "free_external_provider_type",
    }
}

fn push_escaped_path(output: &mut impl ConflictRenderOutput, path: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    output.push('"');
    for byte in path {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-' | b'/' | b'<' | b'>' => {
                output.push(char::from(*byte))
            }
            b'\\' => output.push_str("\\\\"),
            _ => {
                output.push_str("\\x");
                output.push(char::from(HEX[usize::from(byte >> 4)]));
                output.push(char::from(HEX[usize::from(byte & 0x0f)]));
            }
        }
    }
    output.push('"');
}

fn render_bytes_summary(output: &mut impl ConflictRenderOutput, bytes: &[u8]) {
    output.push_str("length ");
    output.push_str(&bytes.len().to_string());
    output.push_str(" sha256 ");
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    push_hex(output, &digest);
}

fn push_hex(output: &mut impl ConflictRenderOutput, bytes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
}

const fn review_role_token(role: ReviewSetRole) -> &'static str {
    match role {
        ReviewSetRole::Baseline => "baseline",
        ReviewSetRole::Candidate => "candidate",
    }
}

const fn change_token(change: ReviewOnlyCapabilityConflictChange) -> &'static str {
    match change {
        ReviewOnlyCapabilityConflictChange::Added => "added",
        ReviewOnlyCapabilityConflictChange::Removed => "removed",
        ReviewOnlyCapabilityConflictChange::Changed => "changed",
    }
}

const fn row_kind_token(kind: PackageReviewCanonicalRowKind) -> &'static str {
    match kind {
        PackageReviewCanonicalRowKind::ProjectionHeader => "projection_header",
        PackageReviewCanonicalRowKind::PublicTrait => "public_trait",
        PackageReviewCanonicalRowKind::PublicDomain => "public_domain",
        PackageReviewCanonicalRowKind::PublicData => "public_data",
        PackageReviewCanonicalRowKind::PublicProposition => "public_proposition",
        PackageReviewCanonicalRowKind::PublicConst => "public_const",
        PackageReviewCanonicalRowKind::PublicOperator => "public_operator",
        PackageReviewCanonicalRowKind::PublicConformance => "public_conformance",
        PackageReviewCanonicalRowKind::RepresentationTcb => "representation_tcb",
        PackageReviewCanonicalRowKind::Callable => "callable",
        PackageReviewCanonicalRowKind::DangerousAuthority => "dangerous_authority",
        PackageReviewCanonicalRowKind::SelectedProviderSet => "selected_provider_set",
        PackageReviewCanonicalRowKind::AcceptedClaim => "accepted_claim",
        PackageReviewCanonicalRowKind::DangerousAuthoritySlack => "dangerous_authority_slack",
        PackageReviewCanonicalRowKind::SemanticDependency => "semantic_dependency",
    }
}

const fn row_kind_tag(kind: PackageReviewCanonicalRowKind) -> u8 {
    match kind {
        PackageReviewCanonicalRowKind::ProjectionHeader => 0,
        PackageReviewCanonicalRowKind::PublicTrait => 1,
        PackageReviewCanonicalRowKind::PublicDomain => 2,
        PackageReviewCanonicalRowKind::PublicData => 3,
        PackageReviewCanonicalRowKind::RepresentationTcb => 4,
        PackageReviewCanonicalRowKind::Callable => 5,
        PackageReviewCanonicalRowKind::DangerousAuthority => 6,
        PackageReviewCanonicalRowKind::SelectedProviderSet => 7,
        PackageReviewCanonicalRowKind::AcceptedClaim => 8,
        PackageReviewCanonicalRowKind::DangerousAuthoritySlack => 9,
        PackageReviewCanonicalRowKind::SemanticDependency => 10,
        PackageReviewCanonicalRowKind::PublicProposition => 11,
        PackageReviewCanonicalRowKind::PublicConst => 12,
        PackageReviewCanonicalRowKind::PublicOperator => 13,
        PackageReviewCanonicalRowKind::PublicConformance => 14,
    }
}

const fn row_risk_token(risk: PackageReviewCanonicalRowRisk) -> &'static str {
    match risk {
        PackageReviewCanonicalRowRisk::Blocking => "blocking",
        PackageReviewCanonicalRowRisk::AuditRecommended => "audit_recommended",
        PackageReviewCanonicalRowRisk::OpaqueBlocking => "opaque_blocking",
    }
}

const fn row_risk_tag(risk: PackageReviewCanonicalRowRisk) -> u8 {
    match risk {
        PackageReviewCanonicalRowRisk::Blocking => 0,
        PackageReviewCanonicalRowRisk::AuditRecommended => 1,
        PackageReviewCanonicalRowRisk::OpaqueBlocking => 2,
    }
}

const fn change_tag(change: ReviewOnlyCapabilityConflictChange) -> u8 {
    match change {
        ReviewOnlyCapabilityConflictChange::Added => 0,
        ReviewOnlyCapabilityConflictChange::Removed => 1,
        ReviewOnlyCapabilityConflictChange::Changed => 2,
    }
}

const fn source_location_role_tag(role: PackageReviewSourceLocationRole) -> u8 {
    match role {
        PackageReviewSourceLocationRole::Declaration => 0,
        PackageReviewSourceLocationRole::DerivationOrigin => 1,
        PackageReviewSourceLocationRole::AuthorityDeclaration => 2,
        PackageReviewSourceLocationRole::AuthorityExposure => 3,
        PackageReviewSourceLocationRole::ProviderSelection => 4,
        PackageReviewSourceLocationRole::ProviderSchemaDeclaration => 5,
        PackageReviewSourceLocationRole::ProviderTypeDeclaration => 6,
        PackageReviewSourceLocationRole::ProviderRealization => 7,
        PackageReviewSourceLocationRole::SemanticDependencyConsumer => 8,
        PackageReviewSourceLocationRole::SemanticDependencyDeclaration => 9,
        PackageReviewSourceLocationRole::ProviderRequirementDeclaration => 10,
    }
}

const fn source_location_role_token(role: PackageReviewSourceLocationRole) -> &'static str {
    match role {
        PackageReviewSourceLocationRole::Declaration => "declaration",
        PackageReviewSourceLocationRole::DerivationOrigin => "derivation_origin",
        PackageReviewSourceLocationRole::AuthorityDeclaration => "authority_declaration",
        PackageReviewSourceLocationRole::AuthorityExposure => "authority_exposure",
        PackageReviewSourceLocationRole::ProviderSelection => "provider_selection",
        PackageReviewSourceLocationRole::ProviderSchemaDeclaration => "provider_schema_declaration",
        PackageReviewSourceLocationRole::ProviderTypeDeclaration => "provider_type_declaration",
        PackageReviewSourceLocationRole::ProviderRequirementDeclaration => {
            "provider_requirement_declaration"
        }
        PackageReviewSourceLocationRole::ProviderRealization => "provider_realization",
        PackageReviewSourceLocationRole::SemanticDependencyConsumer => {
            "semantic_dependency_consumer"
        }
        PackageReviewSourceLocationRole::SemanticDependencyDeclaration => {
            "semantic_dependency_declaration"
        }
    }
}
