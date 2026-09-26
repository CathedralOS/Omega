use crate::declarations::PackageKey;
use crate::lock::{HistoricalPackagePolicyError, PackageLockError};
use crate::resolution::graph::CanonicalSourceClosureSubjectError;
use crate::review::{CompileResolvedPackageReviewsError, PackagePolicyChangeError};
use std::fmt;

#[derive(Debug)]
pub enum PackageChangeError {
    Compilation(CompileResolvedPackageReviewsError),
    UndischargedContract {
        package: Box<PackageKey>,
        count: usize,
    },
    Comparison(PackagePolicyChangeError),
    SourceSubject(CanonicalSourceClosureSubjectError),
    Decisions(HistoricalPackagePolicyError),
    RejectedChanges,
    Lock(PackageLockError),
    AllocationFailed,
}

impl fmt::Display for PackageChangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compilation(error) => {
                write!(formatter, "package candidate checking failed: {error}")
            }
            Self::UndischargedContract { package, count } => write!(
                formatter,
                "package {:?} has {count} unresolved contract obligations; project decisions cannot discharge proofs",
                package.name().as_str()
            ),
            Self::Comparison(error) => {
                write!(formatter, "package candidate comparison failed: {error}")
            }
            Self::SourceSubject(error) => {
                write!(formatter, "package candidate source record failed: {error}")
            }
            Self::Decisions(error) => error.fmt(formatter),
            Self::RejectedChanges => formatter.write_str(
                "project decisions reject the package candidate; accepted files are unchanged",
            ),
            Self::Lock(error) => {
                write!(formatter, "cannot construct proposed package lock: {error}")
            }
            Self::AllocationFailed => {
                formatter.write_str("package lock proposal allocation failed")
            }
        }
    }
}

impl std::error::Error for PackageChangeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Compilation(error) => Some(error),
            Self::Comparison(error) => Some(error),
            Self::SourceSubject(error) => Some(error),
            Self::Decisions(error) => Some(error),
            Self::Lock(error) => Some(error),
            Self::UndischargedContract { .. } | Self::RejectedChanges | Self::AllocationFailed => {
                None
            }
        }
    }
}
