use crate::declarations::dependencies::read::DependencyProjectionError;
use crate::declarations::project::BuildDeclarationError;
use crate::discovery::git::workspace::GitWorkspaceSelectionError;
use omega_package_source::IdentityError;
use omega_package_source::SourceRelativePath;
use omega_package_source::SourceResolveError;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvePackageSourceError {
    Source(SourceResolveError),
    Declaration(BuildDeclarationError),
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
    GitWorkspaceSelection(GitWorkspaceSelectionError),
    GitWorkspaceMemberNavigation {
        member_path: SourceRelativePath,
        message: String,
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
            Self::GitWorkspaceSelection(error) => {
                write!(
                    formatter,
                    "cannot select package from Git workspace: {error}"
                )
            }
            Self::GitWorkspaceMemberNavigation {
                member_path,
                message,
            } => write!(
                formatter,
                "cannot navigate declared Git workspace member `{}`: {message}",
                member_path.as_str()
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

impl From<BuildDeclarationError> for ResolvePackageSourceError {
    fn from(error: BuildDeclarationError) -> Self {
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

impl From<GitWorkspaceSelectionError> for ResolvePackageSourceError {
    fn from(error: GitWorkspaceSelectionError) -> Self {
        Self::GitWorkspaceSelection(error)
    }
}
