use crate::{PackageKey, PackageSourcePatchError, TriageRenderError};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceReviewCustodyRole {
    Baseline,
    Candidate,
}

#[derive(Debug)]
pub enum PackageSourceReviewError {
    MissingCustody {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    UnexpectedCustody {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    DuplicateCustody {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    ResolutionMismatch {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    DuplicateReview {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    ReviewIdentityMismatch {
        role: PackageSourceReviewCustodyRole,
        package: PackageKey,
    },
    MixedReviewTarget {
        role: PackageSourceReviewCustodyRole,
        first: PackageKey,
        conflicting: PackageKey,
    },
    ClosureValidationAllocationFailed,
    TooManySourcePatches {
        maximum: usize,
        required: usize,
    },
    SourcePatch {
        package: PackageKey,
        error: PackageSourcePatchError,
    },
}

impl fmt::Display for PackageSourceReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCustody { role, package } => write!(
                formatter,
                "{} review row `{}` has no matching resolver custody",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::UnexpectedCustody { role, package } => write!(
                formatter,
                "{} resolver custody `{}` has no matching compiler review row",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::DuplicateCustody { role, package } => write!(
                formatter,
                "{} resolver custody repeats package `{}`",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::ResolutionMismatch { role, package } => write!(
                formatter,
                "{} resolver custody and compiler review disagree on `{}` resolution",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::DuplicateReview { role, package } => write!(
                formatter,
                "{} compiler review set repeats package `{}`",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::ReviewIdentityMismatch { role, package } => write!(
                formatter,
                "{} compiler review identity does not match package `{}`",
                custody_role_token(*role),
                package.name().as_str()
            ),
            Self::MixedReviewTarget {
                role,
                first,
                conflicting,
            } => write!(
                formatter,
                "{} compiler review closure mixes targets between `{}` and `{}`",
                custody_role_token(*role),
                first.name().as_str(),
                conflicting.name().as_str()
            ),
            Self::ClosureValidationAllocationFailed => {
                formatter.write_str("package review closure validation allocation failed")
            }
            Self::TooManySourcePatches { maximum, required } => write!(
                formatter,
                "source review requires {required} patches, exceeding the {maximum}-patch ceiling"
            ),
            Self::SourcePatch { package, error } => write!(
                formatter,
                "cannot render source review for `{}`: {error}",
                package.name().as_str()
            ),
        }
    }
}

impl std::error::Error for PackageSourceReviewError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackageSourceReviewRenderError {
    Triage(TriageRenderError),
    TotalExceeded {
        maximum_bytes: usize,
        required_bytes: usize,
    },
}

impl fmt::Display for PackageSourceReviewRenderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Triage(error) => error.fmt(formatter),
            Self::TotalExceeded {
                maximum_bytes,
                required_bytes,
            } => write!(
                formatter,
                "package review input requires {required_bytes} bytes, exceeding the {maximum_bytes}-byte ceiling"
            ),
        }
    }
}

impl std::error::Error for PackageSourceReviewRenderError {}

const fn custody_role_token(role: PackageSourceReviewCustodyRole) -> &'static str {
    match role {
        PackageSourceReviewCustodyRole::Baseline => "baseline",
        PackageSourceReviewCustodyRole::Candidate => "candidate",
    }
}
