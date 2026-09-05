//! Bind verified historical local bytes without claiming a live resolution.

use super::super::projection::project_package_build;
use super::super::{
    PackageSourceCustody, PackageSourceMaterialization, PackageSourceNavigation,
    PackageSourceSelectionEvidence, ResolvePackageSourceError,
};
use crate::declarations::PackageKey;
use package_source::local::operations::{
    recover_cached_local_source_in_lane, verify_package_source_snapshot,
};
use package_source::{
    ImmutableSourceResolution, SourceContentDigest, SourceLineage, SourceResolverStorage,
};

pub(crate) fn recover_cached_external_local_source(
    current: &PackageSourceCustody,
    expected: &SourceContentDigest,
    storage: &SourceResolverStorage,
) -> Result<Option<PackageSourceCustody>, ResolvePackageSourceError> {
    let SourceLineage::ExternalLocal(origin) = current.key().source_lineage() else {
        return Ok(None);
    };
    // This path comes from the current resolver-issued source, not decoded
    // lock state. Recovery uses it only to identify an existing cache entry.
    storage.verify_path_identity()?;
    let Some(source) = recover_cached_local_source_in_lane(
        origin.canonical_absolute_path(),
        expected,
        storage.external_local_sources(),
        current.source_limits(),
    )?
    else {
        return Ok(None);
    };
    let declaration = project_package_build(&source.root, true)?;
    verify_package_source_snapshot(&source.root, expected, current.source_limits())?;
    storage.verify_path_identity()?;
    Ok(Some(PackageSourceCustody::from_resolved_parts(
        PackageKey::new(declaration.name, current.key().source_lineage().clone()),
        declaration.role,
        ImmutableSourceResolution::external_local(expected.clone()),
        PackageSourceMaterialization::from_local(&source),
        source.root,
        PackageSourceNavigation::Root,
        PackageSourceSelectionEvidence::Root,
        current.source_limits(),
        declaration.dependencies,
    )))
}
