//! Recover source for comparison through existing custody issuers only.

use crate::declarations::PackageSelection;
use crate::resolution::graph::{
    CanonicalDependencySourceRequest, CanonicalSourceClosureSubject, PackageRootSourceRequest,
    ResolvedPackageSourceClosure, ResolvedSourceIdentity,
};
use crate::resolution::source::{
    GitPackageSourceRequest, PackageSourceCustody, PackageSourceNavigation,
    resolve_external_local_project_source_with_storage,
    resolve_selected_git_package_source_at_revision_in_lanes,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{
    GitSourceRequest, ImmutableSourceResolution, LocalSourceLimits, SourceResolverStorage,
};

pub(super) fn recover(
    subject: &CanonicalSourceClosureSubject,
    expected: &ResolvedSourceIdentity,
    candidate: &ResolvedPackageSourceClosure,
    storage: &SourceResolverStorage,
) -> Result<PackageSourceCustody, &'static str> {
    let current = candidate
        .custody(expected.key())
        .expect("candidate package selected by key");
    if current.resolution() == expected.resolution()
        && Some(current.navigation()) == subject.package_navigation(expected.key())
    {
        return Ok(current.clone());
    }
    let recovered = match expected.resolution() {
        ImmutableSourceResolution::Git { commit, tree, .. } => {
            // A relative workspace dependency shares its repository lineage with
            // a recorded Git request. Select its declared name at that exact pin;
            // never infer package identity from the name or decode a lock path.
            let (repository, revision) = subject
                .dependency_requests()
                .iter()
                .find_map(|edge| {
                    if edge.selected().key().source_lineage() != expected.key().source_lineage() {
                        return None;
                    }
                    match edge.request() {
                        CanonicalDependencySourceRequest::Git {
                            repository,
                            revision,
                            ..
                        } => Some((repository, revision)),
                        _ => None,
                    }
                })
                .ok_or("no recorded Git acquisition request is available")?;
            let acquisition = GitSourceRequest::new(repository.clone(), Some(revision.clone()))
                .map_err(|_| "recorded Git request is unavailable")?;
            if acquisition.lineage() != expected.key().source_lineage() {
                return Err("recorded Git request lineage differs");
            }
            let selection = match subject.package_navigation(expected.key()) {
                Some(PackageSourceNavigation::Root) => PackageSelection::Root,
                Some(PackageSourceNavigation::Member(_)) => {
                    PackageSelection::Named(expected.key().name().clone())
                }
                None => return Err("recorded package navigation is unavailable"),
            };
            storage
                .verify_path_identity()
                .map_err(|_| "resolver storage is unavailable")?;
            let resolved = resolve_selected_git_package_source_at_revision_in_lanes(
                &GitPackageSourceRequest::new(acquisition, selection),
                commit,
                tree,
                GitExactRevisionAcquisition::AllowFetch,
                storage.git_sources(),
                storage.workspace_members(),
                LocalSourceLimits::default(),
            )
            .map_err(|_| "recorded Git commit/tree could not be recovered or verified")?;
            storage
                .verify_path_identity()
                .map_err(|_| "resolver storage is unavailable")?;
            resolved.into_custody()
        }
        _ if expected.key() == candidate.graph().root() => {
            let PackageRootSourceRequest::ExternalLocal {
                requested_root,
                source_context,
            } = candidate.source_requests().root().request()
            else {
                return Err("exact old local root is unavailable");
            };
            resolve_external_local_project_source_with_storage(
                requested_root,
                storage,
                LocalSourceLimits::default(),
                source_context.clone(),
            )
            .map_err(|_| "exact old local root could not be captured")?
            .into_custody()
        }
        _ => {
            return Err(
                "local source changed; cache-only old local source recovery is unavailable",
            );
        }
    };
    if recovered.key() != expected.key()
        || recovered.resolution() != expected.resolution()
        || Some(recovered.navigation()) != subject.package_navigation(expected.key())
    {
        return Err("recovered source does not match the accepted key, resolution, and navigation");
    }
    Ok(recovered)
}
