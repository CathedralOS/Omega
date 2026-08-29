//! Exact row comparison, resource accounting, and closure commitments.

use super::format::{
    change_tag, row_kind_tag, row_risk_tag, source_location_role_tag, synthetic_source_kind_tag,
};
use super::model::*;
use crate::review::evidence::{PackageReviewEvidence, ReviewOnlyCanonicalRow};
use crate::review::validation::{
    ReviewOnlyClosureValidationError, ReviewOnlySetValidationError, validate_review_only_closure,
    validate_review_only_records,
};
use crate::{
    CompilerIssuedPackageReviewSet, DependencyRequestPath, ImmutableSourceResolution, PackageKey,
    ResolvedPackageSourceClosure,
};
use omega_package_review::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
    PackageReviewSourceLocationOwner,
};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;

const CONFLICT_FINGERPRINT_DOMAIN: &[u8] = b"OMEGA-PACKAGE-CAPABILITY-CONFLICT\0";
const CONFLICT_FINGERPRINT_VERSION: u16 = 17;
const CANDIDATE_CLOSURE_DOMAIN: &[u8] = b"OMEGA-PACKAGE-CANDIDATE-CLOSURE\0";
const CANDIDATE_CLOSURE_VERSION: u16 = 3;

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
    let candidate_closure =
        derive_candidate_closure_commitment(candidate_sources, &candidate_by_key)?;

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

pub(super) fn derive_candidate_closure_commitment<C: PackageReviewEvidence>(
    closure: &ResolvedPackageSourceClosure,
    candidate_reviews: &[&C],
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
        let review_index = candidate_reviews
            .binary_search_by(|review| review.key().cmp(package.source().key()))
            .expect("validated candidate closure has one review per source");
        let review = candidate_reviews[review_index];
        hash_field(&mut digest, &package.source().key().identity().digest());
        hash_resolution(&mut digest, package.source().resolution());
        hash_field(&mut digest, review.target_name().as_bytes());
        hash_field(
            &mut digest,
            &review.source_consumption_commitment().digest(),
        );
        match review.build_observation_commitment() {
            None => digest.update([0]),
            Some(commitment) => {
                digest.update([1]);
                hash_field(&mut digest, &commitment);
            }
        }
        hash_field(&mut digest, &review.whole_review_commitment());
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
