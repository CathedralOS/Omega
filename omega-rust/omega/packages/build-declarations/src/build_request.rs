//! Evaluated project requests, before package resolution or final Build join.
//!
//! These values request work; they grant no acquisition, execution or publication
//! authority. Product and Build dependencies retain separate alias scopes.

use crate::{DependencyPurpose, ProjectName, WorkspaceMemberPath};

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildDependencySource {
    Path(String),
    Git {
        repository: String,
        revision: String,
        /// None selects the repository root; Some selects its named member.
        package: Option<ProjectName>,
    },
}
