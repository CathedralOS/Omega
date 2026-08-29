use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::resolution::binding::PackageSourceCustody;
use crate::resolution::binding::PackageSourceMaterialization;
use crate::resolution::binding::PackageSourceNavigation;
use crate::resolution::binding::PackageSourceSelectionEvidence;
use omega_package_source::LocalSourceLimits;
use omega_package_source::{ImmutableSourceResolution, PackageKey};
use std::path::{Path, PathBuf};

/// An immutable source snapshot after its package-owned declaration has been
/// extracted and joined to canonical source lineage.
///
/// This is source custody, not package admission. Toolchain identity and
/// compiler-issued package evidence are intentionally absent; only those later
/// stages can construct the future sealed `PackageInstance`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackageSource<S> {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    acquisition_materialization: PackageSourceMaterialization,
    materialization: PackageSourceMaterialization,
    acquisition_root: PathBuf,
    snapshot_root: PathBuf,
    navigation: PackageSourceNavigation,
    selection_evidence: PackageSourceSelectionEvidence,
    source_limits: LocalSourceLimits,
    dependency_requests: Vec<DependencySourceRequest>,
    source: S,
}

impl<S> ResolvedPackageSource<S> {
    pub(super) fn from_resolved_parts(
        key: PackageKey,
        resolution: ImmutableSourceResolution,
        acquisition_materialization: PackageSourceMaterialization,
        materialization: PackageSourceMaterialization,
        acquisition_root: PathBuf,
        snapshot_root: PathBuf,
        navigation: PackageSourceNavigation,
        selection_evidence: PackageSourceSelectionEvidence,
        source_limits: LocalSourceLimits,
        dependency_requests: Vec<DependencySourceRequest>,
        source: S,
    ) -> Self {
        Self {
            key,
            resolution,
            acquisition_materialization,
            materialization,
            acquisition_root,
            snapshot_root,
            navigation,
            selection_evidence,
            source_limits,
            dependency_requests,
            source,
        }
    }

    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
    }

    pub const fn acquisition_materialization(&self) -> &PackageSourceMaterialization {
        &self.acquisition_materialization
    }

    pub const fn materialization(&self) -> &PackageSourceMaterialization {
        &self.materialization
    }

    pub fn snapshot_root(&self) -> &Path {
        &self.snapshot_root
    }

    pub fn acquisition_root(&self) -> &Path {
        &self.acquisition_root
    }

    pub const fn navigation(&self) -> &PackageSourceNavigation {
        &self.navigation
    }

    pub const fn selection_evidence(&self) -> &PackageSourceSelectionEvidence {
        &self.selection_evidence
    }

    pub fn dependency_requests(&self) -> &[DependencySourceRequest] {
        &self.dependency_requests
    }

    pub fn source_limits(&self) -> LocalSourceLimits {
        self.source_limits
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    /// Erase the transport-specific resolver payload while retaining the
    /// immutable package source custody needed for closure reconciliation.
    ///
    /// `PackageSourceCustody` has no public constructor: adapters obtain it
    /// only after source resolution, declaration extraction, and dependency
    /// projection have all succeeded.
    pub fn into_custody(self) -> PackageSourceCustody {
        PackageSourceCustody::from_resolved_parts(
            self.key,
            self.resolution,
            self.acquisition_materialization,
            self.materialization,
            self.acquisition_root,
            self.snapshot_root,
            self.navigation,
            self.selection_evidence,
            self.source_limits,
            self.dependency_requests,
        )
    }

    pub fn into_source(self) -> S {
        self.source
    }
}
