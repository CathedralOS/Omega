//! Fail-closed comparison errors and fixed diagnostic rendering.

use std::fmt;

use super::format::review_role_token;
use super::model::ReviewSetRole;
use crate::declarations::PackageKey;

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
    CandidateTargetMismatch {
        package: Box<PackageKey>,
    },
    MixedReviewTarget {
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
    InvalidCandidateSourceClosure,
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
            Self::CandidateTargetMismatch { package } => write!(
                formatter,
                "candidate compiler review target differs from the requested target for `{}`",
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
            Self::InvalidCandidateSourceClosure => formatter.write_str(
                "candidate source closure could not be canonically identified for review",
            ),
            Self::AllocationFailed => {
                formatter.write_str("capability conflict comparison allocation failed")
            }
        }
    }
}

impl std::error::Error for ReviewOnlyCapabilityConflictError {}
