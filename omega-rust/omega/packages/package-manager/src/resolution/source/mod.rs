/*
    We resolve immutable source custody into one declared package source,
    binding executed declarations and historical recovery behind this facade.
    Package entry selection (`package_entry`) inspects the acquired in-memory
    source tree when available, falling back to the physical root only for
    legacy persisted-custody consumers without an in-memory capture.

    We also define the core value and error types shared across source custody:
    `ResolvePackageSourceError` for source resolution and Git-pin retention
    failures, `PackageSourceMaterialization` for the exact package subtree
    exposed to compilation (which may differ from the repository root when
    multiple workspace members share one commit tree), and
    `PackageSourceNavigation` (`Root` or `Member`) for operational navigation
    from an authenticated acquisition root to a package root without embedding
    navigation into `PackageKey` or source lineage identity.
*/

/*
    Package source custody and acquisition belong to the package manager's
    source resolution boundary (`resolution/source/mod.rs`).
*/

mod acquisition;
mod build_resolution;
mod custody;
pub(crate) mod git;
mod local;
mod selection;
mod workspace_acquisition;
pub(crate) mod workspace_path;

pub(crate) use acquisition::SourceAcquisitionOrigin;
pub use acquisition::{
    AcquiredBuildSource, AcquiredSourceSelection, BuildSourceAcquisitionError,
    BuildSourceAcquisitions,
};
pub use build_resolution::{
    AcquiredBuildDependency, BuildSourceAcquisition, DeclaredBuildSource, ExecutedSourceResolution,
    PendingBuildSource, ResolvedBuildSources,
};
pub(crate) use custody::PackageSourceCustody;
pub(crate) use custody::{CHECKED_SOURCE_CACHE_LANE, COMPILED_REVIEW_CACHE_LANE};
pub(crate) use git::GitPackageSourceRequest;
pub(crate) use git::workspace::GitWorkspaceSelectionEvidence;
pub(crate) use git::{RecordedGitPackage, recover_recorded_git_source};
pub(crate) use local::{external_local_dependency_root, recover_cached_external_local_source};
pub(crate) use selection::PackageSourceSelectionEvidence;
pub use selection::PackageSourceSelectionEvidenceError;
pub use workspace_acquisition::{
    AcquiredWorkspaceBuild, AcquiredWorkspaceMembers, WorkspaceMemberObservation,
};

use package_source::{
    ResolvedLocalSource, SourceContentDigest, SourceRelativePath, SourceResolveError,
};
use std::fmt;
use std::path::{Path, PathBuf};

pub(crate) fn package_entry(root: &Path, captured: Option<&source::CapturedSourceTree>) -> PathBuf {
    let has_main = match captured {
        Some(tree) => tree.is_file(Path::new("main.omg")),
        None => root.join("main.omg").is_file(),
    };
    root.join(if has_main { "main.omg" } else { "build.omg" })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolvePackageSourceError {
    Source(SourceResolveError),
    RecordedGitPin {
        locator: String,
        revision: String,
        message: String,
    },
}

impl fmt::Display for ResolvePackageSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(error) => write!(formatter, "cannot resolve package source: {error}"),
            Self::RecordedGitPin {
                locator,
                revision,
                message,
            } => write!(
                formatter,
                "cannot retain Git pin for `{locator}` at `{revision}`: {message}"
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageSourceMaterialization {
    content: SourceContentDigest,
    file_count: usize,
    byte_count: u64,
}

impl PackageSourceMaterialization {
    pub(crate) fn from_local(source: &ResolvedLocalSource) -> Self {
        Self {
            content: SourceContentDigest::derive(source.content_identity.as_bytes()),
            file_count: source.file_count,
            byte_count: source.byte_count,
        }
    }

    pub const fn content(&self) -> &SourceContentDigest {
        &self.content
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum PackageSourceNavigation {
    Root,
    Member(SourceRelativePath),
}

impl PackageSourceNavigation {}
