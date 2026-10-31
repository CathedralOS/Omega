//! Shared synchronized stages and atomic replacement publication.
use super::{
    CapabilityDirectory, CapabilityFile, CapabilityOpenOptions, FollowSymlinks, OsStr, OsString,
    Path, PathBuf, RecordFileError, RecordFileLimits, RecordFileRoot, SeekFrom, enforce_byte_limit,
    io_at, io_error, read_bytes_bounded, synchronize_directory, unconfirmed_at,
};
use cap_fs_ext::OpenOptionsFollowExt;
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::Permissions;
use std::io::Seek;
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
    enforce_byte_limit(bytes.len(), limits)?;
    let mut stage = create_exclusive_capability_stage(&root.directory, &root.display_path)?;
    io_at(stage.file.write_all(bytes), &stage.display_path)?;
    // Apply captured permissions after writing, since a write can clear mode bits.
    if let Some(permissions) = permissions {
        io_at(stage.file.set_permissions(permissions), &stage.display_path)?;
    }
    io_at(stage.file.sync_all(), &stage.display_path)?;
    io_at(stage.file.seek(SeekFrom::Start(0)), &stage.display_path)?;
    let staged = read_bytes_bounded(&mut stage.file, &stage.display_path, limits)?;
    if staged != bytes {
        return Err(RecordFileError::ContentsChanged {
            path: stage.display_path.clone(),
        });
    }
    root.verify_regular(&stage.file_name, &stage.file)?;
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
        root.verify_regular(file_name, &stage.file)?;
        synchronize_directory(&root.directory, &root.display_path)?;
        synchronize_directory(
            &stage.directory,
            stage
                .display_path
                .parent()
                .expect("stage has a parent directory"),
        )?;
        io_at(stage.file.seek(SeekFrom::Start(0)), display_path)?;
        let published = read_bytes_bounded(&mut stage.file, display_path, limits)?;
        if published != bytes {
            return Err(RecordFileError::ContentsChanged {
                path: display_path.to_path_buf(),
            });
        }
        root.verify_regular(file_name, &stage.file)
    })();

    confirmation.map_err(unconfirmed_at(display_path))
}

fn create_exclusive_capability_stage(
    directory: &CapabilityDirectory,
    display_directory: &Path,
) -> Result<PendingCapabilityRecord, RecordFileError> {
    // Clone before creation so a failed clone cannot leave an unguarded stage.
    let directory = io_at(directory.try_clone(), display_directory)?;
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
