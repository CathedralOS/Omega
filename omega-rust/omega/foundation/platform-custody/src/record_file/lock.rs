use super::{
    CapabilityDirectory, OsString, Path, PathBuf, RecordFileError, RecordFileRoot, io_error,
    single_file_name, verify_regular_identity,
};
use crate::lock_file::open_or_create_lock_file;
use std::fs::{File, TryLockError};

/// An exclusive OS lock on a persistent record file. Dropping the guard unlocks
/// and closes the file; it never unlinks the pathname.
pub struct RecordFileLock {
    directory: CapabilityDirectory,
    file: File,
    file_name: OsString,
    display_path: PathBuf,
}

impl RecordFileRoot {
    /// Open or create a stable lock file without truncation or symlink following.
    /// Returns `None` only when the OS reports lock contention. The owned guard
    /// retains its directory independently of this root.
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
        let directory = self
            .directory
            .try_clone()
            .map_err(|error| io_error(&self.display_path, error))?;
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
    /// Check that the pathname still denotes this locked regular file.
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
        // A concurrent process spawn can briefly inherit the open description
        // before close-on-exec. Release our lock now, not on its last close.
        let _ = self.file.unlock();
    }
}
