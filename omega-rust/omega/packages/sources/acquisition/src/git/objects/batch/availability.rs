//! Exact object absence is a successful, closed batch-check response, never
//! an interpretation of a command error. Presence still requires later object
//! authentication before any source bytes can be consumed.

mod protocol;
#[cfg(test)]
mod tests;

use super::PendingGitBatchRequest;
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
use crate::git::objects::identity::{git_object_algorithm, git_object_invalid};
use crate::limits::GIT_STDERR_LIMIT;
use crate::tree::filesystem::io_error;
use resolver_execution::ResolverExecutionPhase;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

const OPERATION: &str = "cat-file --batch-check";
const MAXIMUM_RESPONSE_BYTES: usize = 64 + 1 + 6 + 1 + 20 + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::git) enum ExactGitObjectKind {
    Commit,
    Tree,
    Blob,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::git) enum ExactGitObjectAvailability {
    Present { kind: ExactGitObjectKind, size: u64 },
    Missing,
}

pub(in crate::git) fn probe_exact_git_object(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    oid: &str,
) -> Result<ExactGitObjectAvailability, SourceResolveError> {
    validate_requested_oid(oid)?;
    executor.verify_budget()?;
    repository.verify_identity()?;
    let cache_root =
        repository
            .entry_root
            .parent()
            .ok_or_else(|| SourceResolveError::GitCacheInvalid {
                path: repository.entry_root.clone(),
                message: "Git cache entry has no retained parent".to_owned(),
            })?;
    let mut request = PendingGitBatchRequest::create(&repository.cache_parent, cache_root)?;
    let operation = (|| {
        let mut bytes = [0; 65];
        bytes[..oid.len()].copy_from_slice(oid.as_bytes());
        bytes[oid.len()] = b'\n';
        request
            .file_mut()
            .write_all(&bytes[..=oid.len()])
            .map_err(|error| io_error(&request.display_path, error))?;
        request
            .file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(|error| io_error(&request.display_path, error))?;
        request.verify_current()?;
        let stdin = request
            .file()
            .try_clone()
            .map_err(|error| io_error(&request.display_path, error))?;
        execute(executor, repository, oid, stdin)
    })();
    let namespace = repository
        .verify_identity()
        .and_then(|_| request.verify_current());
    let cleanup = request.remove();
    reconcile_git_cache_operation_result(operation, namespace, Some(cleanup))
}

fn validate_requested_oid(oid: &str) -> Result<(), SourceResolveError> {
    git_object_algorithm(oid)?;
    if oid.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(git_object_invalid(
            oid,
            "exact availability probe requires a canonical full object ID",
        ));
    }
    Ok(())
}

fn execute(
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    oid: &str,
    request: File,
) -> Result<ExactGitObjectAvailability, SourceResolveError> {
    let mut command = sealed_git_command(
        executor,
        repository.path(),
        ResolverExecutionPhase::RepositoryInspection,
    )?;
    let deadline = executor.begin_launch()?;
    command.args(["cat-file", "--batch-check"]);
    let result = run_command_bounded_with_stdin_and_budget(
        command,
        ResolverCommandInput::File(request),
        OPERATION,
        MAXIMUM_RESPONSE_BYTES,
        GIT_STDERR_LIMIT,
        deadline.duration(),
        executor.captured_output_budget.clone(),
    )
    .map_err(|error| deadline.project_error(error));
    let output = reconcile_git_command_result(result, executor.verify_budget())?;
    let availability = protocol::response(
        oid,
        output.status.success(),
        output.status.code(),
        &output.stdout,
        &output.stderr,
    )?;
    executor.verify_budget()?;
    Ok(availability)
}
