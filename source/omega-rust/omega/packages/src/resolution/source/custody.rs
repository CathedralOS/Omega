//! Transport-erased custody for one declared package snapshot.

use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::{DependencySourceRequest, ProjectedDependencies};
use crate::resolution::source::{
    PackageSourceMaterialization, PackageSourceNavigation, PackageSourceSelectionEvidence,
};
use omega_package_source::ImmutableSourceResolution;
use omega_package_source::LocalSourceLimits;
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
    projected_dependencies: ProjectedDependencies,
}

impl PartialEq for PackageSourceCustody {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.role == other.role
            && self.resolution == other.resolution
            && self.materialization == other.materialization
            && self.snapshot_root == other.snapshot_root
            && self.navigation == other.navigation
            && self.selection_evidence == other.selection_evidence
            && self.projected_dependencies == other.projected_dependencies
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
        D: Into<ProjectedDependencies>,
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
            projected_dependencies: projected_dependencies.into(),
        }
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

    pub fn dependency_requests(&self) -> &[DependencySourceRequest] {
        self.projected_dependencies.authored_dependencies()
    }

    pub const fn projected_dependencies(&self) -> &ProjectedDependencies {
        &self.projected_dependencies
    }

    pub(crate) fn semantically_equivalent(&self, other: &Self) -> bool {
        self.key == other.key
            && self.role == other.role
            && self.resolution == other.resolution
            && self.materialization == other.materialization
            && self.navigation == other.navigation
            && self.selection_evidence == other.selection_evidence
            && self.projected_dependencies == other.projected_dependencies
    }
}
