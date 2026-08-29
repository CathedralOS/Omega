//! Filesystem custody shared by local snapshots and Git cache entries.

use super::lock::verify_retained_cache_parent_path;
use super::platform::{
    same_capability_file_identity, verify_capability_cache_node_owner_and_mode,
    verify_macos_open_cache_extended_acl_custody, verify_windows_open_cache_custody,
};
#[cfg(test)]
use super::tree::verify_cache_custody_root;
use super::tree::{CacheCustodyKind, cache_custody_invalid};
use crate::SourceResolveError;
use crate::git::cache::identity::cache_invalid;
use crate::git::snapshot::permissions::make_open_tree_owner_writable;
use crate::limits::STAGING_SEQUENCE;
use crate::local::capture::io_error;
#[cfg(test)]
use crate::local::capture::open_absolute_directory_nofollow;
use cap_fs_ext::DirExt;
#[cfg(unix)]
use cap_std::fs::DirBuilderExt as CapabilityDirBuilderExt;
use cap_std::fs::{
    Dir as CapabilityDirectory, DirBuilder as CapabilityDirBuilder, Metadata as CapabilityMetadata,
};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
#[cfg(test)]
pub(crate) fn publish_cache_directory(
    kind: CacheCustodyKind,
    parent: &Path,
    staged: &Path,
    publication: &Path,
) -> Result<(), SourceResolveError> {
    verify_cache_custody_root(parent, kind)?;
    let directory = open_absolute_directory_nofollow(parent)
        .map_err(|error| cache_custody_invalid(kind, parent, error.to_string()))?;
    let staged_name = direct_cache_child_name(kind, parent, staged)?;
    let publication_name = direct_cache_child_name(kind, parent, publication)?;
    publish_cache_directory_from_open_parent(
        kind,
        parent,
        &directory,
        staged_name,
        publication_name,
        None,
    )
}

pub(crate) fn direct_cache_child_name<'a>(
    kind: CacheCustodyKind,
    parent: &Path,
    child: &'a Path,
) -> Result<&'a OsStr, SourceResolveError> {
    let relative = child.strip_prefix(parent).map_err(|_| {
        cache_custody_invalid(
            kind,
            child,
            "cache publication is outside its retained parent",
        )
    })?;
    let mut components = relative.components();
    let name = match (components.next(), components.next()) {
        (Some(std::path::Component::Normal(name)), None) => name,
        _ => {
            return Err(cache_custody_invalid(
                kind,
                child,
                "cache publication is not a direct child of its retained parent",
            ));
        }
    };
    Ok(name)
}

pub(crate) fn retained_cache_directory_exists(
    kind: CacheCustodyKind,
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
) -> Result<bool, SourceResolveError> {
    match parent.symlink_metadata(name) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(
            cache_custody_invalid(kind, path, "cache entry is not a concrete directory"),
        ),
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(io_error(path, error)),
    }
}

pub(crate) fn publish_cache_directory_from_open_parent(
    kind: CacheCustodyKind,
    parent: &Path,
    directory: &CapabilityDirectory,
    staged_name: &OsStr,
    publication_name: &OsStr,
    expected_staged: Option<&CapabilityMetadata>,
) -> Result<(), SourceResolveError> {
    let staged_path = parent.join(staged_name);
    let publication_path = parent.join(publication_name);
    let staged_metadata = directory
        .symlink_metadata(staged_name)
        .map_err(|error| io_error(&staged_path, error))?;
    if staged_metadata.file_type().is_symlink() || !staged_metadata.is_dir() {
        return Err(cache_custody_invalid(
            kind,
            &staged_path,
            "cache publication stage is not a concrete directory",
        ));
    }
    if expected_staged
        .is_some_and(|expected| !same_capability_file_identity(expected, &staged_metadata))
    {
        return Err(cache_custody_invalid(
            kind,
            &staged_path,
            "cache publication stage no longer identifies the retained directory",
        ));
    }
    match directory.symlink_metadata(publication_name) {
        Ok(_) => {
            return Err(cache_custody_invalid(
                kind,
                &publication_path,
                "cache publication destination already exists",
            ));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error(&publication_path, error)),
    }

    directory
        .rename(staged_name, directory, publication_name)
        .map_err(|error| io_error(&publication_path, error))?;
    let published_metadata = directory
        .symlink_metadata(publication_name)
        .map_err(|error| io_error(&publication_path, error))?;
    if !published_metadata.is_dir()
        || !same_capability_file_identity(&staged_metadata, &published_metadata)
    {
        return Err(cache_custody_invalid(
            kind,
            &publication_path,
            "published cache directory does not identify the staged directory",
        ));
    }
    directory
        .try_clone()
        .map_err(|error| io_error(parent, error))?
        .into_std_file()
        .sync_all()
        .map_err(|error| io_error(parent, error))?;
    Ok(())
}

pub(crate) struct PendingCacheEntry {
    pub(crate) root: PathBuf,
    pub(crate) parent: CapabilityDirectory,
    pub(crate) directory: Option<CapabilityDirectory>,
    pub(crate) stage_name: OsString,
    pub(crate) published: bool,
}

impl PendingCacheEntry {
    pub(crate) fn create(
        cache_dir: &Path,
        cache_directory: &CapabilityDirectory,
        cache_identity: &str,
    ) -> Result<Self, SourceResolveError> {
        let parent = cache_directory
            .try_clone()
            .map_err(|error| io_error(cache_dir, error))?;
        for _ in 0..128 {
            let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let stage_name = OsString::from(format!(
                ".git-{cache_identity}.stage-{}-{sequence}",
                std::process::id()
            ));
            let root = cache_dir.join(&stage_name);
            match create_private_cache_directory(&parent, &stage_name) {
                Ok(()) => {
                    let provisional = ProvisionalCacheDirectory::new(&parent, &stage_name);
                    let directory = retain_private_cache_directory(
                        CacheCustodyKind::Git,
                        &parent,
                        &stage_name,
                        &root,
                    )?;
                    provisional.disarm();
                    return Ok(Self {
                        root,
                        parent,
                        directory: Some(directory),
                        stage_name,
                        published: false,
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(io_error(&root, error)),
            }
        }
        Err(cache_invalid(
            cache_dir,
            "could not allocate a unique Git cache staging directory",
        ))
    }

    pub(crate) fn directory(&self) -> Result<&CapabilityDirectory, SourceResolveError> {
        self.directory
            .as_ref()
            .ok_or_else(|| cache_invalid(&self.root, "Git cache stage handle is absent"))
    }

    pub(crate) fn create_private_directory(
        &self,
        name: &str,
        path: &Path,
    ) -> Result<(), SourceResolveError> {
        let directory = self.directory()?;
        create_private_cache_directory(directory, name).map_err(|error| io_error(path, error))?;
        let provisional = ProvisionalCacheDirectory::new(directory, OsStr::new(name));
        retain_private_cache_directory(CacheCustodyKind::Git, directory, OsStr::new(name), path)?;
        provisional.disarm();
        Ok(())
    }

    pub(crate) fn verify_path_identity(&self) -> Result<CapabilityMetadata, SourceResolveError> {
        let retained = self
            .directory()?
            .dir_metadata()
            .map_err(|error| io_error(&self.root, error))?;
        let named = self
            .parent
            .symlink_metadata(&self.stage_name)
            .map_err(|error| io_error(&self.root, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_invalid(
                &self.root,
                "Git cache stage pathname no longer identifies the retained directory",
            ));
        }
        Ok(retained)
    }

    fn verify_parent_path_identity(&self, cache_dir: &Path) -> Result<(), SourceResolveError> {
        verify_retained_cache_parent_path(CacheCustodyKind::Git, cache_dir, &self.parent)
    }

    pub(crate) fn verify_ambient_path_identity(
        &self,
        cache_dir: &Path,
    ) -> Result<(), SourceResolveError> {
        self.verify_parent_path_identity(cache_dir)?;
        self.verify_path_identity().map(|_| ())
    }

    pub(crate) fn publish(
        &mut self,
        cache_dir: &Path,
        entry_root: &Path,
        entry_name: &OsStr,
    ) -> Result<(), SourceResolveError> {
        let retained = self.verify_path_identity()?;
        publish_cache_directory_from_open_parent(
            CacheCustodyKind::Git,
            cache_dir,
            &self.parent,
            &self.stage_name,
            entry_name,
            Some(&retained),
        )?;
        let published = self
            .parent
            .symlink_metadata(entry_name)
            .map_err(|error| io_error(entry_root, error))?;
        if !same_capability_file_identity(&retained, &published) {
            return Err(cache_invalid(
                entry_root,
                "published Git cache entry does not identify the retained stage",
            ));
        }
        self.published = true;
        Ok(())
    }
}

pub(crate) struct ProvisionalCacheDirectory<'a> {
    pub(crate) parent: &'a CapabilityDirectory,
    pub(crate) name: &'a OsStr,
    pub(crate) armed: bool,
}

impl<'a> ProvisionalCacheDirectory<'a> {
    pub(crate) fn new(parent: &'a CapabilityDirectory, name: &'a OsStr) -> Self {
        Self {
            parent,
            name,
            armed: true,
        }
    }

    pub(crate) fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for ProvisionalCacheDirectory<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.parent.remove_dir_all(self.name);
        }
    }
}

impl Drop for PendingCacheEntry {
    fn drop(&mut self) {
        if !self.published
            && let Some(directory) = self.directory.take()
        {
            make_open_tree_owner_writable(&directory);
            let _ = directory.remove_open_dir_all();
        }
    }
}

pub(crate) fn create_private_cache_directory(
    parent: &CapabilityDirectory,
    name: impl AsRef<Path>,
) -> std::io::Result<()> {
    #[cfg(not(target_os = "wasi"))]
    {
        #[cfg_attr(not(unix), allow(unused_mut))]
        let mut builder = CapabilityDirBuilder::new();
        #[cfg(unix)]
        builder.mode(0o700);
        parent.create_dir_with(name, &builder)
    }
    #[cfg(target_os = "wasi")]
    {
        parent.create_dir(name)
    }
}

pub(crate) fn retain_private_cache_directory(
    kind: CacheCustodyKind,
    parent: &CapabilityDirectory,
    name: &OsStr,
    path: &Path,
) -> Result<CapabilityDirectory, SourceResolveError> {
    let classified = parent
        .symlink_metadata(name)
        .map_err(|error| io_error(path, error))?;
    let directory = parent
        .open_dir_nofollow(name)
        .map_err(|error| cache_custody_invalid(kind, path, error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file()
            .set_permissions(std::fs::Permissions::from_mode(0o700))
            .map_err(|error| io_error(path, error))?;
    }
    let opened = directory
        .dir_metadata()
        .map_err(|error| io_error(path, error))?;
    if !classified.is_dir() || !same_capability_file_identity(&classified, &opened) {
        return Err(cache_custody_invalid(
            kind,
            path,
            "private cache directory changed while being retained",
        ));
    }
    verify_capability_cache_node_owner_and_mode(kind, path, &opened)?;
    #[cfg(unix)]
    {
        use cap_fs_ext::OsMetadataExt;

        if opened.mode() & 0o777 != 0o700 {
            return Err(cache_custody_invalid(
                kind,
                path,
                "private cache directory does not have exact mode 0700",
            ));
        }
    }
    verify_macos_open_cache_extended_acl_custody(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
    )?;
    verify_windows_open_cache_custody(
        kind,
        path,
        &directory
            .try_clone()
            .map_err(|error| io_error(path, error))?
            .into_std_file(),
    )?;
    Ok(directory)
}
