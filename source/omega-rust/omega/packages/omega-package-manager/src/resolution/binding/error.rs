use crate::manifest::dependencies::read::DependencyProjectionError;
use crate::manifest::roles::PackageDeclarationError;
use omega_package_source::IdentityError;
use omega_package_source::SourceResolveError;
use omega_package_source::{PackageName, WorkspaceMemberPath};
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
    NamedGitSelectionRequiresWorkspace {
        found: crate::manifest::BuildDeclarationKind,
    },
    GitWorkspaceMemberInvalid {
        member_path: WorkspaceMemberPath,
        error: Box<ResolvePackageSourceError>,
    },
    GitWorkspaceMemberNavigation {
        member_path: WorkspaceMemberPath,
        message: String,
    },
    NamedGitPackageMissing {
        package: PackageName,
    },
    NamedGitPackageDuplicate {
        package: PackageName,
        member_paths: Vec<WorkspaceMemberPath>,
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
            Self::NamedGitSelectionRequiresWorkspace { found } => write!(
                formatter,
                "named Git package selection requires a workspace root, found {}",
                found.as_str()
            ),
            Self::GitWorkspaceMemberInvalid { member_path, error } => write!(
                formatter,
                "declared Git workspace member `{}` is invalid: {error}",
                member_path.as_str()
            ),
            Self::GitWorkspaceMemberNavigation {
                member_path,
                message,
            } => write!(
                formatter,
                "cannot navigate declared Git workspace member `{}`: {message}",
                member_path.as_str()
            ),
            Self::NamedGitPackageMissing { package } => write!(
                formatter,
                "Git workspace declares no member package named `{}`",
                package.as_str()
            ),
            Self::NamedGitPackageDuplicate {
                package,
                member_paths,
            } => write!(
                formatter,
                "Git workspace declares package `{}` at multiple member paths: {}",
                package.as_str(),
                member_paths
                    .iter()
                    .map(|path| path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
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
