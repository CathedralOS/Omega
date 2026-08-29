//! Resolver-owned private storage for ordinary source acquisition.

use super::SourceResolveError;
use super::custody::platform::same_capability_file_identity;
use super::custody::publication::{create_private_cache_directory, retain_private_cache_directory};
use super::custody::tree::{
    CacheCustodyKind, cache_custody_invalid, verify_cache_custody_root,
    verify_git_cache_root_custody,
};
use super::local::capture::{io_error, open_absolute_directory_nofollow};
use cap_std::fs::Dir as CapabilityDirectory;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

const STORAGE_COMPONENTS: &[&str] = &["CathedralOS", "Omega", "source", "v1"];
const GIT_SOURCES: &str = "git-sources";
const WORKSPACE_MEMBERS: &str = "workspace-members";
const EXTERNAL_LOCAL_SOURCES: &str = "external-local-sources";

#[derive(Debug)]
pub struct RetainedStorageLane {
    path: PathBuf,
    directory: CapabilityDirectory,
    kind: CacheCustodyKind,
}

/// Retained private per-user storage for source acquisition.
///
/// Ordinary package resolution receives this capability rather than choosing
/// an ambient cache pathname. Git quarantine, local snapshots, publication
/// stages, and locks all remain descendants of this retained root.
#[derive(Debug)]
pub struct SourceResolverStorage {
    root: PathBuf,
    directory: CapabilityDirectory,
    git_sources: RetainedStorageLane,
    workspace_members: RetainedStorageLane,
    external_local_sources: RetainedStorageLane,
}

impl SourceResolverStorage {
    /// Open or create the current user's production resolver storage.
    ///
    /// There is deliberately no temporary-directory fallback. If the host has
    /// no usable per-user cache location, source resolution fails closed.
    pub fn for_current_user() -> Result<Self, SourceResolveError> {
        let base = current_user_cache_base()?;
        Self::create_beneath(&base)
    }

    /// Open an isolated resolver tree beneath a caller-custodied base.
    ///
    /// This is the explicit hardened-mode constructor for CI, bootstrap, and
    /// other infrastructure that supplies its own private storage boundary.
    /// Ordinary commands must use [`Self::for_current_user`].
    pub fn for_hardened_base(base: impl AsRef<Path>) -> Result<Self, SourceResolveError> {
        Self::create_beneath(base.as_ref())
    }

    #[cfg(test)]
    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub fn git_sources(&self) -> &RetainedStorageLane {
        &self.git_sources
    }

    pub fn workspace_members(&self) -> &RetainedStorageLane {
        &self.workspace_members
    }

    pub fn external_local_sources(&self) -> &RetainedStorageLane {
        &self.external_local_sources
    }

    pub fn verify_path_identity(&self) -> Result<(), SourceResolveError> {
        verify_git_cache_root_custody(&self.root)?;
        let retained = self
            .directory
            .dir_metadata()
            .map_err(|error| io_error(&self.root, error))?;
        let named_directory = open_absolute_directory_nofollow(&self.root)
            .map_err(|error| io_error(&self.root, error))?;
        let named = named_directory
            .dir_metadata()
            .map_err(|error| io_error(&self.root, error))?;
        if !named.is_dir() || !same_capability_file_identity(&retained, &named) {
            return Err(cache_custody_invalid(
                CacheCustodyKind::Git,
                &self.root,
                "private resolver root pathname no longer identifies its retained directory",
            ));
        }
        for lane in [
            &self.git_sources,
            &self.workspace_members,
            &self.external_local_sources,
        ] {
            lane.verify_path_identity()?;
        }
        Ok(())
    }

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

        let git_sources =
            RetainedStorageLane::create(&root, &directory, GIT_SOURCES, CacheCustodyKind::Git)?;
        let workspace_members = RetainedStorageLane::create(
            &root,
            &directory,
            WORKSPACE_MEMBERS,
            CacheCustodyKind::LocalSnapshot,
        )?;
        let external_local_sources = RetainedStorageLane::create(
            &root,
            &directory,
            EXTERNAL_LOCAL_SOURCES,
            CacheCustodyKind::LocalSnapshot,
        )?;
        let storage = Self {
            root,
            directory,
            git_sources,
            workspace_members,
            external_local_sources,
        };
        storage.verify_path_identity()?;
        Ok(storage)
    }
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
fn current_user_cache_base() -> Result<PathBuf, SourceResolveError> {
    absolute_environment_path("LOCALAPPDATA")
}

#[cfg(target_os = "macos")]
fn current_user_cache_base() -> Result<PathBuf, SourceResolveError> {
    absolute_environment_path("HOME").map(|home| home.join("Library").join("Caches"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn current_user_cache_base() -> Result<PathBuf, SourceResolveError> {
    match std::env::var_os("XDG_CACHE_HOME") {
        Some(value) => absolute_path_from_environment("XDG_CACHE_HOME", value),
        None => absolute_environment_path("HOME").map(|home| home.join(".cache")),
    }
}

#[cfg(all(not(unix), not(target_os = "windows")))]
fn current_user_cache_base() -> Result<PathBuf, SourceResolveError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn isolated_base(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "omega-private-resolver-{name}-{}-{stamp}",
            std::process::id()
        ))
    }

    #[test]
    fn production_constructor_creates_one_retained_private_tree() {
        let base = isolated_base("storage");
        std::fs::create_dir_all(&base).expect("create isolated cache base");
        let storage = SourceResolverStorage::create_beneath(&base)
            .expect("production private-root constructor");
        assert!(storage.root().starts_with(base.canonicalize().unwrap()));
        storage
            .verify_path_identity()
            .expect("retained root identity");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for component in ["CathedralOS", "Omega", "source", "v1"] {
                let path = storage
                    .root()
                    .ancestors()
                    .find(|path| path.file_name().is_some_and(|name| name == component))
                    .expect("managed component");
                assert_eq!(
                    std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                    0o700,
                );
            }
            for lane in [
                storage.git_sources(),
                storage.workspace_members(),
                storage.external_local_sources(),
            ] {
                assert_eq!(
                    std::fs::metadata(lane.path()).unwrap().permissions().mode() & 0o777,
                    0o700,
                );
            }
        }

        drop(storage);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn retained_private_root_rejects_path_replacement() {
        let base = isolated_base("replacement");
        std::fs::create_dir_all(&base).expect("create isolated cache base");
        let storage = SourceResolverStorage::create_beneath(&base)
            .expect("production private-root constructor");
        let retained = storage.root().with_extension("retained");
        std::fs::rename(storage.root(), &retained).expect("move retained root");
        std::fs::create_dir(storage.root()).expect("replace root pathname");
        let error = storage
            .verify_path_identity()
            .expect_err("replacement must not satisfy retained storage custody");
        assert!(error.to_string().contains("no longer identifies"));

        let _ = std::fs::remove_dir_all(storage.root());
        drop(storage);
        let _ = std::fs::remove_dir_all(base);
    }

    #[test]
    fn retained_private_lane_rejects_path_replacement() {
        let base = isolated_base("lane-replacement");
        std::fs::create_dir_all(&base).expect("create isolated cache base");
        let storage = SourceResolverStorage::create_beneath(&base)
            .expect("production private-root constructor");
        let lane = storage.git_sources().path().to_path_buf();
        let retained = lane.with_extension("retained");
        std::fs::rename(&lane, &retained).expect("move retained lane");
        std::fs::create_dir(&lane).expect("replace lane pathname");
        let error = storage
            .verify_path_identity()
            .expect_err("replacement must not satisfy retained lane custody");
        assert!(error.to_string().contains("no longer identifies"));

        drop(storage);
        let _ = std::fs::remove_dir_all(base);
    }
}
