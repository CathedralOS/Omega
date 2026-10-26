mod recovery;
pub(crate) use recovery::recover_cached_external_local_source;

use super::projection::project_package_build;
use super::{
    AcquiredBuildSource, BuildSourceAcquisitionError, ResolvePackageSourceError,
    ResolvedPackageSource, SourceRootRole,
};
use crate::declarations::PackageKey;
use package_source::local::operations::resolve_local_source_snapshot_in_lane;
use package_source::local::staging::StagedLocalSnapshot;
use package_source::storage::RetainedStorageLane;
use package_source::{
    ExternalLocalLineage, ExternalSourceContext, ImmutableSourceResolution, SourceContentDigest,
    SourceLineage,
};
use package_source::{LocalSourceLimits, ResolvedLocalSnapshot, SourceResolver};
use std::path::Path;

pub(crate) fn external_local_dependency_root(
    requester_root: &Path,
    location: &str,
) -> Result<std::path::PathBuf, String> {
    if location.is_empty() || location.bytes().any(|byte| byte.is_ascii_control()) {
        return Err("external-local path must be nonempty and contain no control bytes".into());
    }
    let authored = Path::new(location);
    Ok(if authored.is_absolute() {
        authored.to_path_buf()
    } else {
        requester_root.join(authored)
    })
}

/// Ordinary local compilation owns a closed in-memory capture. Transaction
/// candidates and foreign review continue to request persisted custody below.
pub(crate) fn capture_external_local_project_source(
    root: &Path,
    limits: LocalSourceLimits,
    context: ExternalSourceContext,
) -> Result<super::PackageSourceCustody, ResolvePackageSourceError> {
    let AcquiredBuildSource {
        capture: captured,
        lineage,
        limits,
        ..
    } = AcquiredBuildSource::capture_local(root, limits, context).map_err(|error| match error {
        BuildSourceAcquisitionError::Source(error) => ResolvePackageSourceError::Source(error),
        BuildSourceAcquisitionError::Identity(error) => ResolvePackageSourceError::Identity(error),
    })?;
    let declaration = super::projection::project_captured_package_build(
        captured.canonical_live_root(),
        captured.tree(),
        SourceRootRole::Project,
    )?;
    Ok(super::PackageSourceCustody::from_resolved_parts(
        PackageKey::new(declaration.name, lineage),
        declaration.role,
        ImmutableSourceResolution::external_local(SourceContentDigest::derive(
            captured.normalized().content_identity.as_bytes(),
        )),
        super::PackageSourceMaterialization::from_local(captured.normalized()),
        captured.canonical_live_root().to_path_buf(),
        super::PackageSourceNavigation::Root,
        super::PackageSourceSelectionEvidence::Root,
        limits,
        declaration.dependencies,
    )
    .with_captured_source(std::sync::Arc::clone(captured.tree())))
}

/// Bind a proposed project tree to its original live path and consuming context.
/// The caller retains the stage for later transaction checks and publication.
pub(crate) fn bind_staged_external_local_project_source(
    stage: &StagedLocalSnapshot,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<&StagedLocalSnapshot>, ResolvePackageSourceError> {
    // Binding reads nothing. The stage's snapshot was hashed when it was
    // published, in this same invocation, and the live tree it came from was
    // read once to produce it. The live-edit comparison belongs to the
    // operations that then write the project's declaration files.
    let limits = limits.compiler_bounded();
    let content = SourceContentDigest::derive(stage.normalized().content_identity.as_bytes());
    let result = (|| {
        let lineage = SourceLineage::ExternalLocal(ExternalLocalLineage::canonicalize(
            stage.canonical_live_root(),
            source_context,
        )?);
        let declaration = project_package_build(stage.snapshot_root(), SourceRootRole::Project)?;
        Ok(ResolvedPackageSource::from_resolved_parts(
            PackageKey::new(declaration.name, lineage),
            declaration.role,
            ImmutableSourceResolution::external_local(content),
            super::PackageSourceMaterialization::from_local(stage.normalized()),
            stage.snapshot_root().to_path_buf(),
            super::PackageSourceNavigation::Root,
            super::PackageSourceSelectionEvidence::Root,
            limits,
            declaration.dependencies,
            stage,
        ))
    })();
    result
}

/// Resolve one declared external-local source in a retained lane.
///
/// The role carries the only difference between a dependency source and a
/// compilation root: unlike a dependency, the root of a compilation may
/// declare either `Build::application(builder, ...)` or
/// `Build::package(builder, ...)`. Dependencies reached from it remain
/// package-only. Keeping that distinction here prevents the CLI from falling
/// back to directory-shaped import lookup merely because its root is an
/// application rather than a library package.
pub(crate) fn resolve_external_local_source_in_lane(
    source_root: impl AsRef<Path>,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
    role: SourceRootRole,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    let limits = limits.compiler_bounded();
    let source = resolve_local_source_snapshot_in_lane(source_root.as_ref(), lane, limits)?;
    bind_external_local_source(source, limits, source_context, role)
}

pub(crate) fn resolve_external_local_project_source(
    source_root: impl AsRef<Path>,
    storage: &SourceResolver,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    storage.verify_path_identity()?;
    let result = resolve_external_local_source_in_lane(
        source_root,
        storage.external_local_sources(),
        limits,
        source_context,
        SourceRootRole::Project,
    );
    storage.verify_path_identity()?;
    result
}

fn bind_external_local_source(
    source: ResolvedLocalSnapshot,
    limits: LocalSourceLimits,
    source_context: ExternalSourceContext,
    role: SourceRootRole,
) -> Result<ResolvedPackageSource<ResolvedLocalSnapshot>, ResolvePackageSourceError> {
    let lineage = SourceLineage::ExternalLocal(ExternalLocalLineage::canonicalize(
        source.canonical_live_root(),
        source_context,
    )?);
    let declaration = project_package_build(source.snapshot_root(), role)?;
    let resolution = ImmutableSourceResolution::external_local(SourceContentDigest::derive(
        source.normalized().content_identity.as_bytes(),
    ));
    super::resolved::resolve_local_snapshot_source(source, declaration, lineage, resolution, limits)
}
