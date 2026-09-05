//! Shared synchronized stages and atomic replacement publication.

use super::*;
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::Permissions;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

const MAXIMUM_STAGE_ATTEMPTS: u64 = 256;
static NEXT_STAGE_ID: AtomicU64 = AtomicU64::new(0);

pub(super) struct PendingCapabilityRecord {
    pub(super) directory: CapabilityDirectory,
    pub(super) file_name: OsString,
    pub(super) display_path: PathBuf,
    pub(super) file: CapabilityFile,
    removed: bool,
}

impl PendingCapabilityRecord {
    pub(super) fn remove(&mut self) -> Result<(), RecordFileError> {
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

pub(super) fn prepare(
    root: &RecordFileRoot,
    bytes: &[u8],
    limits: RecordFileLimits,
    permissions: Option<Permissions>,
) -> Result<PendingCapabilityRecord, RecordFileError> {
    if bytes.len() > limits.maximum_bytes {
        return Err(RecordFileError::ByteLimitExceeded {
            actual: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            maximum: limits.maximum_bytes,
        });
    }
    let mut stage = create_exclusive_capability_stage(&root.directory, &root.display_path)?;
    stage
        .file
        .write_all(bytes)
        .map_err(|error| io_error(&stage.display_path, error))?;
    // Apply captured permissions after writing, since a write can clear mode bits.
    if let Some(permissions) = permissions {
        stage
            .file
            .set_permissions(permissions)
            .map_err(|error| io_error(&stage.display_path, error))?;
    }
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
        &root.directory,
        &stage.file_name,
        &stage.file,
        &stage.display_path,
    )?;
    Ok(stage)
}

pub(super) fn replace(
    root: &RecordFileRoot,
    mut stage: PendingCapabilityRecord,
    file_name: &OsStr,
    display_path: &Path,
    bytes: &[u8],
    limits: RecordFileLimits,
) -> Result<(), RecordFileError> {
    stage
        .directory
        .rename(&stage.file_name, &root.directory, file_name)
        .map_err(|error| io_error(display_path, error))?;
    stage.removed = true;

    let confirmation = (|| {
        verify_capability_regular_identity(&root.directory, file_name, &stage.file, display_path)?;
        synchronize_directory(&root.directory, &root.display_path)?;
        synchronize_directory(
            &stage.directory,
            stage
                .display_path
                .parent()
                .expect("stage has a parent directory"),
        )?;
        stage
            .file
            .seek(SeekFrom::Start(0))
            .map_err(|error| io_error(display_path, error))?;
        let published = read_bytes_bounded(&mut stage.file, display_path, limits)?;
        if published != bytes {
            return Err(RecordFileError::ContentsChanged {
                path: display_path.to_path_buf(),
            });
        }
        verify_capability_regular_identity(&root.directory, file_name, &stage.file, display_path)
    })();

    confirmation.map_err(|error| RecordFileError::PublishedButUnconfirmed {
        path: display_path.to_path_buf(),
        message: record_error_message(error),
    })
}

fn create_exclusive_capability_stage(
    directory: &CapabilityDirectory,
    display_directory: &Path,
) -> Result<PendingCapabilityRecord, RecordFileError> {
    // Clone before creation so a failed clone cannot leave an unguarded stage.
    let directory = directory
        .try_clone()
        .map_err(|error| io_error(display_directory, error))?;
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
                    directory,
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
