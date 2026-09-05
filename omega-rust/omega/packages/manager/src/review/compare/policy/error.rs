use crate::declarations::PackageKey;
use crate::resolution::graph::CanonicalSourceClosureSubjectError;
use omega_package_evidence::encoding::PackageReviewEncodingError;
use std::fmt;

#[derive(Debug)]
pub enum PackagePolicyChangeError {
    TargetMismatch,
    CandidateReview {
        package: Option<Box<PackageKey>>,
        reason: &'static str,
    },
    SourceSubject(CanonicalSourceClosureSubjectError),
    Projection {
        package: Box<PackageKey>,
        error: PackageReviewEncodingError,
    },
    IncompleteRowProjection {
        package: Box<PackageKey>,
    },
    InvalidSourcePath {
        package: Box<PackageKey>,
    },
    LimitExceeded {
        resource: &'static str,
        maximum: usize,
    },
    AllocationFailed,
}
impl fmt::Display for PackagePolicyChangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetMismatch => {
                formatter.write_str("normalized policy comparison requires one exact target")
            }
            Self::CandidateReview { package, reason } => write!(
                formatter,
                "invalid fresh candidate review {package:?}: {reason}"
            ),
            Self::SourceSubject(error) => {
                write!(formatter, "invalid candidate source subject: {error}")
            }
            Self::Projection { package, error } => write!(
                formatter,
                "cannot project normalized policy rows for {package:?}: {error}"
            ),
            Self::IncompleteRowProjection { package } => write!(
                formatter,
                "normalized rows do not cover the complete policy for {package:?}"
            ),
            Self::InvalidSourcePath { package } => write!(
                formatter,
                "source subject has no bounded path to {package:?}"
            ),
            Self::LimitExceeded { resource, maximum } => write!(
                formatter,
                "normalized policy comparison exceeds {resource} limit {maximum}"
            ),
            Self::AllocationFailed => {
                formatter.write_str("normalized policy comparison allocation failed")
            }
        }
    }
}
impl std::error::Error for PackagePolicyChangeError {}
