//! Exact object absence is a successful, closed batch-check response, never
//! an interpretation of a command error. Presence still requires later object
//! authentication before any source bytes can be consumed.

mod protocol;
#[cfg(test)]
mod tests;

use super::PendingGitBatchRequest;
use crate::bounded_process::BoundedProcessInput;
use crate::package_source::error::SourceResolveError;
use crate::package_source::git::cache::repository::VerifiedGitRepository;
use crate::package_source::git::executable::executor::GitExecutor;
use crate::package_source::git::git_command::reconciliation::reconcile_git_cache_operation_result;
use crate::package_source::git::git_command::{GitCommandCapture, run_git_output};
use crate::package_source::git::objects::identity::{git_object_algorithm, git_object_invalid};
use crate::package_source::tree::filesystem::io_error;
use crate::resolver_execution::ResolverExecutionPhase;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};

const OPERATION: &str = "cat-file --batch-check";
const MAXIMUM_RESPONSE_BYTES: usize = 64 + 1 + 6 + 1 + 20 + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::package_source::git) enum ExactGitObjectKind {
    Commit,
    Tree,
    Blob,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::package_source::git) enum ExactGitObjectAvailability {
    Present { kind: ExactGitObjectKind, size: u64 },
    Missing,
}

pub(in crate::package_source::git) fn probe_exact_git_object(
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
    let output = run_git_output(
        executor,
        repository.path(),
        ResolverExecutionPhase::RepositoryInspection,
        ["cat-file", "--batch-check"],
        GitCommandCapture {
            operation: OPERATION,
            input: BoundedProcessInput::File(request),
            stdout_limit: MAXIMUM_RESPONSE_BYTES,
        },
    )?;
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
