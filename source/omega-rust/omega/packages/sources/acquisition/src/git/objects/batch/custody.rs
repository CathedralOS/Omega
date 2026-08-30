//! Private request-file allocation, identity verification, and cleanup.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt as CapabilityOpenOptionsExt;
use cap_std::fs::{
    Dir as CapabilityDirectory, Metadata as CapabilityMetadata,
    OpenOptions as CapabilityOpenOptions,
};

use crate::custody::lock::same_std_and_capability_file_identity;
use crate::custody::platform::{
    same_capability_file_identity, verify_capability_cache_node_owner_and_mode,
    verify_macos_open_cache_extended_acl_custody,
};
use crate::custody::tree::CacheCustodyKind;
use crate::error::{SourceResolveError, cache_invalid};
use crate::limits::STAGING_SEQUENCE;
use crate::tree::filesystem::io_error;

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
pub(super) struct TemporaryFileGuard {
    pub(super) path: PathBuf,
}

#[cfg(test)]
impl Drop for TemporaryFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
