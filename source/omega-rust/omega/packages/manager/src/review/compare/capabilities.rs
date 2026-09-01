//! Exact capability-set comparison and row conflict construction.

use super::commitments::{
    ConflictFingerprintBaseline, derive_candidate_closure_commitment, derive_conflict_fingerprint,
};
use super::model::*;
use super::resources::{ComparisonInputBudget, account_review_resources};
use crate::declarations::PackageKey;
use crate::resolution::graph::{DependencyRequestPath, ResolvedPackageSourceClosure};
use crate::review::candidate::validation::{
    ReviewOnlyClosureValidationError, ReviewOnlySetValidationError, validate_review_only_closure,
    validate_review_only_records,
};
use crate::review::candidate::{PackageReviewEvidence, ReviewOnlyCanonicalRow};
use crate::review::{CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet};
use omega_package_evidence::record::{
    PackageReviewCanonicalRowKind, PackageReviewCanonicalRowRisk, PackageReviewCanonicalRowSource,
};
use std::cmp::Ordering;

pub fn compare_review_only_capabilities(
    baseline: &CompilerIssuedPackageReviewSet,
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: ReviewOnlyCapabilityConflictLimits,
) -> Result<ReviewOnlyCapabilityConflictSet, ReviewOnlyCapabilityConflictError> {
    compare_review_only_capability_records(baseline.reviews(), candidate, candidate_sources, limits)
}

/// Derive exact trust-bearing conflicts for a complete candidate closure
/// against an explicit empty admission baseline.
pub fn compare_review_only_initial_capabilities(
    candidate: &CompilerIssuedPackageReviewSet,
    candidate_sources: &ResolvedPackageSourceClosure,
    limits: ReviewOnlyCapabilityConflictLimits,
) -> Result<ReviewOnlyCapabilityConflictSet, ReviewOnlyCapabilityConflictError> {
    compare_review_only_capability_records::<CompilerIssuedPackageReview>(
        &[],
        candidate,
        candidate_sources,
        limits,
    )
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
        .try_reserve(candidate_by_key.len())
        .map_err(|_| ReviewOnlyCapabilityConflictError::AllocationFailed)?;
    let mut owned_budget = OwnedConflictBudget::default();
    for baseline_review in &baseline_by_key {
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
            *baseline_review,
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
            baseline: ReviewOnlyCapabilityConflictBaseline::RetainedReview {
                resolution: baseline_review.resolution().clone(),
                source_consumption: PackageReviewEvidence::source_consumption_commitment(
                    *baseline_review,
                ),
            },
            candidate_resolution: candidate_review.resolution().clone(),
            dependency_path,
            candidate_source_consumption: PackageReviewEvidence::source_consumption_commitment(
                candidate_review,
            ),
            candidate_closure,
            conflicts: row_conflicts,
        });
    }
    for candidate_review in &candidate_by_key {
        if baseline_by_key
            .binary_search_by(|review| review.key().cmp(candidate_review.key()))
            .is_ok()
        {
            continue;
        }
        let key = candidate_review.key();
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
        let row_conflicts = compare_empty_admission_rows(
            key,
            *candidate_review,
            &dependency_path,
            candidate_closure,
            limits,
            &mut owned_budget,
        )?;
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
            baseline: ReviewOnlyCapabilityConflictBaseline::EmptyAdmission,
            candidate_resolution: candidate_review.resolution().clone(),
            dependency_path,
            candidate_source_consumption: PackageReviewEvidence::source_consumption_commitment(
                *candidate_review,
            ),
            candidate_closure,
            conflicts: row_conflicts,
        });
    }
    packages.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(ReviewOnlyCapabilityConflictSet { packages })
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
            ConflictFingerprintBaseline::RetainedReview {
                resolution: baseline_review.resolution(),
                source_consumption: baseline_review.source_consumption_commitment(),
                whole_review: baseline_review.whole_review_commitment(),
            },
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

fn compare_empty_admission_rows<C: PackageReviewEvidence>(
    key: &PackageKey,
    candidate_review: &C,
    dependency_path: &DependencyRequestPath,
    candidate_closure: ReviewOnlyCandidateClosureCommitment,
    limits: ReviewOnlyCapabilityConflictLimits,
    budget: &mut OwnedConflictBudget,
) -> Result<Vec<ReviewOnlyCapabilityConflict>, ReviewOnlyCapabilityConflictError> {
    let selected = candidate_review
        .canonical_rows()
        .iter()
        .filter(|row| initial_admission_requires_root_policy(row.kind()));
    let mut conflicts = Vec::new();
    for row in selected {
        budget.conflicts = budget.conflicts.saturating_add(1);
        budget.bytes = budget
            .bytes
            .saturating_add(row.key_bytes().len())
            .saturating_add(row.canonical_bytes().len());
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
        budget.source_location_bytes = budget
            .source_location_bytes
            .saturating_add(source_metrics(row.source()).1);
        if budget.source_location_bytes > limits.maximum_changed_source_location_bytes {
            return Err(
                ReviewOnlyCapabilityConflictError::ChangedSourceLocationBytesExceeded {
                    maximum_bytes: limits.maximum_changed_source_location_bytes,
                },
            );
        }
        let row_key = clone_bytes(row.key_bytes())?;
        let candidate_row = clone_bytes(row.canonical_bytes())?;
        let candidate_source = row.source().clone();
        let fingerprint = derive_conflict_fingerprint(
            key,
            ConflictFingerprintBaseline::EmptyAdmission,
            candidate_review,
            dependency_path,
            candidate_closure,
            row.kind(),
            row.risk(),
            ReviewOnlyCapabilityConflictChange::Added,
            row.key_bytes(),
            None,
            Some(&candidate_row),
            None,
            Some(&candidate_source),
        );
        conflicts.push(ReviewOnlyCapabilityConflict {
            kind: row.kind(),
            risk: row.risk(),
            change: ReviewOnlyCapabilityConflictChange::Added,
            row_key,
            baseline_row: None,
            candidate_row: Some(candidate_row),
            baseline_source: None,
            candidate_source: Some(candidate_source),
            fingerprint,
        });
    }
    Ok(conflicts)
}

const fn initial_admission_requires_root_policy(kind: PackageReviewCanonicalRowKind) -> bool {
    matches!(
        kind,
        PackageReviewCanonicalRowKind::AcceptedClaim
            | PackageReviewCanonicalRowKind::DangerousAuthority
            | PackageReviewCanonicalRowKind::ExternalExecutableSupply
            | PackageReviewCanonicalRowKind::ContractEntailmentOpenObligation
    )
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

fn source_metrics(source: &PackageReviewCanonicalRowSource) -> (usize, usize) {
    let locations = source.authored_locations().unwrap_or_default();
    (
        locations.len(),
        locations.iter().fold(0usize, |bytes, location| {
            bytes.saturating_add(location.relative_path().len())
        }),
    )
}
