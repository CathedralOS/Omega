use super::*;
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
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
        let mut options = CapabilityOpenOptions::new();
        options
            .read(true)
            .write(true)
            .create(true)
            .follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.mode(0o600);
        let file = directory
            .open_with(file_name, &options)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_drop_unlocks_even_while_a_duplicate_description_remains_open() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omega-record-lock-drop-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir(&path).unwrap();
        let directory =
            CapabilityDirectory::open_ambient_dir(&path, cap_std::ambient_authority()).unwrap();
        let root = RecordFileRoot::from_directory(directory, path.clone()).unwrap();
        let guard = root
            .try_lock(Path::new("transaction.lock"))
            .unwrap()
            .unwrap();
        let duplicate = guard.file.try_clone().unwrap();
        drop(guard);
        let reacquired = root.try_lock(Path::new("transaction.lock")).unwrap();
        assert!(
            reacquired.is_some(),
            "a live duplicate must not retain a dropped guard's lock"
        );
        drop(duplicate);
        drop(reacquired);
        drop(root);
        std::fs::remove_dir_all(path).unwrap();
    }
}
