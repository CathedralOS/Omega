//! Resolve immutable source custody into one declared package source.
//!
//! The entry points are grouped by source lineage. Executed declarations and
//! historical recovery bind authenticated source behind this narrow facade.

mod acquisition;
mod workspace_acquisition;
pub(crate) use acquisition::SourceAcquisitionOrigin;
pub use acquisition::{
    AcquiredBuildSource, AcquiredSourceSelection, BuildSourceAcquisitionError,
    BuildSourceAcquisitions,
};
pub use workspace_acquisition::{
    AcquiredWorkspaceBuild, AcquiredWorkspaceMembers, WorkspaceMemberObservation,
};
mod build_resolution;
mod custody;
pub use build_resolution::{
    AcquiredBuildDependency, BuildSourceAcquisition, DeclaredBuildSource, ExecutedSourceResolution,
    PendingBuildSource, ResolvedBuildSources,
};
mod error;
pub(crate) mod git;
mod local;
pub(crate) use local::external_local_dependency_root;

/// Package entry selection observes acquired bytes when available. The live
/// path is only for legacy persisted-custody consumers without an in-memory
/// capture; application scheduling does not decide or probe source filenames.
pub(crate) fn package_entry(root: &Path, captured: Option<&source::CapturedSourceTree>) -> PathBuf {
    let has_main = match captured {
        Some(tree) => tree.is_file(Path::new("main.omg")),
        None => root.join("main.omg").is_file(),
    };
    root.join(if has_main { "main.omg" } else { "build.omg" })
}

mod materialization;
mod navigation;
mod selection;
pub(crate) mod workspace_path;

pub(crate) use custody::PackageSourceCustody;
pub(crate) use custody::{CHECKED_SOURCE_CACHE_LANE, COMPILED_REVIEW_CACHE_LANE};
pub(crate) use error::ResolvePackageSourceError;
pub(crate) use git::GitPackageSourceRequest;
pub(crate) use git::workspace::GitWorkspaceSelectionEvidence;
pub(crate) use git::{RecordedGitPackage, recover_recorded_git_source};
pub(crate) use local::recover_cached_external_local_source;
pub(crate) use materialization::PackageSourceMaterialization;
pub(crate) use navigation::PackageSourceNavigation;
pub(crate) use selection::PackageSourceSelectionEvidence;
pub use selection::PackageSourceSelectionEvidenceError;
use std::path::{Path, PathBuf};

/// Offer private compiler storage from the ordinary retained source resolver.
/// The lane conveys storage custody only; source and package admission remain
/// per compilation. Embedders can preserve their explicitly supplied resolver.
pub fn retain_compiler_source_storage(
    resolver: Option<&package_source::SourceResolver>,
) -> Result<package_source::storage::RetainedStorageLane, String> {
    let owned;
    let resolver = match resolver {
        Some(resolver) => resolver,
        None => {
            owned = package_source::SourceResolver::for_current_user(
                package_source::PrimaryGitChoices::default(),
            )
            .map_err(|reason| reason.to_string())?;
            &owned
        }
    };
    custody::retain_compiled_review_storage(resolver).map_err(|reason| reason.to_string())
}
