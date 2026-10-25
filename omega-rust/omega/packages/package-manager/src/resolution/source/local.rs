mod recovery;
pub(crate) use recovery::recover_cached_external_local_source;

use super::projection::project_package_build;
use super::{ResolvePackageSourceError, ResolvedPackageSource, SourceRootRole};
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

/// Acquired bytes and source lineage before executing the source's Build.
/// No declaration is parsed here and no provisional package key is minted.
pub struct AcquiredLocalSource {
    capture: package_source::local::resolution_observations::CapturedLocalSource,
    lineage: SourceLineage,
    limits: LocalSourceLimits,
}

#[derive(Debug)]
pub enum LocalSourceAcquisitionError {
    Source(package_source::SourceResolveError),
    Identity(package_source::IdentityError),
}

impl std::fmt::Display for LocalSourceAcquisitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Identity(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for LocalSourceAcquisitionError {}

impl AcquiredLocalSource {
    /// Capture once under the package source limits. Transaction recovery and
    /// lock selection belong to the enclosing package operation, before this
    /// acquisition; this primitive grants no review or execution authority.
    pub fn capture(
        root: &Path,
        limits: LocalSourceLimits,
        context: ExternalSourceContext,
    ) -> Result<Self, LocalSourceAcquisitionError> {
        let limits = limits.compiler_bounded();
        let capture = package_source::local::operations::capture_local_source_input(root, limits)
            .map_err(LocalSourceAcquisitionError::Source)?;
        let canonical_path = capture.canonical_live_root().to_str().ok_or_else(|| {
            LocalSourceAcquisitionError::Identity(
                package_source::IdentityError::UnsupportedNonUtf8Path(
                    capture.canonical_live_root().to_path_buf(),
                ),
            )
        })?;
        let lineage = SourceLineage::ExternalLocal(
            ExternalLocalLineage::from_canonical_path(canonical_path.to_owned(), context)
                .map_err(LocalSourceAcquisitionError::Identity)?,
        );
        Ok(Self {
            capture,
            lineage,
            limits,
        })
    }

    pub fn source_capture(
        &self,
    ) -> &package_source::local::resolution_observations::CapturedLocalSource {
        &self.capture
    }

    /// Establish a named source owner from the returned role, not Build syntax.
    /// The binding is one source, not a settled dependency graph. The compiler
    /// still joins the exact capture and original invocation before Product
    /// typing; a binding alone cannot authenticate a foreign Build return.
    pub fn bind_returned_build(
        &self,
        returned: &build_evaluation::ReturnedBuild,
    ) -> Result<package_compilation::PackageSourceBinding, String> {
        let name = match returned.declaration() {
            build_declarations::BuildDeclaration::Package(package) => &package.name,
            build_declarations::BuildDeclaration::Application(application) => &application.name,
            build_declarations::BuildDeclaration::Workspace(_) => {
                return Err("a workspace declaration does not name a Product source owner".into());
            }
        };
        let key = PackageKey::new(name.clone().into(), self.lineage.clone());
        package_compilation::PackageSourceBinding::new(
            key.identity(),
            key.name().as_str(),
            self.capture.canonical_live_root().to_path_buf(),
        )
        .with_captured_source(std::sync::Arc::clone(self.capture.tree()))
    }
}

/// Ordinary local compilation owns a closed in-memory capture. Transaction
/// candidates and foreign review continue to request persisted custody below.
pub(crate) fn capture_external_local_project_source(
    root: &Path,
    limits: LocalSourceLimits,
    context: ExternalSourceContext,
) -> Result<super::PackageSourceCustody, ResolvePackageSourceError> {
    let AcquiredLocalSource {
        capture: captured,
        lineage,
        limits,
    } = AcquiredLocalSource::capture(root, limits, context).map_err(|error| match error {
        LocalSourceAcquisitionError::Source(error) => ResolvePackageSourceError::Source(error),
        LocalSourceAcquisitionError::Identity(error) => ResolvePackageSourceError::Identity(error),
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
