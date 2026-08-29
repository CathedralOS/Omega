//! End-to-end Git resolution and final custody reconciliation.

use crate::resolution::source::custody::{
    CacheCustodyKind, CacheEntryLock, direct_cache_child_name, retained_cache_directory_exists,
    same_capability_file_identity, verify_git_cache_custody, verify_git_cache_root_custody,
};
use crate::resolution::source::error::SourceResolveError;
use crate::resolution::source::limits::{GIT_CONFIG_SHA256, LocalSourceLimits};
use crate::resolution::source::local::{
    SourceTreePolicy, capture_local_source, io_error, open_absolute_directory_nofollow,
};
use crate::resolution::source::observations::{
    PendingResolvedGitSource, ResolvedGitSource, issue_git_source_resolution_observation,
};
use crate::resolution::source::storage::RetainedStorageLane;
use crate::storage::record_file::{RecordFileLimits, RecordFileRoot};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::cache::{
    VerifiedGitRepository, cache_invalid, create_git_cache_entry, git_cache_identity,
    invalidate_git_cache_entry_from_open_parent,
};
#[cfg(test)]
use super::execution::test_file_network_endpoint;
use super::execution::{
    GitExecutor, reconcile_git_cache_operation_result, reconcile_git_command_result,
};
use super::objects::{
    authenticate_git_commit, inspect_git_tree, is_object_id, verify_exact_git_revision,
};
use super::request::{GitExecutionTransport, GitSourceRequest};
use super::snapshot::resolve_git_snapshot;
use omega_resolver_execution::ResolverExecutionRequestedEndpoint;

pub fn resolve_git_source(
    request: &GitSourceRequest,
    cache_dir: impl AsRef<Path>,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let limits = limits.compiler_bounded();
    let cache_dir = cache_dir.as_ref();
    std::fs::create_dir_all(cache_dir).map_err(|error| io_error(cache_dir, error))?;
    let cache_dir = cache_dir
        .canonicalize()
        .map_err(|error| io_error(cache_dir, error))?;
    verify_git_cache_root_custody(&cache_dir)?;
    let cache_directory = open_absolute_directory_nofollow(&cache_dir)
        .map_err(|error| io_error(&cache_dir, error))?;
    let result =
        resolve_git_source_from_retained_cache(request, &cache_dir, &cache_directory, limits);
    verify_git_cache_root_custody(&cache_dir)?;
    result
}

pub(in crate::resolution) fn resolve_git_source_in_lane(
    request: &GitSourceRequest,
    lane: &RetainedStorageLane,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    lane.verify_path_identity()?;
    let result = resolve_git_source_from_retained_cache(
        request,
        lane.path(),
        lane.directory(),
        limits.compiler_bounded(),
    );
    lane.verify_path_identity()?;
    result
}

fn resolve_git_source_from_retained_cache(
    request: &GitSourceRequest,
    cache_dir: &Path,
    cache_directory: &CapabilityDirectory,
    limits: LocalSourceLimits,
) -> Result<ResolvedGitSource, SourceResolveError> {
    let execution_transport = request.execution_transport();
    #[cfg(test)]
    let requested_network_endpoint = if execution_transport == GitExecutionTransport::File {
        test_file_network_endpoint()
    } else {
        requested_network_endpoint(request)?
    };
    #[cfg(not(test))]
    let requested_network_endpoint = requested_network_endpoint(request)?;
    let executor = GitExecutor::system(execution_transport, requested_network_endpoint, limits)?;
    let result = (|| {
        let requested_rev = request.requested_revision();
        let locator_identity = request.locator_identity();
        let cache_identity =
            git_cache_identity(locator_identity, requested_rev, execution_transport);
        let entry_root = cache_dir.join(format!("git-{cache_identity}"));
        let lock_name = OsString::from(format!("git-{cache_identity}.lock"));
        let entry_lock = CacheEntryLock::acquire_with_git_budget_from_parent(
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
                return Err(failure
                    .err()
                    .expect("failed cache verification must retain one failure"));
            }
        } else {
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
            )?;
        }

        entry_lock.verify_path_identity()?;
        let result = resolve_verified_git_cache_entry(
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
            !cache_entry_existed || !is_object_id(requested_rev),
        );
        let namespace_result = entry_lock.verify_path_identity();
        match result {
            Ok(pending) => {
                namespace_result?;
                finalize_git_resolution(
                    pending,
                    request,
                    &executor,
                    &entry_lock,
                    cache_dir,
                    &entry_root,
                    limits,
                )
            }
            Err(error) => {
                let invalidation_result = invalidate_git_cache_entry_from_open_parent(
                    &cache_dir,
                    entry_lock.parent(),
                    &entry_name,
                    &entry_root,
                );
                reconcile_git_cache_operation_result(
                    Err(error),
                    namespace_result,
                    Some(invalidation_result),
                )
            }
        }
    })();
    let executable_result = executor.verify_content();
    reconcile_git_command_result(result, executable_result, Ok(()))
}

pub(in crate::resolution::source) fn requested_network_endpoint(
    request: &GitSourceRequest,
) -> Result<ResolverExecutionRequestedEndpoint, SourceResolveError> {
    ResolverExecutionRequestedEndpoint::new(
        request.requested_network_endpoint().host(),
        request.requested_network_endpoint().port(),
    )
    .map_err(|error| SourceResolveError::GitExecutionBoundaryInvalid {
        message: format!("validated Git endpoint could not enter the native resolver: {error}"),
    })
}

pub(in crate::resolution::source) fn resolve_verified_git_cache_entry(
    executor: &GitExecutor,
    cache_directory: &CapabilityDirectory,
    entry_name: &OsStr,
    entry_root: &Path,
    requested_locator: &str,
    locator_identity: &str,
    fetch_locator: &str,
    requested_rev: &str,
    execution_transport: GitExecutionTransport,
    limits: LocalSourceLimits,
    fetch_remote: bool,
) -> Result<PendingResolvedGitSource, SourceResolveError> {
    let repository = VerifiedGitRepository::open(
        cache_directory,
        entry_name,
        entry_root,
        locator_identity,
        requested_rev,
        execution_transport,
        limits,
    )?;

    if fetch_remote {
        let canonical_config = repository.read_canonical_config()?;
        let arguments = bounded_git_fetch_arguments(fetch_locator, requested_rev, limits);
        repository.run_git(executor, arguments.iter())?;
        repository.restore_canonical_config(&canonical_config)?;
    }
    repository.verify_current(limits)?;

    let selected_revision = if fetch_remote {
        "FETCH_HEAD"
    } else {
        requested_rev
    };
    let commit = repository.run_git_stdout(
        executor,
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&format!("{selected_revision}^{{commit}}")),
        ],
    )?;
    let commit = commit.trim().to_owned();
    verify_exact_git_revision(requested_rev, &commit)?;
    let tree = repository.run_git_stdout(
        executor,
        [
            OsStr::new("rev-parse"),
            OsStr::new("--verify"),
            OsStr::new(&format!("{commit}^{{tree}}")),
        ],
    )?;
    let tree = tree.trim().to_owned();
    repository.verify_current(limits)?;
    authenticate_git_commit(executor, &repository, &commit, &tree)?;
    let entries = inspect_git_tree(executor, &repository, &tree, limits)?;
    repository.verify_current(limits)?;
    let (snapshot_root, local) =
        resolve_git_snapshot(executor, &repository, &tree, entries, limits)?;
    repository.verify_current(limits)?;
    executor.verify()?;
    executor.validate_execution_policy_observations()?;
    Ok(PendingResolvedGitSource {
        requested_locator: requested_locator.to_owned(),
        locator_identity: locator_identity.to_owned(),
        transport_profile: execution_transport.profile(),
        requested_rev: requested_rev.to_owned(),
        commit,
        tree,
        snapshot_root,
        local,
        git_executable: executor.identity.clone(),
        transport_executable: executor
            .transport_executable
            .as_ref()
            .map(|executable| executable.identity.clone()),
        execution_helper_executables: executor
            .execution_helpers
            .iter()
            .map(|executable| executable.identity.clone())
            .collect(),
        execution_policy_observations: executor.execution_policy_observations.borrow().clone(),
        command_execution_observations: executor.command_execution_observations.borrow().clone(),
        captured_output_observation: executor.captured_output_observation()?,
        network_transfer_observation: executor.network_transfer_observation()?,
    })
}

fn finalize_git_resolution(
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
    verify_git_cache_custody(entry_root, limits)?;
    executor.verify_content()?;
    executor.validate_execution_policy_observations()?;
    validate_pending_git_request(&pending, request)?;
    validate_pending_git_execution(&pending, executor)?;
    let resolution_observation = issue_git_source_resolution_observation(&pending, limits)?;

    Ok(ResolvedGitSource {
        requested_locator: pending.requested_locator,
        locator_identity: pending.locator_identity,
        transport_profile: pending.transport_profile,
        requested_rev: pending.requested_rev,
        commit: pending.commit,
        tree: pending.tree,
        snapshot_root: pending.snapshot_root,
        local: pending.local,
        git_executable: pending.git_executable,
        transport_executable: pending.transport_executable,
        execution_helper_executables: pending.execution_helper_executables,
        execution_policy_observations: pending.execution_policy_observations,
        command_execution_observations: pending.command_execution_observations,
        captured_output_observation: pending.captured_output_observation,
        network_transfer_observation: pending.network_transfer_observation,
        resolution_observation,
    })
}

pub(in crate::resolution::source) fn validate_pending_git_request(
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

pub(in crate::resolution::source) fn verify_pending_git_snapshot(
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

pub(in crate::resolution::source) fn bounded_git_fetch_arguments(
    fetch_locator: &str,
    requested_rev: &str,
    limits: LocalSourceLimits,
) -> Vec<OsString> {
    let first_inadmissible_blob_size = limits
        .max_bytes
        .checked_add(1)
        .expect("compiler-owned Git source byte ceiling leaves room for one sentinel byte");
    vec![
        OsString::from("fetch"),
        OsString::from("--quiet"),
        OsString::from("--depth=1"),
        OsString::from("--no-tags"),
        OsString::from("--no-recurse-submodules"),
        OsString::from(format!(
            "--filter=blob:limit={first_inadmissible_blob_size}"
        )),
        OsString::from("--"),
        OsString::from(fetch_locator),
        OsString::from(requested_rev),
    ]
}

pub(in crate::resolution::source) fn replace_canonical_git_control_file(
    entry: &CapabilityDirectory,
    repository_name: &OsStr,
    repository_path: &Path,
    canonical_config: &[u8],
) -> Result<(), SourceResolveError> {
    let classified = entry
        .symlink_metadata(repository_name)
        .map_err(|error| io_error(repository_path, error))?;
    if classified.file_type().is_symlink() || !classified.is_dir() {
        return Err(cache_invalid(
            repository_path,
            "Git repository is not a concrete directory",
        ));
    }
    let directory = entry
        .open_dir_nofollow(repository_name)
        .map_err(|error| cache_invalid(repository_path, error.to_string()))?;
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(repository_path, error))?;
    if !same_capability_file_identity(&classified, &opened) {
        return Err(cache_invalid(
            repository_path,
            "Git repository changed while opening it for configuration replacement",
        ));
    }
    replace_canonical_git_control_file_from_open_repository(
        &directory,
        repository_path,
        canonical_config,
    )
}

pub(in crate::resolution::source) fn replace_canonical_git_control_file_from_open_repository(
    repository: &CapabilityDirectory,
    repository_path: &Path,
    canonical_config: &[u8],
) -> Result<(), SourceResolveError> {
    let config_path = repository_path.join("config");
    let directory = repository
        .try_clone()
        .map_err(|error| io_error(repository_path, error))?;
    let root = RecordFileRoot::from_directory(directory, repository_path.to_path_buf()).map_err(
        |error| {
            cache_invalid(
                repository_path,
                format!("failed to bind Git configuration directory custody: {error:?}"),
            )
        },
    )?;
    root.replace_existing(
        Path::new("config"),
        canonical_config,
        RecordFileLimits {
            maximum_bytes: GIT_CONFIG_SHA256.len(),
        },
    )
    .map_err(|error| {
        cache_invalid(
            &config_path,
            format!("failed to atomically restore canonical Git configuration: {error:?}"),
        )
    })
}
