//! Acquire or reuse one retained Git cache entry under exact custody.

use crate::custody::lock::CacheEntryLock;
use crate::custody::publication::{direct_cache_child_name, retained_cache_directory_exists};
use crate::custody::tree::CacheCustodyKind;
use crate::error::SourceResolveError;
use crate::git::cache::creation::create_git_cache_entry;
use crate::git::cache::identity::git_cache_identity;
use crate::git::cache::invalidation::invalidate_git_cache_entry_from_open_parent;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::commands::reconciliation::{
    reconcile_git_cache_operation_result, reconcile_git_command_result,
};
use crate::git::executable::executor::GitExecutor;
use crate::git::executable::selection::PrimaryGitSelection;
use crate::git::objects::identity::is_object_id;
use crate::git::request::GitSourceRequest;
use crate::git::workspace::GitWorkspaceProjectionError;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::{GitAcquisitionPin, ResolvedGitSource};
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::issuance::finalize_git_resolution;
use super::materialization::GitMaterializedSource;
use super::repository::resolve_verified_git_cache_entry_with;

pub(super) fn resolve_git_source_from_retained_cache_with<Evidence, PlannerError>(
    primary_git: &PrimaryGitSelection,
    package_controlled_roots: &[PathBuf],
    request: &GitSourceRequest,
    cache_dir: &Path,
    cache_directory: &CapabilityDirectory,
    limits: LocalSourceLimits,
    pin: Option<&GitAcquisitionPin>,
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
    if let Some(pin) = pin {
        if !pin.matches_request(
            request.requested_locator(),
            request.locator_identity(),
            request.transport_profile(),
            request.requested_revision(),
        ) {
            return Err(SourceResolveError::GitExecutionBoundaryInvalid {
                message: "Git acquisition reuse pin does not match the exact source request"
                    .to_owned(),
            }
            .into());
        }
    }
    let execution_transport = request.execution_transport();
    let executor = GitExecutor::selected(
        primary_git,
        execution_transport,
        limits,
        package_controlled_roots,
    )?;
    let mut materialize = Some(materialize);
    let result = (|| {
        let requested_rev = request.requested_revision();
        let locator_identity = request.locator_identity();
        let cache_identity =
            git_cache_identity(locator_identity, requested_rev, execution_transport);
        let entry_root = cache_dir.join(format!("git-{cache_identity}"));
        let lock_name = OsString::from(format!("git-{cache_identity}.lock"));
        let entry_lock = CacheEntryLock::acquire_with_budget_from_parent(
            cache_dir,
            cache_directory,
            &lock_name,
            &executor,
        )?;
        let entry_name =
            direct_cache_child_name(CacheCustodyKind::Git, cache_dir, &entry_root)?.to_os_string();
        let cache_entry_existed = retained_cache_directory_exists(
            CacheCustodyKind::Git,
            entry_lock.parent(),
            &entry_name,
            &entry_root,
        )?;
        entry_lock.verify_path_identity()?;

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
                    &cache_dir,
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
            if pin.is_some() {
                return Err(SourceResolveError::GitCacheInvalid {
                    path: entry_root,
                    message: "pinned Git acquisition cache entry is absent".to_owned(),
                }
                .into());
            }
            let creation_result = create_git_cache_entry(
                &executor,
                &cache_dir,
                entry_lock.parent(),
                &entry_root,
                &entry_name,
                &cache_identity,
                locator_identity,
                request.fetch_locator(),
                requested_rev,
                execution_transport,
                limits,
            );
            reconcile_git_cache_operation_result(
                creation_result,
                entry_lock.verify_path_identity(),
                None,
            )
            .map_err(GitWorkspaceProjectionError::Source)?;
        }

        entry_lock.verify_path_identity()?;
        let result = resolve_verified_git_cache_entry_with(
            &executor,
            entry_lock.parent(),
            &entry_name,
            &entry_root,
            request.requested_locator(),
            locator_identity,
            request.fetch_locator(),
            requested_rev,
            execution_transport,
            limits,
            pin.is_none() && (!cache_entry_existed || !is_object_id(requested_rev)),
            pin,
            materialize
                .take()
                .expect("one Git resolution invokes one materializer"),
        );
        let namespace_result = entry_lock
            .verify_path_identity()
            .map_err(GitWorkspaceProjectionError::Source);
        match result {
            Ok((pending, evidence)) => {
                namespace_result?;
                let source = finalize_git_resolution(
                    pending,
                    request,
                    &executor,
                    &entry_lock,
                    cache_dir,
                    &entry_root,
                    limits,
                )
                .map_err(GitWorkspaceProjectionError::Source)?;
                Ok((source, evidence))
            }
            Err(GitWorkspaceProjectionError::Source(error)) => {
                let invalidation_result = invalidate_git_cache_entry_from_open_parent(
                    &cache_dir,
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
    let executable_result = executor.verify_content();
    match result {
        Ok(value) => reconcile_git_command_result(Ok(value), executable_result, Ok(()))
            .map_err(GitWorkspaceProjectionError::Source),
        Err(GitWorkspaceProjectionError::Source(error)) => {
            reconcile_git_command_result(Err(error), executable_result, Ok(()))
                .map_err(GitWorkspaceProjectionError::Source)
        }
        Err(GitWorkspaceProjectionError::Planner(error)) => {
            reconcile_git_command_result(Ok(()), executable_result, Ok(()))
                .map_err(GitWorkspaceProjectionError::Source)?;
            Err(GitWorkspaceProjectionError::Planner(error))
        }
    }
}
