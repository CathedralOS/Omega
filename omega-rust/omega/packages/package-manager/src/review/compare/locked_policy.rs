//! Exact retained acceptance equality against fresh compiler policy.

use crate::declarations::PackageKey;
use crate::lock::PackageLockTarget;
use crate::review::timings;
use crate::review::{CompilerIssuedPackageReview};
use std::fmt;

/// Failure to associate fresh compiler output with an exact retained target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LockedPolicyComparisonError {
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