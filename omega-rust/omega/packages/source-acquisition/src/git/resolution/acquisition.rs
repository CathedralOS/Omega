//! Acquire or reuse one retained Git cache entry under exact custody.

use crate::custody::lock::CacheEntryLock;
use crate::custody::publication::{direct_cache_child_name, retained_cache_directory_exists};
use crate::custody::tree::CacheCustodyKind;
use crate::error::SourceResolveError;
use crate::git::cache::creation::create_git_cache_entry;
use crate::git::cache::identity::git_cache_identity;
use crate::git::cache::invalidation::invalidate_git_cache_entry_from_open_parent;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::cache::retry::{discard_incomplete_entry, may_preserve_cache};
use crate::git::executable::executor::GitExecutor;
use crate::git::executable::selection::{PrimaryGitSelection, resolver_package_controlled_roots};
use crate::git::git_command::reconciliation::{
    reconcile_git_cache_operation_result, reconcile_git_command_result,
};
use crate::git::objects::identity::{git_object_algorithm, is_object_id};
use crate::git::request::GitSourceRequest;
use crate::git::workspace::GitWorkspaceProjectionError;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::{PendingResolvedGitSource, ResolvedGitSource};
use crate::storage::RetainedStorageLane;
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::issuance::finalize_git_resolution;
use super::materialization::{GitMaterializedSource, materialize_whole_git_source};
use super::repository::inspect_verified_git_cache_entry;
use super::selection::GitRevisionSelection;

/// Resolve one Git source inside the retained lanes that own it.
///
/// The cache entry is held in the first lane; every lane still proves its path
/// identity around the resolution, so a moved or swapped lane cannot publish
/// source it no longer owns. Callers pass the lanes they retain; the resolver
/// derives the lane-controlled roots itself.
pub(super) fn resolve_git_source_from_retained_cache<Evidence, PlannerError>(
    primary_git: &PrimaryGitSelection,
    lanes: &[&RetainedStorageLane],
    request: &GitSourceRequest,
    limits: LocalSourceLimits,
    selection: GitRevisionSelection<'_>,
    materialize: impl FnOnce(
        &GitExecutor,
        &VerifiedGitRepository,
        &str,
        LocalSourceLimits,
    ) -> Result<
        GitMaterializedSource<Evidence>,
        GitWorkspaceProjectionError<PlannerError>,
    >,
) -> Result<(ResolvedGitSource, Evidence), GitWorkspaceProjectionError<PlannerError>> {
    let (cache_lane, _) = lanes
        .split_first()
        .expect("one Git resolution retains at least one storage lane");
    let cache_dir = cache_lane.path();
    let cache_directory = cache_lane.directory();
    let package_controlled_roots = resolver_package_controlled_roots(
        &lanes.iter().map(|lane| lane.path()).collect::<Vec<_>>(),
    )?;
    verify_retained_lane_custody(lanes)?;
    let outcome = with_git_repository_from_retained_cache(
        primary_git,
        &package_controlled_roots,
        request,
        cache_dir,
        cache_directory,
        limits,
        selection,
        |executor, repository, commit, tree, limits| {
            let materialized = materialize(executor, repository, tree, limits)?;
            let pending = PendingResolvedGitSource {
                requested_locator: request.requested_locator().to_owned(),
                lineage: request.lineage().clone(),
                locator_identity: request.locator_identity().to_owned(),
                transport_profile: request.transport_profile(),
                requested_rev: request.requested_revision().to_owned(),
                object_format: git_object_algorithm(commit)?,
                commit: commit.to_owned(),
                tree: tree.to_owned(),
                materialized_tree: materialized.materialized_tree,
                snapshot_root: materialized.snapshot_root,
                local: materialized.local,
                workspace_projection: materialized.workspace_projection,
                source_limits: limits,
            };
            Ok((pending, materialized.evidence))
        },
        |(pending, evidence), entry_lock, entry_root| {
            let source = finalize_git_resolution(
                pending, request, entry_lock, cache_dir, entry_root, limits,
            )?;
            Ok((source, evidence))
        },
    );
    // Custody outranks the budget-reconciled resolution result: a lane that
    // moved while the resolution ran owns neither the published source nor its
    // failure report.
    verify_retained_lane_custody(lanes).map_err(GitWorkspaceProjectionError::Source)?;
    outcome
}

/// Authenticate a repository revision before deciding which source bytes to open.
/// Every operation runs under the same bounded executor and retained cache lock;
/// no caller receives a live repository handle or unverified object identity.
pub(super) fn with_git_repository_from_retained_cache<Evidence, Output, PlannerError>(
    primary_git: &PrimaryGitSelection,
    package_controlled_roots: &[PathBuf],
    request: &GitSourceRequest,
    cache_dir: &Path,
    cache_directory: &CapabilityDirectory,
    limits: LocalSourceLimits,
    selection: GitRevisionSelection<'_>,
    inspect: impl FnOnce(
        &GitExecutor,
        &VerifiedGitRepository,
        &str,
        &str,
        LocalSourceLimits,
    ) -> Result<Evidence, GitWorkspaceProjectionError<PlannerError>>,
    issue: impl FnOnce(Evidence, &CacheEntryLock, &Path) -> Result<Output, SourceResolveError>,
) -> Result<Output, GitWorkspaceProjectionError<PlannerError>> {
    selection.validate_request(request)?;
    let execution_transport = request.execution_transport();
    let executor = GitExecutor::selected(
        primary_git,
        execution_transport,
        limits,
        &package_controlled_roots,
    )?;
    let mut inspect = Some(inspect);
    let result = (|| {
        let requested_rev = request.requested_revision();
        let locator_identity = request.locator_identity();
        let cache_identity =
            git_cache_identity(locator_identity, requested_rev, execution_transport);
        let entry_root = cache_dir.join(format!("git-{cache_identity}"));
        let lock_name = OsString::from(format!("git-{cache_identity}.lock"));
        let entry_lock =
            CacheEntryLock::acquire_from_parent(cache_dir, cache_directory, &lock_name, &executor)?;
        let entry_name =
            direct_cache_child_name(CacheCustodyKind::Git, cache_dir, &entry_root)?.to_os_string();
        let mut cache_entry_existed = retained_cache_directory_exists(
            CacheCustodyKind::Git,
            entry_lock.parent(),
            &entry_name,
            &entry_root,
        )?;
        entry_lock.verify_path_identity()?;

        let permits_new_acquisition = selection.permits_new_acquisition();
        if cache_entry_existed
            && permits_new_acquisition
            && discard_incomplete_entry(
                cache_dir,
                entry_lock.parent(),
                &entry_name,
                &entry_root,
                limits,
            )?
        {
            cache_entry_existed = false;
            entry_lock.verify_path_identity()?;
        }

        if cache_entry_existed {
            let verification_result = VerifiedGitRepository::open(
                entry_lock.parent(),
                &entry_name,
                &entry_root,
                locator_identity,
                requested_rev,
                execution_transport,
                limits,
            );
            let namespace_result = entry_lock.verify_path_identity();
            if verification_result.is_err() || namespace_result.is_err() {
                let invalidation_result = invalidate_git_cache_entry_from_open_parent(
                    cache_dir,
                    entry_lock.parent(),
                    &entry_name,
                    &entry_root,
                );
                let failure = reconcile_git_cache_operation_result(
                    verification_result,
                    namespace_result,
                    Some(invalidation_result),
                );
                return Err(GitWorkspaceProjectionError::Source(
                    failure
                        .err()
                        .expect("failed cache verification must retain one failure"),
                ));
            }
        } else {
            if matches!(selection, GitRevisionSelection::Ordinary(Some(_))) {
                return Err(SourceResolveError::GitCacheInvalid {
                    path: entry_root,
                    message: "pinned Git acquisition cache entry is absent".to_owned(),
                }
                .into());
            }
            if let Some(unavailable) = selection.offline_unavailable() {
                return Err(unavailable.into());
            }
            let creation_result = create_git_cache_entry(
                &executor,
                cache_dir,
                entry_lock.parent(),
                &entry_root,
                &entry_name,
                &cache_identity,
                locator_identity,
                request.fetch_locator(),
                requested_rev,
                execution_transport,
                limits,
                selection.known_object_algorithm(),
            );
            reconcile_git_cache_operation_result(
                creation_result,
                entry_lock.verify_path_identity(),
                None,
            )
            .map_err(GitWorkspaceProjectionError::Source)?;
        }

        entry_lock.verify_path_identity()?;
        let result = inspect_verified_git_cache_entry(
            &executor,
            entry_lock.parent(),
            &entry_name,
            &entry_root,
            locator_identity,
            request.fetch_locator(),
            requested_rev,
            execution_transport,
            limits,
            matches!(selection, GitRevisionSelection::Ordinary(None))
                && (!cache_entry_existed || !is_object_id(requested_rev)),
            selection,
            |executor, repository, commit, tree, limits| {
                inspect
                    .take()
                    .expect("one Git acquisition invokes one inspection")(
                    executor, repository, commit, tree, limits,
                )
            },
        );
        let namespace_result = entry_lock
            .verify_path_identity()
            .map_err(GitWorkspaceProjectionError::Source);
        match result {
            Ok(evidence) => {
                namespace_result?;
                issue(evidence, &entry_lock, &entry_root)
                    .map_err(GitWorkspaceProjectionError::Source)
            }
            Err(GitWorkspaceProjectionError::Source(error)) => {
                // Failed transport does not revoke previously fetched source.
                // Keep the cache only after checking its post-command state;
                // the next request still verifies its exact object graph.
                if namespace_result.is_ok()
                    && may_preserve_cache(&error)
                    && VerifiedGitRepository::open(
                        entry_lock.parent(),
                        &entry_name,
                        &entry_root,
                        locator_identity,
                        requested_rev,
                        execution_transport,
                        limits,
                    )
                    .is_ok()
                {
                    return Err(GitWorkspaceProjectionError::Source(error));
                }
                // Exact absence was established by the object protocol under
                // current cache custody. It is not corrupt selector cache state.
                if matches!(selection, GitRevisionSelection::Recorded(_))
                    && matches!(
                        error,
                        SourceResolveError::GitExactRevisionUnavailable { .. }
                    )
                    && namespace_result.is_ok()
                {
                    return Err(GitWorkspaceProjectionError::Source(error));
                }
                let invalidation_result = invalidate_git_cache_entry_from_open_parent(
                    cache_dir,
                    entry_lock.parent(),
                    &entry_name,
                    &entry_root,
                );
                reconcile_git_cache_operation_result(
                    Err(error),
                    namespace_result.map_err(|error| match error {
                        GitWorkspaceProjectionError::Source(error) => error,
                        GitWorkspaceProjectionError::Planner(_) => unreachable!(),
                    }),
                    Some(invalidation_result),
                )
                .map_err(GitWorkspaceProjectionError::Source)
            }
            Err(GitWorkspaceProjectionError::Planner(error)) => {
                namespace_result?;
                Err(GitWorkspaceProjectionError::Planner(error))
            }
        }
    })();
    let budget_result = executor.verify_budget();
    match result {
        Ok(value) => reconcile_git_command_result(Ok(value), budget_result)
            .map_err(GitWorkspaceProjectionError::Source),
        Err(GitWorkspaceProjectionError::Source(error)) => {
            reconcile_git_command_result(Err(error), budget_result)
                .map_err(GitWorkspaceProjectionError::Source)
        }
        Err(GitWorkspaceProjectionError::Planner(error)) => {
            reconcile_git_command_result(Ok(()), budget_result)
                .map_err(GitWorkspaceProjectionError::Source)?;
            Err(GitWorkspaceProjectionError::Planner(error))
        }
    }
}

/// Resolve one whole Git source in retained lanes.
///
/// Whole-source acquisition has no planner error lane: the whole-source
/// materializer is infallible, so the closed vocabulary is the source failure
/// set. Every whole-source entrance (ordinary, pinned, exact-revision) shares
/// this closure instead of re-deriving the lane roots and re-unwrapping the
/// planner arm.
pub(super) fn resolve_whole_git_source_from_retained_lanes(
    primary_git: &PrimaryGitSelection,
    lanes: &[&RetainedStorageLane],
    request: &GitSourceRequest,
    limits: LocalSourceLimits,
    selection: GitRevisionSelection<'_>,
) -> Result<ResolvedGitSource, SourceResolveError> {
    match resolve_git_source_from_retained_cache(
        primary_git,
        lanes,
        request,
        limits.compiler_bounded(),
        selection,
        materialize_whole_git_source,
    ) {
        Ok((source, ())) => Ok(source),
        Err(GitWorkspaceProjectionError::Source(error)) => Err(error),
        Err(GitWorkspaceProjectionError::Planner(never)) => match never {},
    }
}

/// Every lane a resolution is retained in proves its path identity together.
fn verify_retained_lane_custody(lanes: &[&RetainedStorageLane]) -> Result<(), SourceResolveError> {
    for lane in lanes {
        lane.verify_path_identity()?;
    }
    Ok(())
}
