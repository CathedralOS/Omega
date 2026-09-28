//! Revalidate direct source custody and issue the final immutable result.

use crate::custody::lock::CacheEntryLock;
use crate::custody::tree::{verify_git_cache_custody, verify_git_cache_root_custody};
use crate::error::{SourceResolveError, cache_invalid};
use crate::git::objects::identity::git_object_algorithm;
use crate::git::request::GitSourceRequest;
use crate::limits::LocalSourceLimits;
use crate::observations::resolved::{PendingResolvedGitSource, ResolvedGitSource};
use crate::observations::storage::{
    git_retained_storage_custody, validate_git_retained_storage_custody,
};
use crate::tree::capture::{SourceTreePolicy, capture_local_source};
use std::path::Path;

pub(super) fn finalize_git_resolution(
    pending: PendingResolvedGitSource,
    request: &GitSourceRequest,
    entry_lock: &CacheEntryLock,
    cache_root: &Path,
    entry_root: &Path,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    entry_lock.verify_path_identity()?;
    verify_git_cache_root_custody(cache_root)?;
    verify_git_cache_custody(entry_root, limits)?;
    verify_pending_git_snapshot(&pending, limits)?;

    entry_lock.verify_path_identity()?;
    verify_git_cache_root_custody(cache_root)?;
    let retained_storage_measurement = verify_git_cache_custody(entry_root, limits)?;
    validate_pending_git_request(&pending, request)?;
    validate_pending_git_source_custody(&pending, limits)?;
    let retained_storage =
        git_retained_storage_custody(entry_root, limits, retained_storage_measurement);
    if !validate_git_retained_storage_custody(&retained_storage, entry_root, limits) {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "final Git retained-storage custody is inconsistent".to_owned(),
        });
    }

    Ok(ResolvedGitSource {
        requested_locator: pending.requested_locator,
        lineage: pending.lineage,
        locator_identity: pending.locator_identity,
        transport_profile: pending.transport_profile,
        requested_rev: pending.requested_rev,
        object_format: pending.object_format,
        commit: pending.commit,
        tree: pending.tree,
        materialized_tree: pending.materialized_tree,
        snapshot_root: pending.snapshot_root,
        local: pending.local,
        workspace_projection: pending.workspace_projection,
        source_limits: pending.source_limits,
        retained_storage,
    })
}

pub(crate) fn validate_pending_git_request(
    pending: &PendingResolvedGitSource,
    request: &GitSourceRequest,
) -> Result<(), SourceResolveError> {
    if pending.requested_locator != request.requested_locator
        || pending.lineage != request.lineage
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

pub(crate) fn validate_pending_git_source_custody(
    pending: &PendingResolvedGitSource,
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    let commit_format = git_object_algorithm(&pending.commit)?;
    let tree_format = git_object_algorithm(&pending.tree)?;
    let materialized_format = git_object_algorithm(&pending.materialized_tree)?;
    let projection_matches = match &pending.workspace_projection {
        Some(projection) => projection.selected_member_tree() == pending.materialized_tree,
        None => pending.materialized_tree == pending.tree,
    };
    if pending.source_limits != limits
        || pending.object_format != commit_format
        || tree_format != commit_format
        || materialized_format != commit_format
        || pending.local.root != pending.snapshot_root
        || pending.local.file_count > limits.max_entries
        || pending.local.byte_count > limits.max_bytes
        || !projection_matches
    {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "pending Git result has inconsistent direct source custody".to_owned(),
        });
    }
    Ok(())
}
