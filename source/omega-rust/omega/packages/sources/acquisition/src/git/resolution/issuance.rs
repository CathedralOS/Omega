//! Revalidate pending Git custody and issue the final immutable result.

use crate::custody::lock::CacheEntryLock;
use crate::custody::tree::{verify_git_cache_custody, verify_git_cache_root_custody};
use crate::error::SourceResolveError;
use crate::error::cache_invalid;
use crate::git::executable::executor::GitExecutor;
use crate::git::request::GitSourceRequest;
use crate::limits::LocalSourceLimits;
use crate::observations::receipt::reconstruct_git_source_strict_receipt;
use crate::observations::resolution::issue_git_source_resolution_observation;
use crate::observations::resolved::{PendingResolvedGitSource, ResolvedGitSource};
use crate::observations::storage::issue_git_retained_storage_observation;
use crate::tree::capture::{SourceTreePolicy, capture_local_source};
use std::path::Path;

pub(super) fn finalize_git_resolution(
    pending: PendingResolvedGitSource,
    request: &GitSourceRequest,
    executor: &GitExecutor,
    entry_lock: &CacheEntryLock,
    cache_root: &Path,
    entry_root: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    entry_lock.verify_path_identity()?;
    verify_git_cache_root_custody(cache_root)?;
    verify_git_cache_custody(entry_root, limits)?;
    executor.verify_content()?;
    executor.validate_execution_policy_observations()?;
    verify_pending_git_snapshot(&pending, limits)?;

    entry_lock.verify_path_identity()?;
    verify_git_cache_root_custody(cache_root)?;
    let retained_storage_measurement = verify_git_cache_custody(entry_root, limits)?;
    executor.verify_content()?;
    executor.validate_execution_policy_observations()?;
    validate_pending_git_request(&pending, request)?;
    validate_pending_git_execution(&pending, executor)?;
    let retained_storage_observation =
        issue_git_retained_storage_observation(entry_root, limits, retained_storage_measurement);
    let resolution_observation =
        issue_git_source_resolution_observation(&pending, limits, &retained_storage_observation)?;
    let strict_receipt = reconstruct_git_source_strict_receipt(
        &pending,
        entry_root,
        limits,
        Some(&retained_storage_observation),
        &resolution_observation,
    );

    Ok(ResolvedGitSource {
        requested_locator: pending.requested_locator,
        locator_identity: pending.locator_identity,
        transport_profile: pending.transport_profile,
        requested_rev: pending.requested_rev,
        commit: pending.commit,
        tree: pending.tree,
        materialized_tree: pending.materialized_tree,
        snapshot_root: pending.snapshot_root,
        local: pending.local,
        workspace_projection: pending.workspace_projection,
        git_executable: pending.git_executable,
        transport_executable: pending.transport_executable,
        execution_helper_executables: pending.execution_helper_executables,
        execution_policy_observations: pending.execution_policy_observations,
        command_execution_observations: pending.command_execution_observations,
        captured_output_observation: pending.captured_output_observation,
        network_transfer_observation: pending.network_transfer_observation,
        retained_storage_observation,
        resolution_observation,
        strict_receipt,
    })
}

pub(crate) fn validate_pending_git_request(
    pending: &PendingResolvedGitSource,
    request: &GitSourceRequest,
) -> Result<(), SourceResolveError> {
    if pending.requested_locator != request.requested_locator
        || pending.locator_identity != request.locator_identity
        || pending.requested_rev != request.requested_revision
        || pending.transport_profile != request.transport_profile()
    {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "pending Git result diverged from the validated source request".to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn verify_pending_git_snapshot(
    pending: &PendingResolvedGitSource,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let recaptured = capture_local_source(
        &pending.snapshot_root,
        limits,
        SourceTreePolicy::ExactMaterialized,
    )?
    .normalized;
    if recaptured != pending.local {
        return Err(cache_invalid(
            &pending.snapshot_root,
            "published snapshot changed before final Git result issuance",
        ));
    }
    Ok(())
}

fn validate_pending_git_execution(
    pending: &PendingResolvedGitSource,
    executor: &GitExecutor,
) -> Result<(), SourceResolveError> {
    let expected_transport = executor
        .transport_executable
        .as_ref()
        .map(|executable| &executable.identity);
    let helpers_match = pending.execution_helper_executables.len()
        == executor.execution_helpers.len()
        && pending
            .execution_helper_executables
            .iter()
            .zip(executor.execution_helpers.iter())
            .all(|(pending, current)| pending == &current.identity);
    let policies = executor.execution_policy_observations.borrow();
    let commands = executor.command_execution_observations.borrow();
    let captured_output = executor.captured_output_observation()?;
    let network_transfer = executor.network_transfer_observation()?;
    if pending.transport_profile != executor.execution_transport.profile()
        || pending.git_executable != executor.identity
        || pending.transport_executable.as_ref() != expected_transport
        || !helpers_match
        || pending.execution_policy_observations.as_slice() != policies.as_slice()
        || pending.command_execution_observations.as_slice() != commands.as_slice()
        || pending.captured_output_observation != captured_output
        || pending.network_transfer_observation != network_transfer
    {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "pending Git result diverged from final executable and command custody"
                .to_owned(),
        });
    }
    Ok(())
}
