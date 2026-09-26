//! An invocation owns exactly the directory it created, never a process-wide slot.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

/// A private directory under the host temporary root, removed on drop unless
/// the invocation asked to keep it. `purpose` names the operation in the
/// directory name so leftovers from a kept or interrupted run are attributable.
pub(crate) struct TemporaryDirectory {
    path: PathBuf,
    keep: bool,
}

impl TemporaryDirectory {
    pub(crate) fn create(purpose: &str, keep: bool) -> io::Result<Self> {
        loop {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("omega-{purpose}-{}-{sequence}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path, keep }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
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

#[cfg(test)]
mod tests {
    use super::TemporaryDirectory;

    #[test]
    fn overlapping_invocations_own_distinct_directories_and_cleanup_only_their_own() {
        let first = TemporaryDirectory::create("test", false).unwrap();
        let second = TemporaryDirectory::create("test", false).unwrap();
        assert_ne!(first.path(), second.path());
        let first_path = first.path().to_path_buf();
        drop(first);
        assert!(!first_path.exists());
        assert!(second.path().is_dir());
    }

    #[test]
    fn retained_directory_survives_operation_scope() {
        let output = TemporaryDirectory::create("test", true).unwrap();
        let path = output.path().to_path_buf();
        drop(output);
        assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();
    }
}
