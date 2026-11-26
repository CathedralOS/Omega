// Evaluated project requests produced by `Build` execution before package
// resolution or final Build settlement. These values request work without
// granting acquisition, execution, or publication authority, and keep Product
// and Build dependency alias scopes separate.

use crate::{DependencyPurpose, ProjectName};

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub enum BuildRequest {
    ArtifactOnly,
    Dependency {
        purpose: DependencyPurpose,
        max_reach: semantic_vocabulary::dependency_reach::DependencyReach,
        alias: Option<ProjectName>,
        source: BuildDependencySource,
    },
}

/// A successor chooses a file in the frozen source view by its package-relative filename.
#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct BuildEntryPath(String);

impl BuildEntryPath {
    pub fn validate(path: &str) -> Result<(), &'static str> {
        if source_selection::source_snapshot_path_is_canonical(path.as_bytes(), false)
            && path.rsplit('/').next().is_some_and(|file| {
                file.strip_suffix(".omg")
                    .is_some_and(|stem| !stem.is_empty())
            })
        {
            Ok(())
        } else {
            Err("Build successor entry requires a canonical package-relative .omg filename")
        }
    }

    pub fn parse(path: String) -> Result<Self, &'static str> {
        Self::validate(&path)?;
        Ok(Self(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub enum BuildDependencySource {
    Path(String),
    Bundled(ProjectName),
    Git {
        repository: String,
        revision: String,
        package: Option<ProjectName>, // None selects the repository root; Some selects a named workspace member.
    },
}
