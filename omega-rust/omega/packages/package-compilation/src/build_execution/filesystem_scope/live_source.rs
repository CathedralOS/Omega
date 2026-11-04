//! A live Source directory bound during admission, before consent or execution.
//!
//! Descendants are opened relative to that retained directory, never by joining
//! an ambient path. Replacing the original root name cannot retarget a grant.

use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
use cap_std::fs::{Dir, OpenOptions};
use std::io;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(super) struct LiveBuildSource(Arc<Dir>);

impl PartialEq for LiveBuildSource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for LiveBuildSource {}

impl LiveBuildSource {
    pub(super) fn bind(root: &Path) -> io::Result<Self> {
        Ok(Self(Arc::new(Dir::open_ambient_dir(
            root,
            cap_std::ambient_authority(),
        )?)))
    }

    pub(super) fn open_read(&self, path: &[u8]) -> io::Result<std::fs::File> {
        let spelling = std::str::from_utf8(path)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Build path is not UTF-8"))?;
        let mut components = spelling.split('/').peekable();
        let mut directory = self.0.try_clone()?;
        while let Some(name) = components.next() {
            if components.peek().is_some() {
                directory = directory.open_dir_nofollow(name)?;
                continue;
            }
            if !directory.symlink_metadata(name)?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "live Build Source reads require a regular file",
                ));
            }
            let mut options = OpenOptions::new();
            options.read(true).follow(FollowSymlinks::No).nonblock(true);
            let file = directory.open_with(name, &options)?;
            if !file.metadata()?.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "live Build Source reads require a regular file",
                ));
            }
            return Ok(file.into_std());
        }
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Build input path is empty",
        ))
    }
}
