// We hold transport-erased custody for one declared immutable package source.
// Persistent sources retain a sealed physical root, while an ordinary
// compilation root retains its complete acquisition inventory so transport
// erasure never drops the captured bytes or forces loading or `Build` to reopen
// a changed checkout.

use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::{
    DependencyProjections, DependencyPurpose, DependencyUse,
};
use crate::resolution::source::{
    PackageSourceMaterialization, PackageSourceNavigation, PackageSourceSelectionEvidence,
};
use source_acquisition::ImmutableSourceResolution;
use source_acquisition::LocalSourceLimits;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(crate) struct PackageSourceCustody {
    key: PackageKey,
    role: BuildDeclarationKind,
    resolution: ImmutableSourceResolution,
    materialization: PackageSourceMaterialization,
    pub(crate) snapshot_root: PathBuf,
    captured_source: Option<Arc<source_acquisition::capture::CapturedSourceTree>>,
    preparing_source: Option<package_compilation::CapturedLocalSource>,
    pub(crate) build_entries: package_compilation::PackageBuildEntries,
    navigation: PackageSourceNavigation,
    selection_evidence: PackageSourceSelectionEvidence,
    source_limits: LocalSourceLimits,
    dependency_projections: DependencyProjections,
}

impl PartialEq for PackageSourceCustody {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.role == other.role
            && self.resolution == other.resolution
            && self.materialization == other.materialization
            && self.snapshot_root == other.snapshot_root
            && self.captured_source == other.captured_source
            && self.preparing_source == other.preparing_source
            && self.build_entries == other.build_entries
            && self.navigation == other.navigation
            && self.selection_evidence == other.selection_evidence
            && self.dependency_projections == other.dependency_projections
    }
}

impl Eq for PackageSourceCustody {}

impl PackageSourceCustody {
    pub(crate) fn from_resolved_parts<D>(
        key: PackageKey,
        role: BuildDeclarationKind,
        resolution: ImmutableSourceResolution,
        materialization: PackageSourceMaterialization,
        snapshot_root: PathBuf,
        navigation: PackageSourceNavigation,
        selection_evidence: PackageSourceSelectionEvidence,
        source_limits: LocalSourceLimits,
        projected_dependencies: D,
    ) -> Self
    where
        D: Into<DependencyProjections>,
    {
        debug_assert!(resolution.matches_lineage(key.source_lineage()));
        Self {
            key,
            role,
            resolution,
            materialization,
            snapshot_root,
            captured_source: None,
            preparing_source: None,
            build_entries: Default::default(),
            navigation,
            selection_evidence,
            source_limits,
            dependency_projections: projected_dependencies.into(),
        }
    }

    pub(crate) fn with_captured_source(
        mut self,
        tree: Arc<source_acquisition::capture::CapturedSourceTree>,
    ) -> Self {
        self.captured_source = Some(tree);
        self
    }

    pub(super) fn with_preparing_source(
        mut self,
        source: Option<package_compilation::CapturedLocalSource>,
    ) -> Self {
        self.preparing_source = source;
        self
    }

    pub(super) fn matches_requested_content(
        &self,
        source: &package_compilation::CapturedLocalSource,
    ) -> bool {
        match &self.preparing_source {
            Some(original) => original == source,
            None => {
                self.materialization()
                    == &PackageSourceMaterialization::from_local(source.normalized())
            }
        }
    }

    /// Authenticate the original acquisition even when the compiler consumes a generated directory.
    pub(super) fn matches_acquisition(
        &self,
        source: &package_compilation::CapturedLocalSource,
    ) -> bool {
        match &self.preparing_source {
            Some(original) => original == source,
            None => {
                self.snapshot_root() == source.canonical_live_root()
                    && self.captured_source().map(Arc::as_ref) == Some(source.tree().as_ref())
            }
        }
    }

    pub(crate) fn preparing_source(&self) -> Option<&package_compilation::CapturedLocalSource> {
        self.preparing_source.as_ref()
    }

    pub fn captured_source(&self) -> Option<&Arc<source_acquisition::capture::CapturedSourceTree>> {
        self.captured_source.as_ref()
    }

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

    pub(super) fn with_selection_evidence(
        mut self,
        selection: PackageSourceSelectionEvidence,
    ) -> Self {
        self.selection_evidence = selection;
        self
    }

    pub const fn selection_evidence(&self) -> &PackageSourceSelectionEvidence {
        &self.selection_evidence
    }

    pub fn source_limits(&self) -> LocalSourceLimits {
        self.source_limits
    }

    pub fn dependency_requests(&self, purpose: DependencyPurpose) -> &[DependencyUse] {
        self.dependency_projections.requests(purpose)
    }

    pub const fn dependency_projections(&self) -> &DependencyProjections {
        &self.dependency_projections
    }

    pub(crate) fn semantically_equivalent(&self, other: &Self) -> bool {
        self.key == other.key
            && self.role == other.role
            && self.resolution == other.resolution
            && self.materialization == other.materialization
            && self.navigation == other.navigation
            && self.selection_evidence == other.selection_evidence
            && self.dependency_projections == other.dependency_projections
    }
}
