//! Exact retained acceptance equality against fresh compiler policy.

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
    PurposeMismatch { package: PackageKey },
    ExecutionProfileMismatch { package: PackageKey },
    AllocationFailed,
    Acceptance(crate::lock::PackageLockError),
}

impl fmt::Display for LockedPolicyComparisonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (package, reason) = match self {
            Self::Acceptance(error) => return error.fmt(formatter),
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
            Self::PurposeMismatch { package } => (
                package,
                "was reviewed for a different build/product purpose",
            ),
            Self::ExecutionProfileMismatch { package } => (
                package,
                "was reviewed for a different build execution profile",
            ),
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
/// resolution and checked context before comparing retained policy meaning. The
/// lock's private construction guarantees complete, source/purpose-ordered
/// occurrences; neither role borrows the other role's acceptance.
///
/// Scratch slots are bounded by the retained occurrence count, output slots by
/// package count. Fresh policies project only the retained risk rows. Neither
/// equality nor a changed-key result
/// approves admissions, replays historical decisions, or issues fresh evidence.
pub fn compare_locked_package_policies(
    accepted: &PackageLockTarget,
    reviews: &CompilerIssuedPackageReviewSet,
) -> Result<Vec<PackageKey>, LockedPolicyComparisonError> {
    let sources = accepted.source().packages();
    let mut joined = Vec::new();
    joined
        .try_reserve_exact(accepted.occurrences().len())
        .map_err(|_| LockedPolicyComparisonError::AllocationFailed)?;
    let mut occurrences = accepted.occurrences().iter().peekable();
    for source in sources {
        while occurrences
            .peek()
            .is_some_and(|occurrence| occurrence.acceptance().package() == source.key().identity())
        {
            joined.push((
                source,
                occurrences.next().expect("peeked occurrence"),
                None::<&CompilerIssuedPackageReview>,
            ));
        }
    }

    for review in reviews.reviews() {
        let context = review.checked_context();
        let index = joined
            .binary_search_by(|(source, occurrence, _)| {
                (source.key(), occurrence.context().purpose())
                    .cmp(&(review.key(), context.purpose()))
            })
            .map_err(|_| {
                if sources
                    .binary_search_by(|source| source.key().cmp(review.key()))
                    .is_ok()
                {
                    LockedPolicyComparisonError::PurposeMismatch {
                        package: review.key().clone(),
                    }
                } else {
                    LockedPolicyComparisonError::UnexpectedReview {
                        package: review.key().clone(),
                    }
                }
            })?;
        let (source, occurrence, issued) = &mut joined[index];
        if issued.is_some() {
            return Err(LockedPolicyComparisonError::DuplicateReview {
                package: review.key().clone(),
            });
        }
        if review.resolution() != source.resolution() {
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
        if context.target() != occurrence.context().target()
            || review.projection().target() != context.target()
            || review.policy().target() != context.target()
        {
            return Err(LockedPolicyComparisonError::TargetMismatch {
                package: review.key().clone(),
            });
        }
        if context.build_execution_profile() != occurrence.context().build_execution_profile() {
            return Err(LockedPolicyComparisonError::ExecutionProfileMismatch {
                package: review.key().clone(),
            });
        }
        *issued = Some(review);
    }

    let mut changed = Vec::new();
    changed
        .try_reserve_exact(sources.len())
        .map_err(|_| LockedPolicyComparisonError::AllocationFailed)?;
    for (source, occurrence, review) in joined {
        let review = review.ok_or_else(|| LockedPolicyComparisonError::MissingReview {
            package: source.key().clone(),
        })?;
        let fresh = crate::lock::PackagePolicyAcceptance::from_policy(review.policy())
            .map_err(LockedPolicyComparisonError::Acceptance)?;
        if occurrence.acceptance() != &fresh && changed.last() != Some(source.key()) {
            changed.push(source.key().clone());
        }
    }
    Ok(changed)
}
