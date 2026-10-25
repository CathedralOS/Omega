//! Aggregate canonical-policy accounting and package context for projection failures.

use super::CompileResolvedPackageReviewsError;
use crate::declarations::PackageKey;
use package_evidence::PackageReviewProjectionError;

// Canonical encoding size, not an allocator/heap limit.
pub(super) const MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn projection_failure(
    key: &PackageKey,
    error: PackageReviewProjectionError,
) -> CompileResolvedPackageReviewsError {
    use PackageReviewProjectionError as Error;
    match error {
        Error::SourceConsumptionMissing => {
            CompileResolvedPackageReviewsError::SourceConsumptionMissing {
                package: key.clone(),
            }
        }
        Error::Projection(diagnostics) => CompileResolvedPackageReviewsError::Projection {
            package: key.clone(),
            diagnostics,
        },
        Error::IdentityMismatch => CompileResolvedPackageReviewsError::IdentityMismatch {
            package: key.clone(),
        },
        Error::Encoding(error) => CompileResolvedPackageReviewsError::Encoding {
            package: key.clone(),
            error,
        },
        Error::PolicyBudget => CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
            package: key.clone(),
            maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
        },
        Error::SemanticBinding(error) => {
            super::super::semantic_bindings::projection_error(key, error)
        }
    }
}

pub(super) fn reserve(current: usize, additional: usize) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES)
}
