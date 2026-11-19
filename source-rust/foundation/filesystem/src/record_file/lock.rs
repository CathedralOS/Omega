// Exclusive OS lock guard (`RecordFileLock`) for persistent record files
// beneath a `RecordFileRoot`.
//
// `try_lock` opens or creates the lock file without truncation or symlink
// following, verifies that the open descriptor and directory entry share the
// same regular-file identity both before and after acquiring the non-blocking
// OS lock, and returns `Ok(None)` on contention (`TryLockError::WouldBlock`).
// `Drop` explicitly calls `file.unlock()` before closing the descriptor because
// a concurrent child spawn could briefly inherit the open file description
// prior to `CLOEXEC` and otherwise delay lock release.

use super::{
    CapabilityDirectory, OsString, Path, PathBuf, RecordFileError, RecordFileRoot, io_at, io_error,
    single_file_name, verify_regular_identity,
};
use crate::lock_file::open_or_create_lock_file;
use std::fs::{File, TryLockError};

pub struct RecordFileLock {
    directory: CapabilityDirectory,
    file: File,
    file_name: OsString,
    display_path: PathBuf,
}

impl RecordFileRoot {
    pub fn try_lock(
        &self,
        relative_path: &Path,
    ) -> Result<Option<RecordFileLock>, RecordFileError> {
        let file_name = single_file_name(relative_path)?;
        let display_path = self.display_path.join(file_name);
        match self.directory.symlink_metadata(file_name) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(RecordFileError::NotRegularFile { path: display_path });
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(&display_path, error)),
        }
        let directory = io_at(self.directory.try_clone(), &self.display_path)?;
        let file = open_or_create_lock_file(&directory, file_name)
            .map_err(|error| io_error(&display_path, error))?
            .into_std();
        let guard = RecordFileLock {
            directory,
            file,
            file_name: file_name.to_os_string(),
            display_path,
        };
        guard.verify_current()?;
        match guard.file.try_lock() {
            Ok(()) => {
                guard.verify_current()?;
                Ok(Some(guard))
            }
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(io_error(&guard.display_path, error)),
        }
    }
}

impl RecordFileLock {
    pub fn verify_current(&self) -> Result<(), RecordFileError> {
        let metadata = cap_std::fs::Metadata::from_file(&self.file)
            .map_err(|error| io_error(&self.display_path, error))?;
        verify_regular_identity(
            &self.directory,
            &self.file_name,
            &metadata,
            &self.display_path,
        )
    }
}

impl Drop for RecordFileLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}
