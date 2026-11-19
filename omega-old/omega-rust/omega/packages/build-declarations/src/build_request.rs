// Evaluated project requests produced by `Build` execution before package
// resolution or final Build settlement. These values request work without
// granting acquisition, execution, or publication authority, and keep Product
// and Build dependency alias scopes separate.

use crate::{DependencyPurpose, ProjectName, WorkspaceMemberPath};

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum BuildRequest {
    Package(ProjectName),
    Application(ProjectName),
    Member(WorkspaceMemberPath),
    ArtifactOnly,
    Dependency {
        purpose: DependencyPurpose,
        alias: Option<ProjectName>,
        source: BuildDependencySource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub enum BuildDependencySource {
    Path(String),
    Git {
        repository: String,
        revision: String,
        package: Option<ProjectName>, // None selects the repository root; Some selects a named workspace member.
    },
}
