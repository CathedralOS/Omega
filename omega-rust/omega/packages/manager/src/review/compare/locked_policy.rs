//! Complete normalized policy equality, independent of legacy conflict rows.

use crate::declarations::PackageKey;
use crate::lock::PackageLockTarget;
use crate::review::{CompilerIssuedPackageReview, CompilerIssuedPackageReviewSet};
use std::fmt;

/// Failure to associate fresh compiler output with an exact retained target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockedPolicyComparisonError {
    MissingReview { package: PackageKey },
    UnexpectedReview { package: PackageKey },
    DuplicateReview { package: PackageKey },
    ResolutionMismatch { package: PackageKey },
    PackageIdentityMismatch { package: PackageKey },
    TargetMismatch { package: PackageKey },
    AllocationFailed,
}

impl fmt::Display for LockedPolicyComparisonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (package, reason) = match self {
            Self::MissingReview { package } => (package, "has no fresh compiler review"),
            Self::UnexpectedReview { package } => {
                (package, "is absent from the retained source graph")
            }
            Self::DuplicateReview { package } => (package, "has duplicate fresh compiler reviews"),
            Self::ResolutionMismatch { package } => {
                (package, "has a different immutable source resolution")
            }
            Self::PackageIdentityMismatch { package } => (
                package,
                "has a mismatched compiler projection or normalized policy owner",
            ),
            Self::TargetMismatch { package } => {
                (package, "was reviewed for a different exact target")
            }
            Self::AllocationFailed => {
                return formatter
                    .write_str("cannot allocate bounded locked-policy comparison storage");
            }
        };
        write!(formatter, "locked package {package:?} {reason}")
    }
}

impl std::error::Error for LockedPolicyComparisonError {}

/// Return changed package keys in the retained source's canonical order.
///
/// Reviews have no public constructor: the candidate owner joins their source,
/// checked projection, and complete normalized policy in one final compiler
/// pass. This helper checks that issued set against the exact retained package,
/// resolution, and target before comparing typed policy meaning. The lock's
/// private construction already guarantees complete, source-ordered baselines.
///
/// Scratch and output slots are bounded by that retained package count. No
/// policy is cloned or re-encoded. Neither equality nor a changed-key result
/// approves admissions, replays historical decisions, or issues fresh evidence.
pub fn compare_locked_package_policies(
    accepted: &PackageLockTarget,
    reviews: &CompilerIssuedPackageReviewSet,
) -> Result<Vec<PackageKey>, LockedPolicyComparisonError> {
    let sources = accepted.source().packages();
    let mut reviews_by_source = Vec::<Option<&CompilerIssuedPackageReview>>::new();
    reviews_by_source
        .try_reserve_exact(sources.len())
        .map_err(|_| LockedPolicyComparisonError::AllocationFailed)?;
    reviews_by_source.resize(sources.len(), None);

    for review in reviews.reviews() {
        let index = sources
            .binary_search_by(|source| source.key().cmp(review.key()))
            .map_err(|_| LockedPolicyComparisonError::UnexpectedReview {
                package: review.key().clone(),
            })?;
        if reviews_by_source[index].is_some() {
            return Err(LockedPolicyComparisonError::DuplicateReview {
                package: review.key().clone(),
            });
        }
        if review.resolution() != sources[index].resolution() {
            return Err(LockedPolicyComparisonError::ResolutionMismatch {
                package: review.key().clone(),
            });
        }
        if review.projection().package() != review.key().identity()
            || review.policy().package() != review.key().identity()
        {
            return Err(LockedPolicyComparisonError::PackageIdentityMismatch {
                package: review.key().clone(),
            });
        }
        if review.projection().target() != accepted.target()
            || review.policy().target() != accepted.target()
        {
            return Err(LockedPolicyComparisonError::TargetMismatch {
                package: review.key().clone(),
            });
        }
        reviews_by_source[index] = Some(review);
    }

    let mut changed = Vec::new();
    changed
        .try_reserve_exact(sources.len())
        .map_err(|_| LockedPolicyComparisonError::AllocationFailed)?;
    for ((source, baseline), review) in sources
        .iter()
        .zip(accepted.baselines())
        .zip(reviews_by_source)
    {
        let review = review.ok_or_else(|| LockedPolicyComparisonError::MissingReview {
            package: source.key().clone(),
        })?;
        if baseline != review.policy() {
            changed.push(source.key().clone());
        }
    }
    Ok(changed)
}
