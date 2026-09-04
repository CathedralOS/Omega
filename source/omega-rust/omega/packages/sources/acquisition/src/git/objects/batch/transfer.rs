//! Bounded command execution and request lifecycle orchestration.

use std::ffi::OsStr;
use std::fs::File;
#[cfg(test)]
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
#[cfg(test)]
use std::sync::atomic::Ordering;

use omega_resolver_execution::ResolverExecutionPhase;

use crate::error::SourceResolveError;
use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::commands::capture::{
    ResolverCommandInput, run_command_bounded_with_stdin_and_budget,
};
use crate::git::commands::command::sealed_git_command;
use crate::git::commands::reconciliation::{
    reconcile_git_cache_operation_result, reconcile_git_command_result,
};
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::{GitTreeEntry, GitTreeEntryKind};
#[cfg(test)]
use crate::limits::STAGING_SEQUENCE;
use crate::limits::{GIT_STDERR_LIMIT, LocalSourceLimits};
use crate::tree::filesystem::io_error;

#[cfg(test)]
use super::custody::TemporaryFileGuard;
use super::protocol::git_batch_request_bytes;
use super::{PendingGitBatchRequest, assign_git_batch_output, git_batch_output_limit};

pub(in crate::git::objects) fn read_git_blobs_batch(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    entries: &mut [GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    executor.verify_budget()?;
    if entries
        .iter()
        .all(|entry| matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        return Ok(());
    }
    let stdout_limit = git_batch_output_limit(entries, limits)?;
    let request_bytes = git_batch_request_bytes(entries);
    repository.verify_identity()?;
    let cache_root =
        repository
            .entry_root
            .parent()
            .ok_or_else(|| SourceResolveError::GitCacheInvalid {
                path: repository.entry_root.clone(),
                message: "Git cache entry has no retained parent".to_owned(),
            })?;
    // The verified cache entry is reopened as immutable custody. Keep the
    // transient cat-file request beside it in the retained mutable cache lane;
    // a Windows read-only directory handle cannot create children.
    let mut request = PendingGitBatchRequest::create(&repository.cache_parent, cache_root)?;
    let operation_result = (|| {
        let request_path = request.display_path.clone();
        write_git_batch_request(request.file_mut(), &request_path, &request_bytes)?;
        request.verify_current()?;
        let stdin = request
            .file()
            .try_clone()
            .map_err(|error| io_error(&request.display_path, error))?;
        execute_git_blob_batch(executor, repository.path(), stdin, entries, stdout_limit)
    })();
    let namespace_result = repository
        .verify_identity()
        .and_then(|_| request.verify_current());
    let cleanup_result = request.remove();
    reconcile_git_cache_operation_result(operation_result, namespace_result, Some(cleanup_result))
}

#[cfg(test)]
pub(crate) fn read_git_blobs_batch_from_path(
    executor: &GitExecutor,
    repository: &Path,
    entries: &mut [GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<(), SourceResolveError> {
    executor.verify_budget()?;
    if entries
        .iter()
        .all(|entry| matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        return Ok(());
    }
    let stdout_limit = git_batch_output_limit(entries, limits)?;
    let request_bytes = git_batch_request_bytes(entries);
    let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let request_path = repository
        .parent()
        .expect("validated bare repository has an entry root")
        .join(format!(
            ".omega-cat-file-batch.{}.{}",
            std::process::id(),
            sequence
        ));
    let request_guard = TemporaryFileGuard {
        path: request_path.clone(),
    };
    let mut request = OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&request_path)
        .map_err(|error| io_error(&request_path, error))?;
    write_git_batch_request(&mut request, &request_path, &request_bytes)?;

    let result = execute_git_blob_batch(executor, repository, request, entries, stdout_limit);
    drop(request_guard);
    result
}

fn write_git_batch_request(
    request: &mut File,
    request_path: &Path,
    bytes: &[u8],
) -> Result<(), SourceResolveError> {
    request
        .write_all(bytes)
        .map_err(|error| io_error(request_path, error))?;
    request
        .seek(SeekFrom::Start(0))
        .map(|_| ())
        .map_err(|error| io_error(request_path, error))
}

fn execute_git_blob_batch(
    executor: &GitExecutor,
    repository: &Path,
    request: File,
    entries: &mut [GitTreeEntry],
    stdout_limit: usize,
) -> Result<(), SourceResolveError> {
    let mut command = sealed_git_command(
        executor,
        repository,
        ResolverExecutionPhase::RepositoryInspection,
    )?;
    let command_timeout = executor.begin_launch()?;
    command.args([OsStr::new("cat-file"), OsStr::new("--batch")]);
    let result = run_command_bounded_with_stdin_and_budget(
        command,
        ResolverCommandInput::File(request),
        "cat-file --batch",
        stdout_limit,
        GIT_STDERR_LIMIT,
        command_timeout,
        executor.captured_output_budget.clone(),
    );
    let output = reconcile_git_command_result(result, executor.verify_budget())?;
    if !output.status.success() {
        return Err(SourceResolveError::Git {
            operation: "cat-file --batch".to_owned(),
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    assign_git_batch_output(entries, output.stdout)?;
    executor.verify_budget()
}
