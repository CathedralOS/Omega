//! Recover source for comparison through existing custody issuers only.

use crate::declarations::BuildDeclarationKind;
use crate::resolution::graph::{
    CanonicalDependencySourceRequest, CanonicalSourceClosureSubject, ResolvedPackageSourceClosure,
    ResolvedSourceIdentity,
};
use crate::resolution::source::{
    PackageSourceCustody, RecordedGitPackage, recover_cached_external_local_source,
    recover_recorded_git_source,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::{GitSourceRequest, ImmutableSourceResolution, SourceResolver};

pub(super) fn recover(
    subject: &CanonicalSourceClosureSubject,
    expected: &ResolvedSourceIdentity,
    candidate: &ResolvedPackageSourceClosure,
    storage: &SourceResolver,
    acquisition: GitExactRevisionAcquisition,
) -> Result<PackageSourceCustody, &'static str> {
    let _stage = crate::review::timings::stage("old_source_recovery");
    let current = candidate
        .custody(expected.key())
        .expect("candidate package selected by key");
    if current.resolution() == expected.resolution()
        && Some(current.navigation()) == subject.package_navigation(expected.key())
    {
        return Ok(current.clone());
    }
    let cached = match expected.resolution() {
        ImmutableSourceResolution::ExternalLocal { content } => {
            let dependencies = subject
                .package_dependency_projection(expected.key())
                .ok_or("recorded old Build dependency ledger is unavailable")?;
            let role = if subject.root().selected().key() == expected.key() {
                subject.root_role()
            } else {
                BuildDeclarationKind::Package
            };
            recover_cached_external_local_source(current, content, role, dependencies, storage)
                .map_err(|_| "cached old local source could not be recovered or verified")?
        }
        _ => None,
    };
    let recovered = if let Some(cached) = cached {
        cached
    } else {
        match expected.resolution() {
            ImmutableSourceResolution::Git { .. } => {
                // A relative workspace dependency shares its repository lineage with
                // a recorded Git request. Recover its accepted path at that exact pin;
                // never infer package identity from the name or decode a lock path.
                let (repository, revision) = subject
                    .dependency_requests()
                    .iter()
                    .find_map(|edge| {
                        if edge.selected().key().source_lineage() != expected.key().source_lineage()
                        {
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
                let request = GitSourceRequest::new(repository.clone(), Some(revision.clone()))
                    .map_err(|_| "recorded Git request is unavailable")?;
                let navigation = subject
                    .package_navigation(expected.key())
                    .ok_or("recorded package navigation is unavailable")?;
                let dependencies = subject
                    .package_dependency_projection(expected.key())
                    .ok_or("recorded old Build dependency ledger is unavailable")?;
                let role = if subject.root().selected().key() == expected.key() {
                    subject.root_role()
                } else {
                    BuildDeclarationKind::Package
                };
                recover_recorded_git_source(
                    storage,
                    &request,
                    RecordedGitPackage {
                        key: expected.key(),
                        resolution: expected.resolution(),
                        navigation,
                        role,
                        dependencies,
                    },
                    acquisition,
                )
                .map_err(|_| "recorded Git commit/tree could not be recovered or verified")?
            }
            _ => {
                return Err("accepted local snapshot is missing from the available cache");
            }
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
