use crate::declarations::BuildDeclarationKind;
use crate::declarations::PackageKey;
use crate::declarations::dependencies::read::DependencyProjections;
use crate::resolution::source::PackageSourceCustody;
use crate::resolution::source::PackageSourceMaterialization;
use crate::resolution::source::PackageSourceNavigation;
use crate::resolution::source::PackageSourceSelectionEvidence;
use package_source::ImmutableSourceResolution;
use package_source::LocalSourceLimits;
use std::path::PathBuf;

/// An immutable source snapshot after its package-owned declaration has been
/// extracted and joined to canonical source lineage.
///
/// This is source custody, not package acceptance. Compiler checking and
/// project policy remain separate; source resolution requires no sealed or
/// certified package instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedPackageSource<S> {
    key: PackageKey,
    role: BuildDeclarationKind,
    resolution: ImmutableSourceResolution,
    materialization: PackageSourceMaterialization,
    snapshot_root: PathBuf,
    navigation: PackageSourceNavigation,
    selection_evidence: PackageSourceSelectionEvidence,
    source_limits: LocalSourceLimits,
    dependency_projections: DependencyProjections,
    source: S,
}

impl<S> ResolvedPackageSource<S> {
    pub(super) fn from_resolved_parts<D>(
        key: PackageKey,
        role: BuildDeclarationKind,
        resolution: ImmutableSourceResolution,
        materialization: PackageSourceMaterialization,
        snapshot_root: PathBuf,
        navigation: PackageSourceNavigation,
        selection_evidence: PackageSourceSelectionEvidence,
        source_limits: LocalSourceLimits,
        projected_dependencies: D,
        source: S,
    ) -> Self
    where
        D: Into<DependencyProjections>,
    {
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
            source,
        }
    }

    /// Erase the transport-specific resolver payload while retaining the
    /// immutable package source custody needed for closure reconciliation.
    ///
    /// `PackageSourceCustody` has no public constructor: adapters obtain it
    /// only after source resolution, declaration extraction, and dependency
    /// projection have all succeeded.
    pub(crate) fn into_custody(self) -> PackageSourceCustody {
        PackageSourceCustody::from_resolved_parts(
            self.key,
            self.role,
            self.resolution,
            self.materialization,
            self.snapshot_root,
            self.navigation,
            self.selection_evidence,
            self.source_limits,
            self.dependency_projections,
        )
    }
}

/// Assemble the resolved-package-source record shared by local and
/// workspace resolution: identical digest-derived materialization and
/// root navigation/selection evidence; provenance (lineage + resolution)
/// arrives already formed per source kind.
pub(super) fn resolve_local_snapshot_source(
    source: package_source::ResolvedLocalSnapshot,
    declaration: super::projection::ProjectedPackageBuild,
    lineage: package_source::SourceLineage,
    resolution: ImmutableSourceResolution,
    limits: LocalSourceLimits,
) -> Result<
    ResolvedPackageSource<package_source::ResolvedLocalSnapshot>,
    crate::resolution::source::ResolvePackageSourceError,
> {
    let materialization = PackageSourceMaterialization::from_local(source.normalized());
    Ok(ResolvedPackageSource::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        declaration.role,
        resolution,
        materialization,
        source.snapshot_root().to_path_buf(),
        PackageSourceNavigation::Root,
        PackageSourceSelectionEvidence::Root,
        limits,
        declaration.dependencies,
        source,
    ))
}
