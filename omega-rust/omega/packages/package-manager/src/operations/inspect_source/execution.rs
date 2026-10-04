//! Source acquisition and report assembly for source inspection.

use super::report::PackageSourceInspection;
use super::request::{PackageSourceInspectionError, PackageSourceRequest, SourceAdapter};
use package_source::git::resolution::resolve_git_source_in_lane;
use package_source::{LocalSourceLimits, SourceResolveError, SourceResolver, resolve_local_source};

fn inspect_package_source_with_resolver(
    request: PackageSourceRequest,
    resolver: &SourceResolver,
    limits: LocalSourceLimits,
) -> Result<PackageSourceInspection, SourceResolveError> {
    match request {
        PackageSourceRequest::LocalPath(path) => {
            let resolved = resolve_local_source(&path, limits)?;
            Ok(PackageSourceInspection {
                source_kind: "local-path".to_owned(),
                locator: path.display().to_string(),
                transport_profile: None,
                requested_rev: None,
                resolved_commit: None,
                resolved_tree: None,
                content_identity: resolved.content_identity,
                file_count: resolved.file_count,
                byte_count: resolved.byte_count,
            })
        }
        PackageSourceRequest::Git(request) => {
            let resolved = resolve_git_source_in_lane(
                resolver.primary_git()?,
                &request,
                resolver.git_sources(),
                limits,
            )?;
            Ok(PackageSourceInspection {
                source_kind: "git".to_owned(),
                locator: request.locator_identity().to_owned(),
                transport_profile: Some(resolved.transport_profile().as_str().to_owned()),
                requested_rev: Some(resolved.requested_revision().to_owned()),
                resolved_commit: Some(resolved.commit().to_owned()),
                resolved_tree: Some(resolved.tree().to_owned()),
                content_identity: resolved.local().content_identity.clone(),
                file_count: resolved.local().file_count,
                byte_count: resolved.local().byte_count,
            })
        }
    }
}

/// Inspect one source using manager-owned private resolver storage.
pub fn inspect_package_source(
    request: PackageSourceRequest,
    storage: &SourceResolver,
    limits: LocalSourceLimits,
) -> Result<PackageSourceInspection, SourceResolveError> {
    storage.verify_path_identity()?;
    let result = inspect_package_source_with_resolver(request, storage, limits);
    storage.verify_path_identity()?;
    result
}

/// Parse and inspect one source using manager-owned private resolver storage.
pub fn inspect_package_source_locator(
    adapter: SourceAdapter,
    locator: impl Into<String>,
    rev: Option<String>,
    storage: &SourceResolver,
    limits: LocalSourceLimits,
) -> Result<PackageSourceInspection, PackageSourceInspectionError> {
    let request = PackageSourceRequest::parse(adapter, locator, rev)
        .map_err(PackageSourceInspectionError::Parse)?;
    inspect_package_source(request, storage, limits).map_err(PackageSourceInspectionError::Resolve)
}
