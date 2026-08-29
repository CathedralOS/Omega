//! Retained staging directories and atomic snapshot publication.

use crate::source::{
    CacheCustodyKind, STAGING_SEQUENCE, SourceResolveError, cache_custody_invalid,
    direct_cache_child_name, io_error, publish_cache_directory_from_open_parent,
    same_capability_file_identity,
};
#[cfg(test)]
use crate::source::{open_absolute_directory_nofollow, verify_cache_custody_root};
use cap_fs_ext::DirExt;
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use super::permissions::make_open_tree_owner_writable;

pub(in crate::source) struct PendingMaterializedSnapshot {
    pub(in crate::source) root: PathBuf,
    pub(in crate::source) parent: CapabilityDirectory,
    pub(in crate::source) directory: Option<CapabilityDirectory>,
    pub(in crate::source) stage_name: OsString,
    pub(in crate::source) kind: CacheCustodyKind,
    pub(in crate::source) published: bool,
}

impl PendingMaterializedSnapshot {
    #[cfg(test)]
    pub(in crate::source) fn create(
        kind: CacheCustodyKind,
        snapshots: &Path,
        prefix: &str,
    ) -> Result<Self, SourceResolveError> {
        verify_cache_custody_root(snapshots, kind)?;
        let parent = open_absolute_directory_nofollow(snapshots)
            .map_err(|error| cache_custody_invalid(kind, snapshots, error.to_string()))?;
        Self::create_from_open_parent(kind, snapshots, &parent, prefix)
    }

    pub(in crate::source) fn create_from_open_parent(
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

    pub(in crate::source) fn directory(&self) -> Result<&CapabilityDirectory, SourceResolveError> {
        self.directory.as_ref().ok_or_else(|| {
            cache_custody_invalid(self.kind, &self.root, "snapshot stage handle is absent")
        })
    }

    pub(in crate::source) fn publish(
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
        publish_cache_directory_from_open_parent(
            self.kind,
            snapshots,
            &self.parent,
            &self.stage_name,
            publication_name,
            Some(&retained),
        )?;
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
            }
        }
    }
}
