//! An invocation owns exactly the directory it created, never a process-wide slot.

use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_RUN: AtomicU64 = AtomicU64::new(0);

pub(super) struct TemporaryOutput {
    path: PathBuf,
    keep: bool,
}

impl TemporaryOutput {
    pub(super) fn create(keep: bool) -> io::Result<Self> {
        loop {
            let sequence = NEXT_RUN.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("omega-probe-{}-{sequence}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path, keep }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if !self.keep {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_runs_own_distinct_directories_and_cleanup_only_their_own() {
        let first = TemporaryOutput::create(false).unwrap();
        let second = TemporaryOutput::create(false).unwrap();
        assert_ne!(first.path(), second.path());
        let first_path = first.path().to_path_buf();
        drop(first);
        assert!(!first_path.exists());
        assert!(second.path().is_dir());
    }

    #[test]
    fn retained_output_survives_operation_scope() {
        let output = TemporaryOutput::create(true).unwrap();
        let path = output.path().to_path_buf();
        drop(output);
        assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();
    }
}
