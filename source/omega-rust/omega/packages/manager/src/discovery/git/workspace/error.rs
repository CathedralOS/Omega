use crate::declarations::{BuildDeclarationError, BuildDeclarationKind, DependencyProjectionError};
use omega_build_declarations::{ProjectName, WorkspaceMemberPath};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitWorkspaceSelectionLimit {
    DeclarationBytes,
    TotalDeclarationBytes,
    WorkspaceMembers,
}

impl GitWorkspaceSelectionLimit {
    const fn description(self) -> &'static str {
        match self {
            Self::DeclarationBytes => "per-declaration byte",
            Self::TotalDeclarationBytes => "aggregate declaration byte",
            Self::WorkspaceMembers => "workspace member",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitWorkspaceSelectionError {
    ResourceLimit {
        limit: GitWorkspaceSelectionLimit,
        maximum: usize,
        observed: usize,
    },
    NonUtf8Declaration {
        repository_path: String,
    },
    MalformedDeclaration {
        repository_path: String,
        error: BuildDeclarationError,
    },
    WrongRole {
        repository_path: String,
        expected: BuildDeclarationKind,
        found: BuildDeclarationKind,
    },
    StaticDependencyProjection {
        member_path: WorkspaceMemberPath,
        error: DependencyProjectionError,
    },
    DuplicateMemberBuild {
        member_path: WorkspaceMemberPath,
    },
    MissingMemberBuild {
        member_path: WorkspaceMemberPath,
    },
    ExtraMemberBuild {
        member_path: WorkspaceMemberPath,
    },
    PackageMissing {
        package_name: ProjectName,
    },
    PackageDuplicate {
        package_name: ProjectName,
        member_paths: Vec<WorkspaceMemberPath>,
    },
    DeclarationEvidenceChanged,
}

impl fmt::Display for GitWorkspaceSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ResourceLimit {
                limit,
                maximum,
                observed,
            } => write!(
                formatter,
                "Git workspace selection exceeded its {} ceiling of {maximum} (observed {observed})",
                limit.description()
            ),
            Self::NonUtf8Declaration { repository_path } => write!(
                formatter,
                "authenticated declaration `{repository_path}` is not UTF-8 Omega source"
            ),
            Self::MalformedDeclaration {
                repository_path,
                error,
            } => write!(
                formatter,
                "cannot project authenticated declaration `{repository_path}`: {error}"
            ),
            Self::WrongRole {
                repository_path,
                expected,
                found,
            } => write!(
                formatter,
                "authenticated declaration `{repository_path}` must declare a {}, found {}",
                expected.as_str(),
                found.as_str()
            ),
            Self::StaticDependencyProjection { member_path, error } => write!(
                formatter,
                "cannot statically project dependencies for workspace member `{}`: {error}",
                member_path.as_str()
            ),
            Self::DuplicateMemberBuild { member_path } => write!(
                formatter,
                "authenticated member declaration `{}/build.omg` was provided more than once",
                member_path.as_str()
            ),
            Self::MissingMemberBuild { member_path } => write!(
                formatter,
                "workspace member `{}` has no provided authenticated build.omg",
                member_path.as_str()
            ),
            Self::ExtraMemberBuild { member_path } => write!(
                formatter,
                "authenticated build.omg was provided for undeclared workspace path `{}`",
                member_path.as_str()
            ),
            Self::PackageMissing { package_name } => write!(
                formatter,
                "Git workspace declares no member package named `{}`",
                package_name.as_str()
            ),
            Self::PackageDuplicate {
                package_name,
                member_paths,
            } => write!(
                formatter,
                "Git workspace declares package `{}` at multiple member paths: {}",
                package_name.as_str(),
                member_paths
                    .iter()
                    .map(WorkspaceMemberPath::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::DeclarationEvidenceChanged => formatter.write_str(
                "authenticated Git workspace declaration evidence changed during replay",
            ),
        }
    }
}

impl std::error::Error for GitWorkspaceSelectionError {}
