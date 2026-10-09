use super::super::reconcile::PackageSourceClosureResolutionError;
use crate::declarations::PackageKey;
use crate::resolution::source::ResolvePackageSourceError;
use build_declarations::WorkspaceMemberPath;
use package_source::GitSourceRequestError;
use package_source::WorkspaceLineageIdentity;
use std::fmt;

#[derive(Debug)]
pub enum ResolveWorkspacePackageClosureError {
    Root(ResolvePackageSourceError),
    RootRequestMismatch,
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

#[derive(Debug)]
pub enum ResolveExternalLocalPackageClosureError {
    Root(ResolvePackageSourceError),
    RootRequestMismatch,
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

#[derive(Debug)]
pub enum ResolveGitPackageClosureError {
    Root(ResolvePackageSourceError),
    RootRequestMismatch,
    RootWorkspace(ResolveDependencySourceError),
    Closure(PackageSourceClosureResolutionError<ResolveDependencySourceError>),
}

impl fmt::Display for ResolveGitPackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot resolve root package: {error}"),
            Self::RootRequestMismatch => formatter
                .write_str("resolved root Git source does not match its exact validated request"),
            Self::RootWorkspace(error) => {
                write!(formatter, "cannot register root Git workspace: {error}")
            }
            Self::Closure(error) => write!(formatter, "cannot resolve package closure: {error}"),
        }
    }
}

impl std::error::Error for ResolveGitPackageClosureError {}

impl fmt::Display for ResolveExternalLocalPackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot resolve root package: {error}"),
            Self::RootRequestMismatch => {
                formatter.write_str("resolved external-local root does not match its exact request")
            }
            Self::Closure(error) => write!(formatter, "cannot resolve package closure: {error}"),
        }
    }
}

impl std::error::Error for ResolveExternalLocalPackageClosureError {}

impl fmt::Display for ResolveWorkspacePackageClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Root(error) => write!(formatter, "cannot resolve root package: {error}"),
            Self::RootRequestMismatch => formatter
                .write_str("resolved workspace root does not match its exact member request"),
            Self::Closure(error) => write!(formatter, "cannot resolve package closure: {error}"),
        }
    }
}

impl std::error::Error for ResolveWorkspacePackageClosureError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveDependencySourceError {
    InvalidPath {
        location: String,
        reason: String,
    },
    UnknownWorkspace {
        package: PackageKey,
    },
    ConflictingWorkspaceRoot {
        identity: WorkspaceLineageIdentity,
    },
    UnknownExternalRoot {
        package: PackageKey,
    },
    ConflictingExternalRoot {
        package: PackageKey,
    },
    MissingExternalSourceContext,
    InvalidGitRequest(GitSourceRequestError),
    UndeclaredGitWorkspaceMember {
        package: PackageKey,
        member_path: WorkspaceMemberPath,
    },
    Source(ResolvePackageSourceError),
}

impl fmt::Display for ResolveDependencySourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath { location, reason } => {
                write!(formatter, "invalid path dependency `{location}`: {reason}")
            }
            Self::UnknownWorkspace { package } => write!(
                formatter,
                "package `{}` has no registered immutable workspace root",
                package.name().as_str()
            ),
            Self::ConflictingWorkspaceRoot { .. } => formatter
                .write_str("one workspace lineage resolved to conflicting immutable source roots"),
            Self::UnknownExternalRoot { package } => write!(
                formatter,
                "external-local package `{}` has no registered live source root",
                package.name().as_str()
            ),
            Self::ConflictingExternalRoot { package } => write!(
                formatter,
                "external-local package `{}` resolved to conflicting live source roots",
                package.name().as_str()
            ),
            Self::MissingExternalSourceContext => formatter.write_str(
                "an external-local dependency requires an explicit consuming source context",
            ),
            Self::InvalidGitRequest(error) => error.fmt(formatter),
            Self::UndeclaredGitWorkspaceMember {
                package,
                member_path,
            } => write!(
                formatter,
                "Git package `{}` requested undeclared workspace member `{}`",
                package.name().as_str(),
                member_path.as_str()
            ),
            Self::Source(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ResolveDependencySourceError {}

impl From<ResolvePackageSourceError> for ResolveDependencySourceError {
    fn from(error: ResolvePackageSourceError) -> Self {
        Self::Source(error)
    }
}

impl From<GitSourceRequestError> for ResolveDependencySourceError {
    fn from(error: GitSourceRequestError) -> Self {
        Self::InvalidGitRequest(error)
    }
}
