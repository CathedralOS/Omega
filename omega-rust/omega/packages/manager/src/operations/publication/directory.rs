//! Retain the project and its ignored transaction directory.

use cap_fs_ext::{DirExt, MetadataExt};
use cap_std::fs::Dir;
use std::path::{Path, PathBuf};

use super::error::io_error;
use super::{PackagePublicationError, STATE_DIRECTORY};

pub(super) struct ProjectDirectories {
    pub root_path: PathBuf,
    pub root: Dir,
    pub state: Dir,
}

impl ProjectDirectories {
    pub fn open(root: &Path) -> Result<Self, PackagePublicationError> {
        let root_path = root.canonicalize().map_err(|error| io_error(root, error))?;
        let directory = Dir::open_ambient_dir(&root_path, cap_std::ambient_authority())
            .map_err(|error| io_error(&root_path, error))?;
        let build = child(&directory, "build", &root_path)?;
        let state = child(&build, STATE_DIRECTORY, &root_path.join("build"))?;
        let result = Self {
            root_path,
            root: directory,
            state,
        };
        result.verify()?;
        Ok(result)
    }

    pub fn state_path(&self) -> PathBuf {
        self.root_path.join("build").join(STATE_DIRECTORY)
    }

    pub fn verify(&self) -> Result<(), PackagePublicationError> {
        let current = Dir::open_ambient_dir(&self.root_path, cap_std::ambient_authority())
            .map_err(|error| io_error(&self.root_path, error))?;
        same_directory(&self.root, &current)?;
        let build = current
            .open_dir_nofollow("build")
            .map_err(|error| io_error(&self.root_path.join("build"), error))?;
        let state = build
            .open_dir_nofollow(STATE_DIRECTORY)
            .map_err(|error| io_error(&self.state_path(), error))?;
        same_directory(&self.state, &state)
    }
}

fn child(parent: &Dir, name: &str, display_parent: &Path) -> Result<Dir, PackagePublicationError> {
    let path = display_parent.join(name);
    match parent.create_dir(name) {
        Ok(()) => synchronize(parent, display_parent)?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(io_error(&path, error)),
    }
    parent
        .open_dir_nofollow(name)
        .map_err(|error| io_error(&path, error))
}

fn same_directory(left: &Dir, right: &Dir) -> Result<(), PackagePublicationError> {
    let left = left
        .dir_metadata()
        .map_err(|error| io_error(Path::new("project directory"), error))?;
    let right = right
        .dir_metadata()
        .map_err(|error| io_error(Path::new("project directory"), error))?;
    if left.dev() != right.dev() || left.ino() != right.ino() {
        return Err(PackagePublicationError::DirectoryChanged);
    }
    Ok(())
}

#[cfg(unix)]
fn synchronize(directory: &Dir, path: &Path) -> Result<(), PackagePublicationError> {
    directory
        .try_clone()
        .map_err(|error| io_error(path, error))?
        .into_std_file()
        .sync_all()
        .map_err(|error| io_error(path, error))
}

#[cfg(not(unix))]
fn synchronize(_: &Dir, _: &Path) -> Result<(), PackagePublicationError> {
    Ok(())
}
