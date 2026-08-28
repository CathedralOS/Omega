use cap_fs_ext::{FollowSymlinks, OpenOptionsFollowExt};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt as CapabilityOpenOptionsExt;
use cap_std::fs::{
    Dir as CapabilityDirectory, File as CapabilityFile, OpenOptions as CapabilityOpenOptions,
};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAXIMUM_STAGE_ATTEMPTS: u64 = 256;
static NEXT_STAGE_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) fn is_portable_record_file_name(value: &str, maximum_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum_bytes
        && !value.contains('/')
        && !value.contains('\\')
        && !value.bytes().any(|byte| byte.is_ascii_control())
        && !matches!(value, "." | "..")
        && !value.ends_with('.')
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
        && !is_windows_reserved_path_component(value)
}

fn is_windows_reserved_path_component(component: &str) -> bool {
    let stem = component.split('.').next().unwrap_or(component);
    let uppercase = stem.to_ascii_uppercase();
    matches!(uppercase.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || uppercase
            .strip_prefix("COM")
            .or_else(|| uppercase.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordFileLimits {
    pub(crate) maximum_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecordFileError {
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
pub(crate) struct RecordFileRoot {
    display_path: PathBuf,
    directory: CapabilityDirectory,
}

impl RecordFileRoot {
    pub(crate) fn from_directory(
        directory: CapabilityDirectory,
        display_path: PathBuf,
    ) -> Result<Self, RecordFileError> {
        let metadata = directory
            .dir_metadata()
            .map_err(|error| io_error(&display_path, error))?;
        if !metadata.is_dir() {
            return Err(RecordFileError::NotRegularFile { path: display_path });
        }
        Ok(Self {
            display_path,
            directory,
        })
    }

    pub(crate) fn read(
        &self,
        relative_path: &Path,
        limits: RecordFileLimits,
    ) -> Result<RootRecordRead<'_>, RecordFileError> {
        let file_name = single_file_name(relative_path)?;
        let display_path = self.display_path.join(file_name);
        let path_metadata = self
            .directory
            .symlink_metadata(file_name)
            .map_err(|error| io_error(&display_path, error))?;
        if path_metadata.file_type().is_symlink() || !path_metadata.is_file() {
            return Err(RecordFileError::NotRegularFile { path: display_path });
        }
        let mut options = CapabilityOpenOptions::new();
        options.read(true).follow(FollowSymlinks::No);
        let file = self
            .directory
            .open_with(file_name, &options)
            .map_err(|error| io_error(&display_path, error))?;
        verify_capability_regular_identity(&self.directory, file_name, &file, &display_path)?;
        let metadata = file
            .metadata()
            .map_err(|error| io_error(&display_path, error))?;
        if metadata.len() > u64::try_from(limits.maximum_bytes).unwrap_or(u64::MAX) {
            return Err(RecordFileError::ByteLimitExceeded {
                actual: metadata.len(),
                maximum: limits.maximum_bytes,
            });
        }
        let bytes = read_bytes_bounded(&file, &display_path, limits)?;
        let mut read = RootRecordRead {
            bytes,
            file,
            root: self,
            file_name: file_name.to_os_string(),
            display_path,
        };
        read.verify_current(limits)?;
        Ok(read)
    }

    pub(crate) fn write_new(
        &self,
        relative_path: &Path,
        bytes: &[u8],
        limits: RecordFileLimits,
    ) -> Result<(), RecordFileError> {
        if bytes.len() > limits.maximum_bytes {
            return Err(RecordFileError::ByteLimitExceeded {
                actual: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                maximum: limits.maximum_bytes,
            });
        }
        let file_name = single_file_name(relative_path)?;
        let display_path = self.display_path.join(file_name);
        let mut stage = create_exclusive_capability_stage(&self.directory, &self.display_path)?;
        stage
            .file
            .write_all(bytes)
            .map_err(|error| io_error(&stage.display_path, error))?;
        stage
            .file
            .sync_all()
            .map_err(|error| io_error(&stage.display_path, error))?;
        stage
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|error| io_error(&stage.display_path, error))?;
        let staged = read_bytes_bounded(&mut stage.file, &stage.display_path, limits)?;
        if staged != bytes {
            return Err(RecordFileError::ContentsChanged {
                path: stage.display_path.clone(),
            });
        }
        verify_capability_regular_identity(
            &self.directory,
            &stage.file_name,
            &stage.file,
            &stage.display_path,
        )?;

        match self
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
            self.directory
                .try_clone()
                .map_err(|error| io_error(&self.display_path, error))?
                .into_std_file()
                .sync_all()
                .map_err(|error| io_error(&self.display_path, error))?;
            verify_capability_regular_identity(
                &self.directory,
                file_name,
                &stage.file,
                &display_path,
            )
        })();

        confirmation.map_err(|error| RecordFileError::PublishedButUnconfirmed {
            path: display_path,
            message: record_error_message(error),
        })
    }

    /// Atomically replace one existing regular file beneath this exact root.
    ///
    /// The synchronized stage remains open across the handle-relative rename,
    /// allowing the published pathname to be checked against the exact bytes
    /// and file identity that were prepared. This is suitable for
    /// resolver-owned mutable control files; immutable records continue to use
    /// [`Self::write_new`].
    pub(crate) fn replace_existing(
        &self,
        relative_path: &Path,
        bytes: &[u8],
        limits: RecordFileLimits,
    ) -> Result<(), RecordFileError> {
        if bytes.len() > limits.maximum_bytes {
            return Err(RecordFileError::ByteLimitExceeded {
                actual: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                maximum: limits.maximum_bytes,
            });
        }
        let file_name = single_file_name(relative_path)?;
        let display_path = self.display_path.join(file_name);
        let destination_metadata = self
            .directory
            .symlink_metadata(file_name)
            .map_err(|error| io_error(&display_path, error))?;
        if destination_metadata.file_type().is_symlink() || !destination_metadata.is_file() {
            return Err(RecordFileError::NotRegularFile { path: display_path });
        }

        let mut stage = create_exclusive_capability_stage(&self.directory, &self.display_path)?;
        stage
            .file
            .write_all(bytes)
            .map_err(|error| io_error(&stage.display_path, error))?;
        stage
            .file
            .sync_all()
            .map_err(|error| io_error(&stage.display_path, error))?;
        stage
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|error| io_error(&stage.display_path, error))?;
        let staged = read_bytes_bounded(&mut stage.file, &stage.display_path, limits)?;
        if staged != bytes {
            return Err(RecordFileError::ContentsChanged {
                path: stage.display_path.clone(),
            });
        }
        verify_capability_regular_identity(
            &self.directory,
            &stage.file_name,
            &stage.file,
            &stage.display_path,
        )?;

        self.directory
            .rename(&stage.file_name, &self.directory, file_name)
            .map_err(|error| io_error(&display_path, error))?;
        stage.removed = true;

        let confirmation = (|| {
            verify_capability_regular_identity(
                &self.directory,
                file_name,
                &stage.file,
                &display_path,
            )?;
            self.directory
                .try_clone()
                .map_err(|error| io_error(&self.display_path, error))?
                .into_std_file()
                .sync_all()
                .map_err(|error| io_error(&self.display_path, error))?;
            stage
                .file
                .seek(SeekFrom::Start(0))
                .map_err(|error| io_error(&display_path, error))?;
            let published = read_bytes_bounded(&mut stage.file, &display_path, limits)?;
            if published != bytes {
                return Err(RecordFileError::ContentsChanged {
                    path: display_path.clone(),
                });
            }
            verify_capability_regular_identity(
                &self.directory,
                file_name,
                &stage.file,
                &display_path,
            )
        })();

        confirmation.map_err(|error| RecordFileError::PublishedButUnconfirmed {
            path: display_path,
            message: record_error_message(error),
        })
    }
}

pub(crate) struct RootRecordRead<'root> {
    bytes: Vec<u8>,
    file: CapabilityFile,
    root: &'root RecordFileRoot,
    file_name: OsString,
    display_path: PathBuf,
}

impl RootRecordRead<'_> {
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn verify_current(
        &mut self,
        limits: RecordFileLimits,
    ) -> Result<(), RecordFileError> {
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| io_error(&self.display_path, error))?;
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

struct PendingCapabilityRecord {
    directory: CapabilityDirectory,
    file_name: OsString,
    display_path: PathBuf,
    file: CapabilityFile,
    removed: bool,
}

impl PendingCapabilityRecord {
    fn remove(&mut self) -> Result<(), RecordFileError> {
        self.directory
            .remove_file(&self.file_name)
            .map_err(|error| io_error(&self.display_path, error))?;
        self.removed = true;
        Ok(())
    }
}

impl Drop for PendingCapabilityRecord {
    fn drop(&mut self) {
        if !self.removed {
            let _ = self.directory.remove_file(&self.file_name);
        }
    }
}

fn create_exclusive_capability_stage(
    directory: &CapabilityDirectory,
    display_directory: &Path,
) -> Result<PendingCapabilityRecord, RecordFileError> {
    for _ in 0..MAXIMUM_STAGE_ATTEMPTS {
        let stage_id = NEXT_STAGE_ID.fetch_add(1, Ordering::Relaxed);
        let file_name = OsString::from(format!(
            ".omega-record-stage-{}-{stage_id}",
            std::process::id()
        ));
        let display_path = display_directory.join(&file_name);
        let mut options = CapabilityOpenOptions::new();
        options
            .read(true)
            .write(true)
            .create_new(true)
            .follow(FollowSymlinks::No);
        #[cfg(unix)]
        options.mode(0o600);
        match directory.open_with(&file_name, &options) {
            Ok(file) => {
                return Ok(PendingCapabilityRecord {
                    directory: directory
                        .try_clone()
                        .map_err(|error| io_error(display_directory, error))?,
                    file_name,
                    display_path,
                    file,
                    removed: false,
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(&display_path, error)),
        }
    }
    Err(RecordFileError::StageNameSpaceExhausted {
        directory: display_directory.to_path_buf(),
    })
}

fn verify_capability_regular_identity(
    directory: &CapabilityDirectory,
    relative_path: &OsStr,
    file: &CapabilityFile,
    display_path: &Path,
) -> Result<(), RecordFileError> {
    let path_metadata = directory
        .symlink_metadata(relative_path)
        .map_err(|error| io_error(display_path, error))?;
    let handle_metadata = file
        .metadata()
        .map_err(|error| io_error(display_path, error))?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || !handle_metadata.is_file()
        || !same_capability_file_identity(&path_metadata, &handle_metadata)
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
    use cap_std::fs::MetadataExt;

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
        let count = reader
            .read(&mut chunk)
            .map_err(|error| io_error(path, error))?;
        if count == 0 {
            break;
        }
        let next_length = bytes
            .len()
            .checked_add(count)
            .ok_or(RecordFileError::LengthOverflow)?;
        if next_length > limits.maximum_bytes {
            return Err(RecordFileError::ByteLimitExceeded {
                actual: u64::try_from(next_length).unwrap_or(u64::MAX),
                maximum: limits.maximum_bytes,
            });
        }
        bytes
            .try_reserve(count)
            .map_err(|_| RecordFileError::AllocationFailed)?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    Ok(bytes)
}

fn io_error(path: &Path, error: std::io::Error) -> RecordFileError {
    RecordFileError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_std::{ambient_authority, fs::Dir};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn retained_read_detects_in_place_content_change() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should follow the Unix epoch")
            .as_nanos();
        let directory_path = std::env::temp_dir().join(format!(
            "omega-root-record-content-change-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&directory_path).expect("create policy directory");
        let file_path = directory_path.join("policy.record");
        fs::write(&file_path, b"first").expect("write initial record");
        let directory = Dir::open_ambient_dir(&directory_path, ambient_authority())
            .expect("open policy directory capability");
        let root = RecordFileRoot::from_directory(directory, directory_path.clone())
            .expect("bind policy directory capability");
        let limits = RecordFileLimits { maximum_bytes: 5 };
        let mut read = root
            .read(Path::new("policy.record"), limits)
            .expect("read initial record");

        fs::write(&file_path, b"other").expect("replace bytes in the same file");

        assert!(matches!(
            read.verify_current(limits),
            Err(RecordFileError::ContentsChanged { .. })
        ));
        let _ = fs::remove_dir_all(directory_path);
    }

    #[test]
    fn rooted_replacement_publishes_exact_synchronized_bytes_without_a_stage_residue() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should follow the Unix epoch")
            .as_nanos();
        let directory_path = std::env::temp_dir().join(format!(
            "omega-root-record-replacement-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&directory_path).expect("create record directory");
        fs::write(directory_path.join("config"), b"mutable helper bytes")
            .expect("write existing control file");
        let directory = Dir::open_ambient_dir(&directory_path, ambient_authority())
            .expect("open record directory capability");
        let root = RecordFileRoot::from_directory(directory, directory_path.clone())
            .expect("bind record directory capability");

        root.replace_existing(
            Path::new("config"),
            b"canonical bytes",
            RecordFileLimits { maximum_bytes: 64 },
        )
        .expect("replace exact control file");

        assert_eq!(
            fs::read(directory_path.join("config")).expect("read replacement"),
            b"canonical bytes"
        );
        assert!(
            fs::read_dir(&directory_path)
                .expect("list record directory")
                .all(|entry| !entry
                    .expect("record entry")
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".omega-record-stage-"))
        );
        let _ = fs::remove_dir_all(directory_path);
    }

    #[cfg(unix)]
    #[test]
    fn rooted_replacement_rejects_a_symlink_destination_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should follow the Unix epoch")
            .as_nanos();
        let directory_path = std::env::temp_dir().join(format!(
            "omega-root-record-replacement-link-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&directory_path).expect("create record directory");
        let target = directory_path.join("target");
        fs::write(&target, b"outside replacement").expect("write symlink target");
        symlink("target", directory_path.join("config")).expect("create destination symlink");
        let directory = Dir::open_ambient_dir(&directory_path, ambient_authority())
            .expect("open record directory capability");
        let root = RecordFileRoot::from_directory(directory, directory_path.clone())
            .expect("bind record directory capability");

        assert!(matches!(
            root.replace_existing(
                Path::new("config"),
                b"canonical bytes",
                RecordFileLimits { maximum_bytes: 64 },
            ),
            Err(RecordFileError::NotRegularFile { .. })
        ));
        assert_eq!(
            fs::read(target).expect("read symlink target"),
            b"outside replacement"
        );
        let _ = fs::remove_dir_all(directory_path);
    }
}
