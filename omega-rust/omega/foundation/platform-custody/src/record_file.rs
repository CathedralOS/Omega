//! Bounded, symlink-safe record persistence under retained directory custody.

use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
use cap_std::fs::{
    Dir as CapabilityDirectory, File as CapabilityFile, OpenOptions as CapabilityOpenOptions,
};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

mod lock;
mod stage;

pub use lock::RecordFileLock;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecordFileLimits {
    pub maximum_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordFileError {
    Io { path: PathBuf, message: String },
    InvalidDestination { path: PathBuf },
    NotRegularFile { path: PathBuf },
    DestinationExists { path: PathBuf },
    PublishedButUnconfirmed { path: PathBuf, message: String },
    ContentsChanged { path: PathBuf },
    ByteLimitExceeded { actual: u64, maximum: usize },
    LengthOverflow,
    AllocationFailed,
    StageNameSpaceExhausted { directory: PathBuf },
}

#[derive(Debug)]
pub struct RecordFileRoot {
    display_path: PathBuf,
    directory: CapabilityDirectory,
}

impl RecordFileRoot {
    pub fn from_directory(
        directory: CapabilityDirectory,
        display_path: PathBuf,
    ) -> Result<Self, RecordFileError> {
        let metadata = io_at(directory.dir_metadata(), &display_path)?;
        if !metadata.is_dir() {
            return Err(RecordFileError::NotRegularFile { path: display_path });
        }
        Ok(Self {
            display_path,
            directory,
        })
    }

    pub fn read(
        &self,
        relative_path: &Path,
        limits: RecordFileLimits,
    ) -> Result<RootRecordRead<'_>, RecordFileError> {
        self.read_optional(relative_path, limits)?.ok_or_else(|| {
            io_error(
                &self.display_path.join(relative_path),
                std::io::ErrorKind::NotFound.into(),
            )
        })
    }

    /// Read a record if its pathname exists. Symlinks and invalid destinations
    /// remain errors, including dangling symlinks. No lock is acquired.
    pub fn read_optional(
        &self,
        relative_path: &Path,
        limits: RecordFileLimits,
    ) -> Result<Option<RootRecordRead<'_>>, RecordFileError> {
        let file_name = single_file_name(relative_path)?;
        let display_path = self.display_path.join(file_name);
        let path_metadata = match self.directory.symlink_metadata(file_name) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error(&display_path, error)),
        };
        if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
            return Err(RecordFileError::NotRegularFile { path: display_path });
        }
        let mut options = CapabilityOpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = io_at(self.directory.open_with(file_name, &options), &display_path)?;
        verify_capability_regular_identity(&self.directory, file_name, &file, &display_path)?;
        let metadata = io_at(file.metadata(), &display_path)?;
        if metadata.len() > u64::try_from(limits.maximum_bytes).unwrap_or(u64::MAX) {
            return Err(RecordFileError::ByteLimitExceeded {
                actual: metadata.len(),
                maximum: limits.maximum_bytes,
            });
        }
        let bytes = read_bytes_bounded(&file, &display_path, limits)?;
        let mut read = RootRecordRead {
            bytes,
            permissions: metadata.permissions(),
            file,
            root: self,
            file_name: file_name.to_os_string(),
            display_path,
        };
        read.verify_current(limits)?;
        Ok(Some(read))
    }

    pub fn write_new(
        &self,
        relative_path: &Path,
        bytes: &[u8],
        limits: RecordFileLimits,
    ) -> Result<(), RecordFileError> {
        self.write_new_in(relative_path, bytes, limits, self)
    }

    /// Publish a new record from a synchronized stage beneath `staging`.
    /// Cross-device hard-link errors propagate without a copy fallback.
    pub fn write_new_in(
        &self,
        relative_path: &Path,
        bytes: &[u8],
        limits: RecordFileLimits,
        staging: &RecordFileRoot,
    ) -> Result<(), RecordFileError> {
        enforce_byte_limit(bytes.len(), limits)?;
        let file_name = single_file_name(relative_path)?;
        let display_path = self.display_path.join(file_name);
        let mut stage = stage::prepare(staging, bytes, limits, None)?;

        match stage
            .directory
            .hard_link(&stage.file_name, &self.directory, file_name)
        {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(RecordFileError::DestinationExists { path: display_path });
            }
            Err(error) => return Err(io_error(&display_path, error)),
        }

        let confirmation = (|| {
            verify_capability_regular_identity(
                &self.directory,
                file_name,
                &stage.file,
                &display_path,
            )?;
            stage.remove()?;
            synchronize_directory(&self.directory, &self.display_path)?;
            synchronize_directory(&staging.directory, &staging.display_path)?;
            verify_capability_regular_identity(
                &self.directory,
                file_name,
                &stage.file,
                &display_path,
            )
        })();

        confirmation.map_err(unconfirmed_at(&display_path))
    }

    /// Atomically replace one existing regular file beneath this exact root.
    ///
    /// The synchronized stage remains open across the handle-relative rename,
    /// allowing the published pathname to be checked against the exact bytes
    /// and file identity that were prepared. This is suitable for
    /// resolver-owned mutable control files; immutable records continue to use
    /// [`Self::write_new`].
    pub fn replace_existing(
        &self,
        relative_path: &Path,
        bytes: &[u8],
        limits: RecordFileLimits,
    ) -> Result<(), RecordFileError> {
        enforce_byte_limit(bytes.len(), limits)?;
        let file_name = single_file_name(relative_path)?;
        let display_path = self.display_path.join(file_name);
        let destination_metadata =
            io_at(self.directory.symlink_metadata(file_name), &display_path)?;
        if destination_metadata.file_type().is_symlink() || !destination_metadata.is_file() {
            return Err(RecordFileError::NotRegularFile { path: display_path });
        }

        let stage = stage::prepare(self, bytes, limits, None)?;
        stage::replace(self, stage, file_name, &display_path, bytes, limits)
    }
}

#[cfg(unix)]
fn synchronize_directory(
    directory: &CapabilityDirectory,
    display_path: &Path,
) -> Result<(), RecordFileError> {
    io_at(
        io_at(directory.open("."), display_path)?
            .into_std()
            .sync_all(),
        display_path,
    )
}

#[cfg(not(unix))]
fn synchronize_directory(
    _directory: &CapabilityDirectory,
    _display_path: &Path,
) -> Result<(), RecordFileError> {
    Ok(())
}

pub struct RootRecordRead<'root> {
    bytes: Vec<u8>,
    permissions: cap_std::fs::Permissions,
    file: CapabilityFile,
    root: &'root RecordFileRoot,
    file_name: OsString,
    display_path: PathBuf,
}

impl RootRecordRead<'_> {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Replace the observed record, preserving its captured file permissions.
    ///
    /// Old bytes and pathname identity are checked after staging and immediately
    /// before atomic rename. Callers serialize journal operations; this does not
    /// acquire a lock or provide filesystem compare-and-swap against an editor
    /// between the check and rename. A confirmation failure after rename returns
    /// `PublishedButUnconfirmed` because the replacement has already occurred.
    pub fn replace(self, proposed: &[u8], limits: RecordFileLimits) -> Result<(), RecordFileError> {
        let root = self.root;
        self.replace_in(proposed, limits, root)
    }

    /// Replace the observed record using a synchronized stage beneath `staging`.
    /// Uses the checks and permission preservation of [`Self::replace`].
    /// Cross-device rename errors propagate without a copy fallback.
    pub fn replace_in(
        mut self,
        proposed: &[u8],
        limits: RecordFileLimits,
        staging: &RecordFileRoot,
    ) -> Result<(), RecordFileError> {
        let stage = stage::prepare(staging, proposed, limits, Some(self.permissions.clone()))?;
        self.verify_current(limits)?;
        stage::replace(
            self.root,
            stage,
            &self.file_name,
            &self.display_path,
            proposed,
            limits,
        )
    }

    /// Unlink the observed record after checking its bytes and pathname identity.
    ///
    /// Callers serialize journal operations; the check and unlink are not a
    /// filesystem compare-and-swap. Directory synchronization uses the same
    /// platform contract as replacement. If it fails after unlink, the result is
    /// `PublishedButUnconfirmed` and the record has already been removed.
    pub fn remove(mut self, limits: RecordFileLimits) -> Result<(), RecordFileError> {
        self.verify_current(limits)?;
        self.root
            .directory
            .remove_file(&self.file_name)
            .map_err(|error| io_error(&self.display_path, error))?;
        synchronize_directory(&self.root.directory, &self.root.display_path)
            .map_err(unconfirmed_at(&self.display_path))
    }

    pub fn verify_current(&mut self, limits: RecordFileLimits) -> Result<(), RecordFileError> {
        io_at(self.file.seek(SeekFrom::Start(0)), &self.display_path)?;
        let current = read_bytes_bounded(&mut self.file, &self.display_path, limits)?;
        if current != self.bytes {
            return Err(RecordFileError::ContentsChanged {
                path: self.display_path.clone(),
            });
        }
        verify_capability_regular_identity(
            &self.root.directory,
            &self.file_name,
            &self.file,
            &self.display_path,
        )
    }
}

fn single_file_name(path: &Path) -> Result<&OsStr, RecordFileError> {
    let mut components = path.components();
    let Some(std::path::Component::Normal(file_name)) = components.next() else {
        return Err(RecordFileError::InvalidDestination {
            path: path.to_path_buf(),
        });
    };
    if components.next().is_some() {
        return Err(RecordFileError::InvalidDestination {
            path: path.to_path_buf(),
        });
    }
    Ok(file_name)
}

fn verify_capability_regular_identity(
    directory: &CapabilityDirectory,
    relative_path: &OsStr,
    file: &CapabilityFile,
    display_path: &Path,
) -> Result<(), RecordFileError> {
    let handle_metadata = io_at(file.metadata(), display_path)?;
    verify_regular_identity(directory, relative_path, &handle_metadata, display_path)
}

fn verify_regular_identity(
    directory: &CapabilityDirectory,
    relative_path: &OsStr,
    handle_metadata: &cap_std::fs::Metadata,
    display_path: &Path,
) -> Result<(), RecordFileError> {
    let path_metadata = io_at(directory.symlink_metadata(relative_path), display_path)?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || !handle_metadata.is_file()
        || !same_capability_file_identity(&path_metadata, handle_metadata)
    {
        return Err(RecordFileError::NotRegularFile {
            path: display_path.to_path_buf(),
        });
    }
    Ok(())
}

#[cfg(unix)]
fn same_capability_file_identity(
    left: &cap_std::fs::Metadata,
    right: &cap_std::fs::Metadata,
) -> bool {
    use cap_std::fs::MetadataExt;

    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(windows)]
fn same_capability_file_identity(
    left: &cap_std::fs::Metadata,
    right: &cap_std::fs::Metadata,
) -> bool {
    // cap-std exposes these methods only when its nightly feature probe passes.
    // The compatibility extension preserves Option-valued identity; cap-fs-ext's
    // dev/ino convenience methods instead panic when backing identity is absent.
    use cap_primitives::fs::_WindowsByHandle;

    matches!(
        (
            left.volume_serial_number(),
            left.file_index(),
            right.volume_serial_number(),
            right.file_index(),
        ),
        (Some(left_volume), Some(left_index), Some(right_volume), Some(right_index))
            if left_volume == right_volume && left_index == right_index
    )
}

#[cfg(not(any(unix, windows)))]
fn same_capability_file_identity(
    _left: &cap_std::fs::Metadata,
    _right: &cap_std::fs::Metadata,
) -> bool {
    false
}

fn record_error_message(error: RecordFileError) -> String {
    match error {
        RecordFileError::Io { message, .. }
        | RecordFileError::PublishedButUnconfirmed { message, .. } => message,
        other => format!("{other:?}"),
    }
}

fn read_bytes_bounded(
    mut reader: impl Read,
    path: &Path,
    limits: RecordFileLimits,
) -> Result<Vec<u8>, RecordFileError> {
    let mut bytes = Vec::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let count = io_at(reader.read(&mut chunk), path)?;
        if count == 0 {
            break;
        }
        let next_length = bytes
            .len()
            .checked_add(count)
            .ok_or(RecordFileError::LengthOverflow)?;
        enforce_byte_limit(next_length, limits)?;
        bytes
            .try_reserve(count)
            .map_err(|_| RecordFileError::AllocationFailed)?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(bytes)
}

fn enforce_byte_limit(actual: usize, limits: RecordFileLimits) -> Result<(), RecordFileError> {
    if actual > limits.maximum_bytes {
        return Err(RecordFileError::ByteLimitExceeded {
            actual: u64::try_from(actual).unwrap_or(u64::MAX),
            maximum: limits.maximum_bytes,
        });
    }
    Ok(())
}

fn unconfirmed_at(path: &Path) -> impl FnOnce(RecordFileError) -> RecordFileError + '_ {
    move |error| RecordFileError::PublishedButUnconfirmed {
        path: path.to_path_buf(),
        message: record_error_message(error),
    }
}

fn io_at<T>(result: std::io::Result<T>, path: &Path) -> Result<T, RecordFileError> {
    result.map_err(|error| io_error(path, error))
}

fn io_error(path: &Path, error: std::io::Error) -> RecordFileError {
    RecordFileError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
