// Bounded `git cat-file --batch` blob transfer and `--batch-check` exact-object
// availability probes, backed by private capability-verified request files.

mod availability;
mod transfer;

use super::tree::validate_git_symlink_target;
use super::{GitBlobBytes, GitTreeEntry, GitTreeEntryKind};
use crate::custody::lock::same_std_and_capability_file_identity;
use crate::custody::platform::{
    same_capability_file_identity, verify_capability_cache_node_owner_and_mode,
    verify_macos_open_cache_extended_acl_custody,
};
use crate::custody::tree::CacheCustodyKind;
use crate::error::{SourceResolveError, cache_invalid, git_tree_invalid, too_many_bytes};
use crate::limits::{LocalSourceLimits, STAGING_SEQUENCE};
use crate::tree::filesystem::{IoResultAt, io_error};
pub(in crate::git) use availability::{
    ExactGitObjectAvailability, ExactGitObjectKind, probe_exact_git_object,
};
use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{
    Dir as CapabilityDirectory, Metadata as CapabilityMetadata,
    OpenOptions as CapabilityOpenOptions,
};
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;
pub(super) use transfer::read_git_blobs_batch;
#[cfg(test)]
pub(crate) use transfer::read_git_blobs_batch_from_path;

// Private request-file allocation, capability identity verification, and
// fail-closed cleanup (`PendingGitBatchRequest`) for `git cat-file` batch
// operations inside the retained cache parent.

#[cfg(unix)]
use cap_std::fs::OpenOptionsExt as CapabilityOpenOptionsExt;

#[cfg(unix)]
use crate::custody::publication::synchronize_directory;

pub(crate) struct PendingGitBatchRequest {
    pub(crate) parent: CapabilityDirectory,
    pub(crate) name: OsString,
    pub(crate) display_path: PathBuf,
    pub(crate) file: Option<File>,
    pub(crate) identity: Option<CapabilityMetadata>,
    pub(crate) removed: bool,
}

impl PendingGitBatchRequest {
    pub(crate) fn create(
        entry: &CapabilityDirectory,
        entry_root: &Path,
    ) -> Result<Self, SourceResolveError> {
        let parent = entry.try_clone().at(entry_root)?;
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
                    .at(&pending.display_path)?
                    .permissions();
                permissions.set_mode(0o600);
                pending
                    .file()
                    .set_permissions(permissions)
                    .at(&pending.display_path)?;
            }
            let identity = pending
                .parent
                .symlink_metadata(&pending.name)
                .at(&pending.display_path)?;
            pending.identity = Some(identity);
            pending.verify_current()?;
            return Ok(pending);
        }
        Err(cache_invalid(
            entry_root,
            "could not allocate a unique Git batch-request file",
        ))
    }

    pub(super) fn file(&self) -> &File {
        self.file
            .as_ref()
            .expect("live Git batch request retains its file")
    }

    pub(super) fn file_mut(&mut self) -> &mut File {
        self.file
            .as_mut()
            .expect("live Git batch request retains its file")
    }

    pub(crate) fn verify_current(&self) -> Result<(), SourceResolveError> {
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

    pub(crate) fn remove(&mut self) -> Result<(), SourceResolveError> {
        self.verify_current()?;
        drop(self.file.take());
        let named = self
            .parent
            .symlink_metadata(&self.name)
            .at(&self.display_path)?;
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
        self.parent.remove_file(&self.name).at(&self.display_path)?;
        #[cfg(unix)]
        synchronize_directory(&self.parent).at(&self.display_path)?;
        self.removed = true;
        Ok(())
    }
}

pub(in crate::git::objects::batch) fn verify_git_batch_request_identity(
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
    file: &File,
    expected: &CapabilityMetadata,
) -> Result<(), SourceResolveError> {
    let named = parent.symlink_metadata(name).at(path)?;
    let opened = file.metadata().at(path)?;
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
    Ok(())
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
    pub(super) path: PathBuf,
}

#[cfg(test)]
impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

// Exact `git cat-file --batch` request encoding, response byte-ceiling
// calculation (`git_batch_output_limit`), and zero-copy `Arc<Vec<u8>>` slice
// assignment (`assign_git_batch_output`) with symlink target validation.

fn git_batch_request_bytes(entries: &[GitTreeEntry]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in entries.iter().filter(|entry| entry.kind.carries_blob()) {
        bytes.extend_from_slice(entry.oid.as_bytes());
        bytes.push(b'\n');
    }
    bytes
}

pub(crate) fn git_batch_output_limit(
    entries: &[GitTreeEntry],
    limits: LocalSourceLimits,
) -> Result<usize, SourceResolveError> {
    for entry in entries {
        entry.validate_blob_entry()?;
    }
    let mut payload_bytes = 0_u64;
    let mut output_bytes = 0_usize;
    for entry in entries.iter().filter(|entry| entry.kind.carries_blob()) {
        payload_bytes = payload_bytes
            .checked_add(entry.size)
            .ok_or(too_many_bytes(limits.max_bytes))?;
        if payload_bytes > limits.max_bytes {
            return Err(too_many_bytes(limits.max_bytes));
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

pub(in crate::git::objects::batch) fn decimal_digit_count(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

pub(crate) fn assign_git_batch_output(
    entries: &mut [GitTreeEntry],
    output: Vec<u8>,
) -> Result<(), SourceResolveError> {
    for entry in entries.iter() {
        entry.validate_blob_entry()?;
    }
    let mut remaining = output.as_slice();
    let mut offset = 0_usize;
    let mut ranges = Vec::with_capacity(entries.len());
    for entry in entries.iter().filter(|entry| entry.kind.carries_blob()) {
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
        .filter(|entry| entry.kind.carries_blob())
        .zip(ranges)
    {
        match &mut entry.kind {
            GitTreeEntryKind::Tree | GitTreeEntryKind::Gitlink => {
                unreachable!("only blobs can receive payload bytes")
            }
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
