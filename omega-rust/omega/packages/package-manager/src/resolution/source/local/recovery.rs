// We bind verified historical local bytes from the private source cache without
// claiming a live resolution or re-executing `Build`. The origin path comes
// from the current resolver-issued source to identify an existing cache entry,
// while the accepted source subject supplies the executed `Build`'s name, role,
// and complete dependency ledger at this content pin after snapshot
// verification.

use super::super::{
    CHECKED_SOURCE_CACHE_LANE, COMPILED_REVIEW_CACHE_LANE, PackageSourceCustody,
    PackageSourceMaterialization, PackageSourceNavigation, PackageSourceSelectionEvidence,
    ResolvePackageSourceError,
};
use crate::declarations::BuildDeclarationKind;
use crate::declarations::dependencies::read::DependencyProjections;
use package_source::local::operations::{
    recover_cached_local_source_in_lane, verify_package_source_snapshot,
};
use package_source::{
    ImmutableSourceResolution, SourceContentDigest, SourceLineage, SourceResolver,
};

pub(crate) fn recover_cached_external_local_source(
    current: &PackageSourceCustody,
    expected: &SourceContentDigest,
    recorded_role: BuildDeclarationKind,
    recorded_dependencies: &DependencyProjections,
    storage: &SourceResolver,
) -> Result<Option<PackageSourceCustody>, ResolvePackageSourceError> {
    let SourceLineage::ExternalLocal(origin) = current.key().source_lineage() else {
        return Ok(None);
    };
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
    verify_package_source_snapshot(&source.root, expected, current.source_limits())?;
    storage.verify_path_identity()?;
    let cache_dir = storage
        .external_local_sources()
        .retain_child(CHECKED_SOURCE_CACHE_LANE)?
        .path()
        .to_path_buf();
    let review_cache_dir = storage
        .external_local_sources()
        .retain_child(COMPILED_REVIEW_CACHE_LANE)?
        .path()
        .to_path_buf();
    Ok(Some(
        PackageSourceCustody::from_resolved_parts(
            current.key().clone(),
            recorded_role,
            ImmutableSourceResolution::external_local(expected.clone()),
            PackageSourceMaterialization::from_local(&source),
            source.root,
            PackageSourceNavigation::Root,
            PackageSourceSelectionEvidence::Root,
            current.source_limits(),
            recorded_dependencies.clone(),
        )
        .with_checked_source_cache_dir(cache_dir)
        .with_compiled_review_cache_dir(review_cache_dir),
    ))
}

#[cfg(test)]
mod tests;
