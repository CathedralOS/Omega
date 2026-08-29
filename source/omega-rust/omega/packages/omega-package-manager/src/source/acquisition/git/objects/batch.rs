//! Bounded `cat-file --batch` transfer and request-file custody.

use std::ffi::{OsStr, OsString};
use std::fs::File;
#[cfg(test)]
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt as CapabilityOpenOptionsExt;
use cap_std::fs::{
    Dir as CapabilityDirectory, Metadata as CapabilityMetadata,
    OpenOptions as CapabilityOpenOptions,
};
use omega_resolver_execution::ResolverExecutionPhase;

use crate::source::acquisition::custody::{
    CacheCustodyKind, same_capability_file_identity, same_std_and_capability_file_identity,
    verify_capability_cache_node_owner_and_mode, verify_macos_open_cache_extended_acl_custody,
    verify_windows_open_cache_custody,
};
use crate::source::acquisition::error::SourceResolveError;
use crate::source::acquisition::git::cache::{VerifiedGitRepository, cache_invalid};
use crate::source::acquisition::git::execution::{
    GitExecutor, git_batch_stdin_identity, git_command_configuration_identity,
    reconcile_git_cache_operation_result, reconcile_git_command_result,
    run_command_bounded_with_stdin_and_budget, sealed_git_command_with_route,
};
use crate::source::acquisition::limits::{GIT_STDERR_LIMIT, LocalSourceLimits, STAGING_SEQUENCE};
use crate::source::acquisition::local::capture::io_error;
use crate::source::acquisition::{Seek, SeekFrom};

use super::tree::{git_tree_invalid, validate_git_symlink_target};
use super::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};

pub(super) fn read_git_blobs_batch(
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
    repository.verify_identity()?;
    let mut request = PendingGitBatchRequest::create(&repository.entry, &repository.entry_root)?;
    let operation_result = (|| {
        let request_path = request.display_path.clone();
        write_git_batch_request(request.file_mut(), &request_path, entries)?;
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
pub(in crate::source::acquisition) fn read_git_blobs_batch_from_path(
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
    write_git_batch_request(&mut request, &request_path, entries)?;

    let result = execute_git_blob_batch(executor, repository, request, entries, stdout_limit);
    drop(request_guard);
    result
}

fn write_git_batch_request(
    request: &mut File,
    request_path: &Path,
    entries: &[GitTreeEntry],
) -> Result<(), SourceResolveError> {
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        request
            .write_all(entry.oid.as_bytes())
            .and_then(|_| request.write_all(b"\n"))
            .map_err(|error| io_error(request_path, error))?;
    }
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
    let mut command = sealed_git_command_with_route(
        executor,
        repository,
        ResolverExecutionPhase::RepositoryInspection,
        None,
    )?;
    let command_timeout = executor.begin_launch()?;
    command.args([OsStr::new("cat-file"), OsStr::new("--batch")]);
    let stdin_identity = git_batch_stdin_identity(entries);
    let command_identity = git_command_configuration_identity(
        &command,
        ResolverExecutionPhase::RepositoryInspection,
        &stdin_identity,
    );
    let result = run_command_bounded_with_stdin_and_budget(
        &mut command,
        Stdio::from(request),
        "cat-file --batch",
        stdout_limit,
        GIT_STDERR_LIMIT,
        command_timeout,
        executor.captured_output_budget.clone(),
    );
    let output = reconcile_git_command_result(result, executor.verify(), executor.verify_budget())?;
    executor.record_command_execution(
        ResolverExecutionPhase::RepositoryInspection,
        command_identity,
        &output,
        None,
    )?;
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

pub(in crate::source::acquisition) fn git_batch_output_limit(
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<usize, SourceResolveError> {
    let mut payload_bytes = 0_u64;
    let mut output_bytes = 0_usize;
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        payload_bytes =
            payload_bytes
                .checked_add(entry.size)
                .ok_or(SourceResolveError::TooManyBytes {
                    limit: limits.max_bytes,
                })?;
        if payload_bytes > limits.max_bytes {
            return Err(SourceResolveError::TooManyBytes {
                limit: limits.max_bytes,
            });
        }
        let size = usize::try_from(entry.size).map_err(|_| {
            git_tree_invalid(entry.oid.as_bytes(), "blob cannot fit in host memory")
        })?;
        output_bytes = output_bytes
            .checked_add(entry.oid.len())
            .and_then(|value| value.checked_add(b" blob ".len()))
            .and_then(|value| value.checked_add(decimal_digit_count(entry.size)))
            .and_then(|value| value.checked_add(1))
            .and_then(|value| value.checked_add(size))
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                git_tree_invalid(
                    entry.oid.as_bytes(),
                    "batch output cannot fit in host memory",
                )
            })?;
    }
    Ok(output_bytes)
}

fn decimal_digit_count(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

pub(in crate::source::acquisition) fn assign_git_batch_output(
    entries: &mut [GitTreeEntry],
    output: Vec<u8>,
) -> Result<(), SourceResolveError> {
    let mut remaining = output.as_slice();
    let mut offset = 0_usize;
    let mut ranges = Vec::with_capacity(entries.len());
    for entry in entries
        .iter()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
    {
        let Some(header_end) = remaining.iter().position(|byte| *byte == b'\n') else {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "truncated cat-file batch header",
            ));
        };
        let header = &remaining[..=header_end];
        let expected_header = format!("{} blob {}\n", entry.oid, entry.size);
        if header != expected_header.as_bytes() {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "cat-file batch header did not match the exact requested blob",
            ));
        }
        remaining = &remaining[header_end + 1..];
        offset = offset
            .checked_add(header_end + 1)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
        let size = usize::try_from(entry.size).map_err(|_| {
            git_tree_invalid(entry.oid.as_bytes(), "blob cannot fit in host memory")
        })?;
        let Some(bytes) = remaining.get(..size) else {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "truncated cat-file batch blob",
            ));
        };
        if remaining.get(size) != Some(&b'\n') {
            return Err(git_tree_invalid(
                entry.oid.as_bytes(),
                "cat-file batch blob lacks its separator",
            ));
        }
        if matches!(&entry.kind, GitTreeEntryKind::Symlink { .. }) {
            validate_git_symlink_target(&entry.relative_bytes, bytes)?;
        }
        let end = offset
            .checked_add(size)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
        ranges.push(offset..end);
        remaining = &remaining[size + 1..];
        offset = end
            .checked_add(1)
            .ok_or_else(|| git_tree_invalid(entry.oid.as_bytes(), "batch offset overflow"))?;
    }
    if !remaining.is_empty() {
        return Err(git_tree_invalid(
            Vec::new(),
            "cat-file batch returned an unexpected trailing response",
        ));
    }
    let batch = Arc::new(output);
    for (entry, range) in entries
        .iter_mut()
        .filter(|entry| !matches!(&entry.kind, GitTreeEntryKind::Tree))
        .zip(ranges)
    {
        match &mut entry.kind {
            GitTreeEntryKind::Tree => unreachable!("tree rows are excluded from blob assignment"),
            GitTreeEntryKind::File { bytes, .. } => {
                *bytes = GitBlobBytes {
                    batch: Arc::clone(&batch),
                    start: range.start,
                    end: range.end,
                };
            }
            GitTreeEntryKind::Symlink { target_bytes } => {
                *target_bytes = GitBlobBytes {
                    batch: Arc::clone(&batch),
                    start: range.start,
                    end: range.end,
                };
            }
        }
    }
    Ok(())
}

pub(in crate::source::acquisition) struct PendingGitBatchRequest {
    pub(in crate::source::acquisition) parent: CapabilityDirectory,
    pub(in crate::source::acquisition) name: OsString,
    pub(in crate::source::acquisition) display_path: PathBuf,
    pub(in crate::source::acquisition) file: Option<File>,
    pub(in crate::source::acquisition) identity: Option<CapabilityMetadata>,
    pub(in crate::source::acquisition) removed: bool,
}

impl PendingGitBatchRequest {
    pub(in crate::source::acquisition) fn create(
        entry: &CapabilityDirectory,
        entry_root: &Path,
    ) -> Result<Self, SourceResolveError> {
        let parent = entry
            .try_clone()
            .map_err(|error| io_error(entry_root, error))?;
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let name = OsString::from(format!(
                ".omega-cat-file-batch.{}.{}",
                std::process::id(),
                sequence
            ));
            let display_path = entry_root.join(&name);
            let mut options = CapabilityOpenOptions::new();
            options
                .read(true)
                .write(true)
                .create_new(true)
                .follow(FollowSymlinks::No);
            #[cfg(unix)]
            options.mode(0o600);
            let capability_file = match parent.open_with(&name, &options) {
                Ok(file) => file,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&display_path, error)),
            };
            let file = capability_file.into_std();
            let mut pending = Self {
                parent,
                name,
                display_path,
                file: Some(file),
                identity: None,
                removed: false,
            };
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;

                let mut permissions = pending
                    .file()
                    .metadata()
                    .map_err(|error| io_error(&pending.display_path, error))?
                    .permissions();
                permissions.set_mode(0o600);
                pending
                    .file()
                    .set_permissions(permissions)
                    .map_err(|error| io_error(&pending.display_path, error))?;
            }
            let identity = pending
                .parent
                .symlink_metadata(&pending.name)
                .map_err(|error| io_error(&pending.display_path, error))?;
            pending.identity = Some(identity);
            pending.verify_current()?;
            return Ok(pending);
        }
        Err(cache_invalid(
            entry_root,
            "could not allocate a unique Git batch-request file",
        ))
    }

    fn file(&self) -> &File {
        self.file
            .as_ref()
            .expect("live Git batch request retains its file")
    }

    fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("live Git batch request retains its file")
    }

    pub(in crate::source::acquisition) fn verify_current(&self) -> Result<(), SourceResolveError> {
        let identity = self.identity.as_ref().ok_or_else(|| {
            cache_invalid(
                &self.display_path,
                "Git batch-request identity has not been retained",
            )
        })?;
        verify_git_batch_request_identity(
            &self.parent,
            &self.name,
            &self.display_path,
            self.file(),
            identity,
        )
    }

    pub(in crate::source::acquisition) fn remove(&mut self) -> Result<(), SourceResolveError> {
        self.verify_current()?;
        drop(self.file.take());
        let named = self
            .parent
            .symlink_metadata(&self.name)
            .map_err(|error| io_error(&self.display_path, error))?;
        if named.file_type().is_symlink()
            || !named.is_file()
            || !self
                .identity
                .as_ref()
                .is_some_and(|identity| same_capability_file_identity(identity, &named))
        {
            return Err(cache_invalid(
                &self.display_path,
                "Git batch-request name no longer identifies the retained file",
            ));
        }
        self.parent
            .remove_file(&self.name)
            .map_err(|error| io_error(&self.display_path, error))?;
        self.parent
            .try_clone()
            .map_err(|error| io_error(&self.display_path, error))?
            .into_std_file()
            .sync_all()
            .map_err(|error| io_error(&self.display_path, error))?;
        self.removed = true;
        Ok(())
    }
}

fn verify_git_batch_request_identity(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    file: &File,
    expected: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let named = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    let opened = file.metadata().map_err(|error| io_error(path, error))?;
    if named.file_type().is_symlink()
        || !named.is_file()
        || !opened.is_file()
        || !same_capability_file_identity(expected, &named)
        || !same_std_and_capability_file_identity(&opened, expected)
    {
        return Err(cache_invalid(
            path,
            "Git batch-request name does not identify the retained file",
        ));
    }
    verify_capability_cache_node_owner_and_mode(CacheCustodyKind::Git, path, &named)?;
    #[cfg(unix)]
    {
        use cap_fs_ext::OsMetadataExt;

        if named.mode() & 0o777 != 0o600 {
            return Err(cache_invalid(
                path,
                "Git batch-request file does not have exact private mode 0600",
            ));
        }
    }
    verify_macos_open_cache_extended_acl_custody(CacheCustodyKind::Git, path, file)?;
    verify_windows_open_cache_custody(CacheCustodyKind::Git, path, file)
}

impl Drop for PendingGitBatchRequest {
    fn drop(&mut self) {
        if self.removed {
            return;
        }
        let Ok(retained_name) = self.parent.symlink_metadata(&self.name) else {
            return;
        };
        if retained_name.file_type().is_symlink() || !retained_name.is_file() {
            return;
        }
        if let Some(file) = self.file.as_ref() {
            let Ok(opened) = file.metadata() else {
                return;
            };
            if !opened.is_file() || !same_std_and_capability_file_identity(&opened, &retained_name)
            {
                return;
            }
        } else if !self
            .identity
            .as_ref()
            .is_some_and(|identity| same_capability_file_identity(identity, &retained_name))
        {
            return;
        }
        drop(self.file.take());
        if let Ok(current_name) = self.parent.symlink_metadata(&self.name)
            && !current_name.file_type().is_symlink()
            && current_name.is_file()
            && same_capability_file_identity(&retained_name, &current_name)
        {
            let _ = self.parent.remove_file(&self.name);
        }
    }
}

#[cfg(test)]
struct TemporaryFileGuard {
    pub(in crate::source::acquisition) path: PathBuf,
}

#[cfg(test)]
impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
