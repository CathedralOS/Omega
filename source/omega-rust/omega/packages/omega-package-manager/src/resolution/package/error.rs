use crate::manifest::declaration::PackageDeclarationError;
use crate::manifest::dependency_projection::DependencyProjectionError;
use crate::resolution::SourceResolveError;
use crate::resolution::identity::IdentityError;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvePackageSourceError {
    Source(SourceResolveError),
    Declaration(PackageDeclarationError),
    DependencyProjection(DependencyProjectionError),
    Identity(IdentityError),
    WorkspacePath {
        path: PathBuf,
        message: String,
    },
    WorkspaceMemberEscapesRoot {
        workspace_root: PathBuf,
        member_root: PathBuf,
    },
    WorkspaceMemberIsRoot {
        workspace_root: PathBuf,
    },
}

impl fmt::Display for ResolvePackageSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "cannot resolve package source: {error}"),
            Self::Declaration(error) => {
                write!(formatter, "cannot establish package declaration: {error}")
            }
            Self::DependencyProjection(error) => {
                write!(formatter, "cannot project package dependencies: {error}")
            }
            Self::Identity(error) => {
                write!(formatter, "cannot establish package identity: {error}")
            }
            Self::WorkspacePath { path, message } => write!(
                formatter,
                "cannot establish canonical workspace path `{}`: {message}",
                path.display()
            ),
            Self::WorkspaceMemberEscapesRoot {
                workspace_root,
                member_root,
            } => write!(
                formatter,
                "workspace member `{}` resolves outside workspace root `{}`",
                member_root.display(),
                workspace_root.display()
            ),
            Self::WorkspaceMemberIsRoot { workspace_root } => write!(
                formatter,
                "workspace member resolves to the whole workspace root `{}`",
                workspace_root.display()
            ),
        }
    }
}

impl std::error::Error for ResolvePackageSourceError {}

impl From<SourceResolveError> for ResolvePackageSourceError {
    fn from(error: SourceResolveError) -> Self {
        Self::Source(error)
    }
}

impl From<PackageDeclarationError> for ResolvePackageSourceError {
    fn from(error: PackageDeclarationError) -> Self {
        Self::Declaration(error)
    }
}

impl From<DependencyProjectionError> for ResolvePackageSourceError {
    fn from(error: DependencyProjectionError) -> Self {
        Self::DependencyProjection(error)
    }
}

impl From<IdentityError> for ResolvePackageSourceError {
    fn from(error: IdentityError) -> Self {
        Self::Identity(error)
    }
}
