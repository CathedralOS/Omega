//! Retained private directory custody for ordinary source acquisition.

use super::SourceResolveError;
use super::custody::platform::same_capability_file_identity;
use super::custody::publication::{create_private_cache_directory, retain_private_cache_directory};
use super::custody::tree::{
    CacheCustodyKind, cache_custody_invalid, verify_cache_custody_root,
    verify_git_cache_root_custody,
};
use crate::tree::filesystem::{io_error, open_absolute_directory_nofollow};
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const STORAGE_COMPONENTS: &[&str] = &["CathedralOS", "Omega", "source", "v1"];

/// Retained directory custody for the resolver's private source root.
#[derive(Debug)]
pub(crate) struct RetainedStorageRoot {
    path: PathBuf,
    directory: CapabilityDirectory,
}

impl RetainedStorageRoot {
    pub(crate) fn create_beneath(base: &Path) -> Result<Self, SourceResolveError> {
        if !base.is_absolute() {
            return Err(SourceResolveError::PrivateStorageUnavailable {
                message: format!("per-user cache base `{}` is not absolute", base.display()),
            });
        }
        std::fs::create_dir_all(base).map_err(|error| io_error(base, error))?;
        let canonical_base = base.canonicalize().map_err(|error| io_error(base, error))?;
        let mut directory = open_absolute_directory_nofollow(&canonical_base)
            .map_err(|error| io_error(&canonical_base, error))?;
        let mut root = canonical_base;

        for component in STORAGE_COMPONENTS {
            root.push(component);
            match create_private_cache_directory(&directory, component) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(io_error(&root, error)),
            }
            directory = retain_private_cache_directory(
                CacheCustodyKind::Git,
                &directory,
                OsStr::new(component),
                &root,
            )?;
        }

        Ok(Self {
            path: root,
            directory,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn create_lane(
        &self,
        name: &str,
        kind: CacheCustodyKind,
    ) -> Result<RetainedStorageLane, SourceResolveError> {
        RetainedStorageLane::create(&self.path, &self.directory, name, kind)
    }

    pub(crate) fn verify_path_identity(&self) -> Result<(), SourceResolveError> {
        verify_git_cache_root_custody(&self.path)?;
        let retained = self
            .directory
            .dir_metadata()
            .map_err(|error| io_error(&self.path, error))?;
        let named_directory = open_absolute_directory_nofollow(&self.path)
            .map_err(|error| io_error(&self.path, error))?;
        let named = named_directory
            .dir_metadata()
            .map_err(|error| io_error(&self.path, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_custody_invalid(
                CacheCustodyKind::Git,
                &self.path,
                "private resolver root pathname no longer identifies its retained directory",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct RetainedStorageLane {
    path: PathBuf,
    directory: CapabilityDirectory,
    kind: CacheCustodyKind,
}

impl RetainedStorageLane {
    fn create(
        root: &Path,
        root_directory: &CapabilityDirectory,
        name: &str,
        kind: CacheCustodyKind,
    ) -> Result<Self, SourceResolveError> {
        let path = root.join(name);
        match create_private_cache_directory(root_directory, name) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error(&path, error)),
        }
        let directory =
            retain_private_cache_directory(kind, root_directory, OsStr::new(name), &path)?;
        let lane = Self {
            path,
            directory,
            kind,
        };
        lane.verify_path_identity()?;
        Ok(lane)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn directory(&self) -> &CapabilityDirectory {
        &self.directory
    }

    pub fn retain_child(&self, name: &str) -> Result<Self, SourceResolveError> {
        self.verify_path_identity()?;
        let child = Self::create(&self.path, &self.directory, name, self.kind)?;
        self.verify_path_identity()?;
        Ok(child)
    }

    pub fn verify_path_identity(&self) -> Result<(), SourceResolveError> {
        verify_cache_custody_root(&self.path, self.kind)?;
        let retained = self
            .directory
            .dir_metadata()
            .map_err(|error| io_error(&self.path, error))?;
        let named_directory = open_absolute_directory_nofollow(&self.path)
            .map_err(|error| io_error(&self.path, error))?;
        let named = named_directory
            .dir_metadata()
            .map_err(|error| io_error(&self.path, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_custody_invalid(
                self.kind,
                &self.path,
                "private resolver lane pathname no longer identifies its retained directory",
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn current_user_cache_base() -> Result<PathBuf, SourceResolveError> {
    absolute_environment_path("LOCALAPPDATA")
}

#[cfg(target_os = "macos")]
pub(crate) fn current_user_cache_base() -> Result<PathBuf, SourceResolveError> {
    absolute_environment_path("HOME").map(|home| home.join("Library").join("Caches"))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn current_user_cache_base() -> Result<PathBuf, SourceResolveError> {
    match std::env::var_os("XDG_CACHE_HOME") {
        Some(value) => absolute_path_from_environment("XDG_CACHE_HOME", value),
        None => absolute_environment_path("HOME").map(|home| home.join(".cache")),
    }
}

#[cfg(all(not(unix), not(target_os = "windows")))]
pub(crate) fn current_user_cache_base() -> Result<PathBuf, SourceResolveError> {
    Err(SourceResolveError::PrivateStorageUnavailable {
        message: "this platform has no compiler-owned per-user cache location".to_owned(),
    })
}

fn absolute_environment_path(name: &'static str) -> Result<PathBuf, SourceResolveError> {
    let value =
        std::env::var_os(name).ok_or_else(|| SourceResolveError::PrivateStorageUnavailable {
            message: format!("required host location `{name}` is absent"),
        })?;
    absolute_path_from_environment(name, value)
}

fn absolute_path_from_environment(
    name: &'static str,
    value: OsString,
) -> Result<PathBuf, SourceResolveError> {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(SourceResolveError::PrivateStorageUnavailable {
            message: format!("host location `{name}` is not absolute"),
        })
    }
}
