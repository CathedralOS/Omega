use crate::manifest::dependency_projection::DependencySourceRequest;
use crate::resolution::LocalSourceLimits;
use crate::resolution::closure::{PackageSourceCustody, ResolvedSourceIdentity};
use crate::resolution::identity::{ImmutableSourceResolution, PackageKey};
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
    snapshot_root: PathBuf,
    source_limits: LocalSourceLimits,
    dependency_requests: Vec<DependencySourceRequest>,
    source: S,
}

impl<S> ResolvedPackageSource<S> {
    pub(super) fn from_resolved_parts(
        key: PackageKey,
        resolution: ImmutableSourceResolution,
        snapshot_root: PathBuf,
        source_limits: LocalSourceLimits,
        dependency_requests: Vec<DependencySourceRequest>,
        source: S,
    ) -> Self {
        Self {
            key,
            resolution,
            snapshot_root,
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

    pub fn snapshot_root(&self) -> &Path {
        &self.snapshot_root
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

    pub fn identity(&self) -> ResolvedSourceIdentity {
        ResolvedSourceIdentity::from_validated_parts(self.key.clone(), self.resolution.clone())
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
            self.snapshot_root,
            self.source_limits,
            self.dependency_requests,
        )
    }

    pub fn into_source(self) -> S {
        self.source
    }
}
