//! Transport-erased custody for one declared package snapshot.

use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::{
    DependencyProjections, DependencyPurpose, DependencySourceRequest,
};
use crate::resolution::source::{
    PackageSourceMaterialization, PackageSourceNavigation, PackageSourceSelectionEvidence,
};
use package_source::ImmutableSourceResolution;
use package_source::LocalSourceLimits;
use std::path::{Path, PathBuf};

/// Immutable package source after acquisition, declaration extraction, and
/// dependency projection have all succeeded.
#[derive(Debug, Clone)]
pub struct PackageSourceCustody {
    key: PackageKey,
    role: BuildDeclarationKind,
    resolution: ImmutableSourceResolution,
    materialization: PackageSourceMaterialization,
    pub(crate) snapshot_root: PathBuf,
    navigation: PackageSourceNavigation,
    selection_evidence: PackageSourceSelectionEvidence,
    source_limits: LocalSourceLimits,
    dependency_projections: DependencyProjections,
    /// Retained resolver lane directory that may persist this custody's
    /// canonical source index. It is a storage location, not source
    /// identity, so equality and semantic equivalence ignore it.
    checked_source_cache_dir: Option<PathBuf>,
}

/// Name of the lane child holding persistent checked-source indexes beneath
/// a retained resolver storage lane.
pub(crate) const CHECKED_SOURCE_CACHE_LANE: &str = "checked-source-cache";

impl PartialEq for PackageSourceCustody {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.role == other.role
            && self.resolution == other.resolution
            && self.materialization == other.materialization
            && self.snapshot_root == other.snapshot_root
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
            navigation,
            selection_evidence,
            source_limits,
            dependency_projections: projected_dependencies.into(),
            checked_source_cache_dir: None,
        }
    }

    /// Attach the retained lane directory offered to the checked source
    /// cache during compiler-input preparation.
    pub(crate) fn with_checked_source_cache_dir(mut self, directory: PathBuf) -> Self {
        self.checked_source_cache_dir = Some(directory);
        self
    }

    /// Retained directory offered to the checked source cache, when
    /// resolution supplied one.
    pub fn checked_source_cache_dir(&self) -> Option<&Path> {
        self.checked_source_cache_dir.as_deref()
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

    pub fn snapshot_root(&self) -> &Path {
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
    pub fn dependency_requests(&self, purpose: DependencyPurpose) -> &[DependencySourceRequest] {
        self.dependency_projections.requests(purpose)
    }

    /// Authored product-purpose requests, in requester-local order.
    pub fn product_dependency_requests(&self) -> &[DependencySourceRequest] {
        self.dependency_requests(DependencyPurpose::Product)
    }

    /// Authored build-purpose requests, in requester-local order.
    pub fn build_dependency_requests(&self) -> &[DependencySourceRequest] {
        self.dependency_requests(DependencyPurpose::Build)
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
