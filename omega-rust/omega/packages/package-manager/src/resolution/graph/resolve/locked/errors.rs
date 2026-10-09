use crate::declarations::PackageKey;
use crate::resolution::graph::{
    CanonicalSourceClosureSubjectError, PackageSourceClosureResolutionError,
    ResolveDependencySourceError,
};
use crate::resolution::source::ResolvePackageSourceError;
use std::fmt;

#[derive(Debug)]
pub enum ResolveLockedPackageClosureError {
    RootRequestMismatch,
    SourceMismatch {
        package: PackageKey,
        detail: &'static str,
    },
    LimitExceeded,
    Source(ResolvePackageSourceError),
    Dependency(ResolveDependencySourceError),
    Closure(Box<PackageSourceClosureResolutionError<Self>>),
    Subject(CanonicalSourceClosureSubjectError),
}

impl ResolveLockedPackageClosureError {
    pub(super) fn mismatch(package: &PackageKey, detail: &'static str) -> Self {
        Self::SourceMismatch {
            package: package.clone(),
            detail,
        }
    }
}

impl From<ResolvePackageSourceError> for ResolveLockedPackageClosureError {
    fn from(error: ResolvePackageSourceError) -> Self {
        Self::Source(error)
    }
}
impl From<ResolveDependencySourceError> for ResolveLockedPackageClosureError {
    fn from(error: ResolveDependencySourceError) -> Self {
        Self::Dependency(error)
    }
}
impl From<CanonicalSourceClosureSubjectError> for ResolveLockedPackageClosureError {
    fn from(error: CanonicalSourceClosureSubjectError) -> Self {
        Self::Subject(error)
    }
}
impl fmt::Display for ResolveLockedPackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RootRequestMismatch => {
                formatter.write_str("locked root request differs from the exact caller request")
            }
            Self::SourceMismatch { package, detail } => {
                write!(formatter, "locked source {package:?}: {detail}")
            }
            Self::LimitExceeded => {
                formatter.write_str("locked source closure exceeds the requested recovery limits")
            }
            Self::Source(error) => error.fmt(formatter),
            Self::Dependency(error) => error.fmt(formatter),
            Self::Closure(error) => error.fmt(formatter),
            Self::Subject(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for ResolveLockedPackageClosureError {}
