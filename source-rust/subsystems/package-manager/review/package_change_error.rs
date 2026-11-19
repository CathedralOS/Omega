// Error vocabulary (`PackageChangeError`) for candidate review, policy comparison, and proposed lock target construction.

use crate::lock::{HistoricalPackagePolicyError, PackageLockError};
use crate::resolution::graph::CanonicalSourceClosureSubjectError;
use crate::review::{CompileResolvedPackageReviewsError, PackagePolicyChangeError};
use std::fmt;

#[derive(Debug)]
pub enum PackageChangeError {
    CompilerIdentity(String),
    Compilation(CompileResolvedPackageReviewsError),
    Comparison(PackagePolicyChangeError),
    SourceSubject(CanonicalSourceClosureSubjectError),
    Decisions(HistoricalPackagePolicyError),
    RejectedChanges,
    Lock(PackageLockError),
}

impl fmt::Display for PackageChangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_diagnostics(formatter, &diagnostics::write_bounded_diagnostics)
    }
}

impl PackageChangeError {
    pub fn fmt_with_diagnostics(
        &self,
        formatter: &mut fmt::Formatter<'_>,
        render: &diagnostics::WriteDiagnosticList<'_>,
    ) -> fmt::Result {
        match self {
            Self::CompilerIdentity(error) => write!(
                formatter,
                "package review compiler identity failed: {error}"
            ),
            Self::Compilation(error) => {
                formatter.write_str("package candidate checking failed: ")?;
                error.fmt_with_diagnostics(formatter, render)
            }
            Self::Comparison(error) => {
                write!(formatter, "package candidate comparison failed: {error}")
            }
            Self::SourceSubject(error) => {
                write!(formatter, "package candidate source record failed: {error}")
            }
            Self::Decisions(error) => fmt::Display::fmt(error, formatter),
            Self::RejectedChanges => formatter.write_str(
                "project decisions reject the package candidate; accepted files are unchanged",
            ),
            Self::Lock(error) => {
                write!(formatter, "cannot construct proposed package lock: {error}")
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
            Self::RejectedChanges | Self::CompilerIdentity(_) => None,
        }
    }
}

impl From<CompileResolvedPackageReviewsError> for PackageChangeError {
    fn from(error: CompileResolvedPackageReviewsError) -> Self {
        Self::Compilation(error)
    }
}
