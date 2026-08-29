use crate::review::evidence::PackageReviewEvidence;
use crate::review::validation::{ReviewOnlyClosureValidationError, ReviewOnlySetValidationError};
use crate::{PackageKey, PackageSourceCustody};
use std::collections::BTreeMap;

use super::super::error::{PackageSourceReviewCustodyRole, PackageSourceReviewError};

pub(super) fn map_closure_validation_error(
    role: PackageSourceReviewCustodyRole,
    error: ReviewOnlyClosureValidationError,
) -> PackageSourceReviewError {
    match error {
        ReviewOnlyClosureValidationError::ReviewSet(error) => map_set_validation_error(role, error),
        ReviewOnlyClosureValidationError::MissingReview { package } => {
            PackageSourceReviewError::UnexpectedCustody { role, package }
        }
        ReviewOnlyClosureValidationError::UnexpectedReview { package } => {
            PackageSourceReviewError::MissingCustody { role, package }
        }
        ReviewOnlyClosureValidationError::ResolutionMismatch { package } => {
            PackageSourceReviewError::ResolutionMismatch { role, package }
        }
        ReviewOnlyClosureValidationError::AllocationFailed => {
            PackageSourceReviewError::ClosureValidationAllocationFailed
        }
    }
}

pub(super) fn map_set_validation_error(
    role: PackageSourceReviewCustodyRole,
    error: ReviewOnlySetValidationError,
) -> PackageSourceReviewError {
    match error {
        ReviewOnlySetValidationError::DuplicateReview { package } => {
            PackageSourceReviewError::DuplicateReview { role, package }
        }
        ReviewOnlySetValidationError::ProjectionIdentityMismatch { package } => {
            PackageSourceReviewError::ReviewIdentityMismatch { role, package }
        }
        ReviewOnlySetValidationError::MixedTarget { first, conflicting } => {
            PackageSourceReviewError::MixedReviewTarget {
                role,
                first,
                conflicting,
            }
        }
        ReviewOnlySetValidationError::MixedCompilerExecutableCommitment { first, conflicting } => {
            PackageSourceReviewError::MixedCompilerExecutableCommitment {
                role,
                first,
                conflicting,
            }
        }
        ReviewOnlySetValidationError::AllocationFailed => {
            PackageSourceReviewError::ClosureValidationAllocationFailed
        }
    }
}

pub(super) fn validate_partial_custody<'source, R: PackageReviewEvidence>(
    reviews: &[R],
    sources: &'source [PackageSourceCustody],
    role: PackageSourceReviewCustodyRole,
) -> Result<BTreeMap<PackageKey, &'source PackageSourceCustody>, PackageSourceReviewError> {
    let mut validated = BTreeMap::new();
    for custody in sources {
        let review = reviews
            .iter()
            .find(|review| review.key() == custody.key())
            .ok_or_else(|| PackageSourceReviewError::UnexpectedCustody {
                role,
                package: custody.key().clone(),
            })?;
        if custody.resolution() != review.resolution() {
            return Err(PackageSourceReviewError::ResolutionMismatch {
                role,
                package: custody.key().clone(),
            });
        }
        if validated.insert(custody.key().clone(), custody).is_some() {
            return Err(PackageSourceReviewError::DuplicateCustody {
                role,
                package: custody.key().clone(),
            });
        }
    }
    Ok(validated)
}
