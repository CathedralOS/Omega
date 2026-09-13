//! Retain normalized policy from the same checked source as legacy review.

use super::CompileResolvedPackageReviewsError;
use crate::declarations::PackageKey;
use compiler::CheckedCompilation;
use package_evidence::{project_checked_package_policy, record::PackagePolicyBaseline};
use target::TargetProfile;

// This limits the sum of canonical encodings, not an exact measure of Vec
// capacities or allocator overhead. The component writer separately bounds
// each policy's encoding and recursive structure. Temporary sizing buffers
// are discarded; only the typed policies survive the disposable session.
const MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES: usize = 64 * 1024 * 1024;

pub(super) fn project(
    checked: &CheckedCompilation,
    key: &PackageKey,
    target: TargetProfile,
    current: usize,
) -> Result<(PackagePolicyBaseline, usize), CompileResolvedPackageReviewsError> {
    let policy =
        project_checked_package_policy(checked, target, key.identity()).map_err(|diagnostics| {
            CompileResolvedPackageReviewsError::Projection {
                package: key.clone(),
                diagnostics,
            }
        })?;
    if policy.package() != key.identity() || policy.target() != target {
        return Err(CompileResolvedPackageReviewsError::IdentityMismatch {
            package: key.clone(),
        });
    }
    let bytes =
        policy
            .canonical_bytes()
            .map_err(|error| CompileResolvedPackageReviewsError::Encoding {
                package: key.clone(),
                error,
            })?;
    let total = reserve(current, bytes.len()).ok_or_else(|| {
        CompileResolvedPackageReviewsError::RetainedPolicyCanonicalBudget {
            package: key.clone(),
            maximum_bytes: MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES,
        }
    })?;
    Ok((policy, total))
}

fn reserve(current: usize, additional: usize) -> Option<usize> {
    current
        .checked_add(additional)
        .filter(|total| *total <= MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_policy_budget_is_aggregate_exact_and_overflow_safe() {
        let ceiling = MAXIMUM_RETAINED_POLICY_CANONICAL_BYTES;
        assert_eq!(reserve(0, ceiling), Some(ceiling));
        assert_eq!(reserve(ceiling - 1, 1), Some(ceiling));
        assert_eq!(reserve(ceiling, 0), Some(ceiling));
        assert_eq!(reserve(ceiling, 1), None);
        assert_eq!(reserve(0, ceiling + 1), None);
        assert_eq!(reserve(usize::MAX, 1), None);
        assert_eq!(reserve(1, usize::MAX), None);
    }
}
