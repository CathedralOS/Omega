//! Resolve immutable source custody into one declared package source.
//!
//! The entry points are grouped by source lineage. Shared declaration
//! projection, errors, and resolved custody live behind this narrow facade.

/// The declared role of one resolved root: a dependency source is
/// package-only, while a compilation project root may also declare an
/// application (`Build::application`). The role selects root acquisition and
/// declaration binding, so every resolution entry point carries it as one
/// named input instead of splitting into a package/project entry-point family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceRootRole {
    /// A dependency source: the declaration must be a package.
    Package,
    /// A compilation root: the declaration may be a package or application.
    Project,
}

impl SourceRootRole {
    /// Whether an application declaration is accepted at this root.
    pub(crate) const fn allows_application_root(self) -> bool {
        matches!(self, Self::Project)
    }
}

mod acquisition;
mod workspace_acquisition;
pub use acquisition::{AcquiredBuildSource, BuildSourceAcquisitionError, BuildSourceAcquisitions};
pub use workspace_acquisition::{AcquiredWorkspaceBuild, AcquiredWorkspaceMembers};
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
pub(crate) fn package_entry(
    root: &std::path::Path,
    captured: Option<&source::CapturedSourceTree>,
) -> std::path::PathBuf {
    let has_main = match captured {
        Some(tree) => tree.is_file(std::path::Path::new("main.omg")),
        None => root.join("main.omg").is_file(),
    };
    root.join(if has_main { "main.omg" } else { "build.omg" })
}

mod materialization;
mod navigation;
mod projection;
mod resolved;
mod selection;
pub(crate) mod workspace_path;

pub(crate) use custody::PackageSourceCustody;
pub(crate) use custody::{CHECKED_SOURCE_CACHE_LANE, COMPILED_REVIEW_CACHE_LANE};
pub(crate) use error::ResolvePackageSourceError;
pub(crate) use git::GitPackageSourceRequest;
pub(crate) use git::resolve_selected_git_source_at_revision_in_lanes;
pub(crate) use git::workspace::{GitWorkspaceSelectionDeclarations, GitWorkspaceSelectionEvidence};
pub(crate) use local::recover_cached_external_local_source;
pub(crate) use local::resolve_external_local_project_source;
pub(crate) use materialization::PackageSourceMaterialization;
pub(crate) use navigation::PackageSourceNavigation;
pub(crate) use resolved::ResolvedPackageSource;
pub(crate) use selection::{PackageSourceSelectionEvidence, PackageSourceSelectionEvidenceError};

/// The accessor surface every resolved-package-source record shares:
/// custody and resolved shapes carry identically-named projection fields and
/// expose the same dependency-request delegation.
macro_rules! package_source_surface_accessors {
    () => {
        pub fn key(&self) -> &PackageKey {
            &self.key
        }

        pub const fn role(&self) -> BuildDeclarationKind {
            self.role
        }

        pub fn resolution(&self) -> &ImmutableSourceResolution {
            &self.resolution
        }

        pub const fn materialization(&self) -> &PackageSourceMaterialization {
            &self.materialization
        }

        pub fn snapshot_root(&self) -> &std::path::Path {
            &self.snapshot_root
        }

        pub const fn navigation(&self) -> &PackageSourceNavigation {
            &self.navigation
        }

        pub const fn selection_evidence(&self) -> &PackageSourceSelectionEvidence {
            &self.selection_evidence
        }

        pub fn source_limits(&self) -> LocalSourceLimits {
            self.source_limits
        }

        /// Authored requests for one purpose scope, in requester-local order.
        pub fn dependency_requests(
            &self,
            purpose: DependencyPurpose,
        ) -> &[DependencySourceRequest] {
            self.dependency_projections.requests(purpose)
        }

        pub const fn dependency_projections(&self) -> &DependencyProjections {
            &self.dependency_projections
        }
    };
}
pub(crate) use package_source_surface_accessors;
