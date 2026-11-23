// Private per-invocation scratch directory (`TemporaryDirectory`) under the
// host temporary root, named `omega-<purpose>-<pid>-<seq>` so kept or
// interrupted directories are attributable to the operation that created them.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub struct TemporaryDirectory {
    path: PathBuf,
    keep: bool,
}

impl TemporaryDirectory {
    pub fn create(purpose: &str, keep: bool) -> io::Result<Self> {
        let directory = &mut std::fs::DirBuilder::new();
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(directory, 0o700);
        loop {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("omega-{purpose}-{}-{sequence}", std::process::id()));
            match directory.create(&path) {
                Ok(()) => return Ok(Self { path, keep }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::TemporaryDirectory;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn invocation_scratch_is_private_until_it_is_removed() {
        let scratch = TemporaryDirectory::create("private", false).unwrap();
        let path = scratch.path().to_owned();
        assert_eq!(path.metadata().unwrap().permissions().mode() & 0o077, 0);
        std::fs::write(path.join("entry"), b"scratch").unwrap();
        drop(scratch);
        assert!(!path.exists());
    }
}
