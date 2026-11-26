// Directory capability for admitted live input and private staging.
//
// Descendants are opened relative to the retained `cap_std::fs::Dir` handle using
// `open_dir_nofollow` and `FollowSymlinks::No` rather than joining ambient paths,
// so renaming or symlinking the root or any intermediate directory after admission
// cannot retarget a read or creation outside the admitted directory tree.
// Pointer equality on `Arc<Dir>` ensures two independent live-root admissions
// never compare equal or exchange filesystem effects.

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct BoundDirectory(Arc<Dir>);

impl PartialEq for BoundDirectory {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for BoundDirectory {}

impl BoundDirectory {
    pub(super) fn bind(root: &Path) -> io::Result<Self> {
        Ok(Self(Arc::new(Dir::open_ambient_dir(
            root,
            cap_std::ambient_authority(),
        )?)))
    }

    fn parent<'path>(&self, path: &'path [u8]) -> io::Result<(Dir, &'path str)> {
        if !filesystem_authority::filesystem_root_relative_path_is_canonical(path, false) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "noncanonical Build path",
            ));
        }
        let spelling = std::str::from_utf8(path)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Build path is not UTF-8"))?;
        let mut components = spelling.split('/');
        let leaf = components.next_back().ok_or(io::ErrorKind::InvalidInput)?;
        let mut directory = self.0.try_clone()?;
        for name in components {
            directory = directory.open_dir_nofollow(name)?;
        }
        Ok((directory, leaf))
    }

    pub(super) fn open_read(&self, path: &[u8]) -> io::Result<std::fs::File> {
        let (directory, name) = self.parent(path)?;
        if !directory.symlink_metadata(name)?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Build directory reads require a regular file",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true).follow(FollowSymlinks::No).nonblock(true);
        let file = directory.open_with(name, &options)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Build directory reads require a regular file",
            ));
        }
        Ok(file.into_std())
    }

    pub(super) fn create_new(&self, path: &[u8]) -> io::Result<std::fs::File> {
        let (directory, name) = self.parent(path)?;
        let mut options = OpenOptions::new();
        options
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        directory
            .open_with(name, &options)
            .map(cap_std::fs::File::into_std)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::BoundDirectory;
    use std::{fs, os::unix::fs::symlink};

    #[test]
    fn creation_uses_the_bound_directory_when_its_host_name_is_replaced() {
        let temp = platform_custody::temporary_directory::TemporaryDirectory::create(
            "bound-staging-directory",
            false,
        )
        .unwrap();
        let named = temp.path().join("staging");
        let original = temp.path().join("original");
        let outside = temp.path().join("outside");
        fs::create_dir(&named).unwrap();
        fs::create_dir(&outside).unwrap();
        let bound = BoundDirectory::bind(&named).unwrap();
        fs::rename(&named, &original).unwrap();
        symlink(&outside, &named).unwrap();
        symlink(&outside, original.join("linked")).unwrap();
        bound.create_new(b"result").unwrap();
        assert!(original.join("result").is_file());
        assert!(!outside.join("result").exists());
        assert!(bound.create_new(b"linked/escape").is_err());
        assert!(!outside.join("escape").exists());
    }
}
