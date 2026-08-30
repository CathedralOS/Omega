//! Retained staging directories and atomic snapshot publication.

use crate::SourceResolveError;
use crate::custody::platform::same_capability_file_identity;
use crate::custody::publication::{
    direct_cache_child_name, publish_cache_directory_from_open_parent,
};
#[cfg(test)]
use crate::custody::tree::verify_cache_custody_root;
use crate::custody::tree::{CacheCustodyKind, cache_custody_invalid};
use crate::limits::STAGING_SEQUENCE;
use crate::tree::filesystem::io_error;
#[cfg(test)]
use crate::tree::filesystem::open_absolute_directory_nofollow;
use cap_fs_ext::DirExt;
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::permissions::{make_open_snapshot_root_publishable, make_open_tree_owner_writable};
#[cfg(not(windows))]
use super::permissions::{make_open_snapshot_root_read_only, verify_open_snapshot_tree_modes};

pub(crate) struct PendingMaterializedSnapshot {
    pub(crate) root: PathBuf,
    pub(crate) parent: CapabilityDirectory,
    pub(crate) directory: Option<CapabilityDirectory>,
    pub(crate) stage_name: OsString,
    pub(crate) kind: CacheCustodyKind,
    pub(crate) published: bool,
}

impl PendingMaterializedSnapshot {
    #[cfg(test)]
    pub(crate) fn create(
        kind: CacheCustodyKind,
        snapshots: &Path,
        prefix: &str,
    ) -> Result<Self, SourceResolveError> {
        verify_cache_custody_root(snapshots, kind)?;
        let parent = open_absolute_directory_nofollow(snapshots)
            .map_err(|error| cache_custody_invalid(kind, snapshots, error.to_string()))?;
        Self::create_from_open_parent(kind, snapshots, &parent, prefix)
    }

    pub(crate) fn create_from_open_parent(
        kind: CacheCustodyKind,
        snapshots: &Path,
        retained_parent: &CapabilityDirectory,
        prefix: &str,
    ) -> Result<Self, SourceResolveError> {
        let parent = retained_parent
            .try_clone()
            .map_err(|error| io_error(snapshots, error))?;
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stage_name = OsString::from(format!("{prefix}-{}-{sequence}", std::process::id()));
            let root = snapshots.join(&stage_name);
            match parent.create_dir(&stage_name) {
                Ok(()) => {
                    let classified = parent
                        .symlink_metadata(&stage_name)
                        .map_err(|error| io_error(&root, error))?;
                    let directory = parent
                        .open_dir_nofollow(&stage_name)
                        .map_err(|error| cache_custody_invalid(kind, &root, error.to_string()))?;
                    let opened = directory
                        .dir_metadata()
                        .map_err(|error| io_error(&root, error))?;
                    if !classified.is_dir() || !same_capability_file_identity(&classified, &opened)
                    {
                        return Err(cache_custody_invalid(
                            kind,
                            &root,
                            "snapshot staging directory changed while being retained",
                        ));
                    }
                    return Ok(Self {
                        root,
                        parent,
                        directory: Some(directory),
                        stage_name,
                        kind,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(cache_custody_invalid(
            kind,
            snapshots,
            "could not allocate a unique materialized-snapshot staging directory",
        ))
    }

    pub(crate) fn directory(&self) -> Result<&CapabilityDirectory, SourceResolveError> {
        self.directory.as_ref().ok_or_else(|| {
            cache_custody_invalid(self.kind, &self.root, "snapshot stage handle is absent")
        })
    }

    pub(crate) fn publish(
        &mut self,
        snapshots: &Path,
        publication: &Path,
    ) -> Result<(), SourceResolveError> {
        let directory = self.directory()?;
        let retained = directory
            .dir_metadata()
            .map_err(|error| io_error(&self.root, error))?;
        let named = self
            .parent
            .symlink_metadata(&self.stage_name)
            .map_err(|error| io_error(&self.root, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_custody_invalid(
                self.kind,
                &self.root,
                "snapshot stage pathname no longer identifies the retained directory",
            ));
        }
        let publication_name = direct_cache_child_name(self.kind, snapshots, publication)?;
        make_open_snapshot_root_publishable(directory, &self.root)?;
        #[cfg(windows)]
        drop(self.directory.take());
        let renamed = std::cell::Cell::new(false);
        let publication_result = publish_cache_directory_from_open_parent(
            self.kind,
            snapshots,
            &self.parent,
            &self.stage_name,
            publication_name,
            Some(&retained),
            || renamed.set(true),
        );
        if renamed.get() {
            self.stage_name = publication_name.to_os_string();
            self.root = publication.to_path_buf();
        }
        publication_result?;

        #[cfg(not(windows))]
        {
            let directory = self.directory()?;
            make_open_snapshot_root_read_only(directory, publication)?;
            verify_open_snapshot_tree_modes(self.kind, directory, publication)?;
            let published = self
                .parent
                .symlink_metadata(publication_name)
                .map_err(|error| io_error(publication, error))?;
            if !published.is_dir() || !same_capability_file_identity(&retained, &published) {
                return Err(cache_custody_invalid(
                    self.kind,
                    publication,
                    "published snapshot does not identify the retained stage",
                ));
            }
        }
        self.published = true;
        Ok(())
    }
}

impl Drop for PendingMaterializedSnapshot {
    fn drop(&mut self) {
        if !self.published {
            if let Some(directory) = self.directory.take() {
                make_open_tree_owner_writable(&directory);
                let _ = directory.remove_open_dir_all();
            } else {
                let _ = self.parent.remove_dir_all(&self.stage_name);
            }
        }
    }
}
