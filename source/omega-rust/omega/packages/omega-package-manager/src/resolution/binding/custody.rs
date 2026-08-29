//! Transport-erased custody for one declared package snapshot.

use crate::manifest::dependencies::read::DependencySourceRequest;
use crate::resolution::binding::PackageSourceNavigation;
use omega_package_source::LocalSourceLimits;
use omega_package_source::{ImmutableSourceResolution, PackageKey};
use std::path::{Path, PathBuf};

/// Immutable package source after acquisition, declaration extraction, and
/// dependency projection have all succeeded.
#[derive(Debug, Clone)]
pub struct PackageSourceCustody {
    key: PackageKey,
    resolution: ImmutableSourceResolution,
    acquisition_root: PathBuf,
    pub(crate) snapshot_root: PathBuf,
    navigation: PackageSourceNavigation,
    source_limits: LocalSourceLimits,
    dependency_requests: Vec<DependencySourceRequest>,
}

impl PartialEq for PackageSourceCustody {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
            && self.resolution == other.resolution
            && self.acquisition_root == other.acquisition_root
            && self.snapshot_root == other.snapshot_root
            && self.navigation == other.navigation
            && self.dependency_requests == other.dependency_requests
    }
}

impl Eq for PackageSourceCustody {}

impl PackageSourceCustody {
    pub(crate) fn from_resolved_parts(
        key: PackageKey,
        resolution: ImmutableSourceResolution,
        acquisition_root: PathBuf,
        snapshot_root: PathBuf,
        navigation: PackageSourceNavigation,
        source_limits: LocalSourceLimits,
        dependency_requests: Vec<DependencySourceRequest>,
    ) -> Self {
        debug_assert!(resolution.matches_lineage(key.source_lineage()));
        Self {
            key,
            resolution,
            acquisition_root,
            snapshot_root,
            navigation,
            source_limits,
            dependency_requests,
        }
    }

    pub fn key(&self) -> &PackageKey {
        &self.key
    }

    pub fn resolution(&self) -> &ImmutableSourceResolution {
        &self.resolution
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

    pub fn source_limits(&self) -> LocalSourceLimits {
        self.source_limits
    }

    pub fn dependency_requests(&self) -> &[DependencySourceRequest] {
        &self.dependency_requests
    }

    pub(crate) fn semantically_equivalent(&self, other: &Self) -> bool {
        self.key == other.key
            && self.resolution == other.resolution
            && self.navigation == other.navigation
            && self.dependency_requests == other.dependency_requests
    }
}
