use super::EACCES;
use super::io_errno;
use crate::interpreter::evaluator::{EvalResult, PreparedByteOutput};
use crate::{CanonicalFilesystemMetadataRowKind, FilesystemMetadataObservationKind};

pub(super) enum SelectedFilesystemMetadata {
    Canonical(CanonicalFilesystemMetadataRowKind),
    Physical(std::fs::Metadata),
}

pub(super) fn canonical_metadata_values(
    metadata: CanonicalFilesystemMetadataRowKind,
) -> (u32, i64, i64) {
    let (mode, size) = match metadata {
        CanonicalFilesystemMetadataRowKind::Directory => (0o040555, 0),
        CanonicalFilesystemMetadataRowKind::File {
            executable,
            logical_byte_length,
        } => (
            if executable { 0o100555 } else { 0o100444 },
            logical_byte_length as i64,
        ),
        CanonicalFilesystemMetadataRowKind::Symlink {
            target_spelling_logical_byte_length,
        } => (0o120777, target_spelling_logical_byte_length as i64),
    };
    (mode, size, 1_000_000_000)
}

pub(super) fn host_metadata_matches_canonical_kind(
    metadata: &std::fs::Metadata,
    canonical: CanonicalFilesystemMetadataRowKind,
) -> bool {
    match canonical {
        CanonicalFilesystemMetadataRowKind::Directory => metadata.is_dir(),
        CanonicalFilesystemMetadataRowKind::File { .. } => metadata.is_file(),
        CanonicalFilesystemMetadataRowKind::Symlink { .. } => metadata.file_type().is_symlink(),
    }
}

impl<'program> crate::interpreter::evaluator::Evaluator<'program> {
    pub(super) fn real_read_metadata(
        &mut self,
        path: Vec<u8>,
        buffer: &PreparedByteOutput,
        kind: FilesystemMetadataObservationKind,
    ) -> EvalResult<i64> {
        let no_follow = kind == FilesystemMetadataObservationKind::UnfollowedFinalPath;
        let authorized = if no_follow {
            self.authorized_path_no_follow(&path, false, 0)
        } else {
            self.authorized_path(&path, false, 0)
        };
        let Some(path) = authorized else {
            return Ok(-1);
        };
        let looked_up = if no_follow {
            std::fs::symlink_metadata(&path)
        } else {
            std::fs::metadata(&path)
        };
        match looked_up {
            Ok(metadata) => {
                let selected = self
                    .real_fs
                    .as_ref()
                    .expect("real metadata calls have a real provider")
                    // No-follow authorization already returned the
                    // parent-resolved authored leaf coordinate; followed
                    // authorization returned the canonical target.
                    .canonical_metadata_for_path(&path);
                match selected {
                    Ok(Some(canonical))
                        if host_metadata_matches_canonical_kind(&metadata, canonical) =>
                    {
                        self.write_canonical_fs_stat(buffer, kind, canonical)?;
                    }
                    Ok(Some(_)) | Err(_) => {
                        self.real_fs_mut().errno = EACCES;
                        return Ok(-1);
                    }
                    Ok(None) => self.write_real_fs_stat(buffer, kind, &metadata)?,
                }
                Ok(0)
            }
            Err(error) => {
                self.real_fs_mut().errno = io_errno(&error);
                Ok(-1)
            }
        }
    }

    pub(super) fn real_change_owner_no_follow(&mut self, path: Vec<u8>, uid: i32, gid: i32) -> i64 {
        let Some(path) = self.authorized_path_no_follow(&path, true, 0) else {
            return -1;
        };
        #[cfg(unix)]
        {
            let owner = (uid >= 0).then_some(uid as u32);
            let group = (gid >= 0).then_some(gid as u32);
            self.real_result_unit(std::os::unix::fs::lchown(path, owner, group))
        }
        #[cfg(not(unix))]
        {
            let _ = (path, uid, gid);
            self.real_fs_mut().errno = super::ENOTSUP;
            -1
        }
    }
    /// canonical observation row. Native-field parity beyond mode, size, and
    /// mtime remains separate work; modeled fields are explicit rather than
    /// accidental host-layout residue.
    pub(super) fn write_real_fs_stat(
        &mut self,
        output: &PreparedByteOutput,
        kind: FilesystemMetadataObservationKind,
        metadata: &std::fs::Metadata,
    ) -> EvalResult<()> {
        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            let type_bits = if metadata.is_dir() {
                0o040000
            } else if metadata.file_type().is_symlink() {
                0o120000
            } else {
                0o100000
            };
            (type_bits | (metadata.permissions().mode() & 0o7777)) as u16
        };
        #[cfg(not(unix))]
        let mode: u16 = if metadata.is_dir() {
            0o040000
        } else {
            0o100000
        };
        let size = metadata.len() as i64;
        let mtime_secs = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_secs() as i64)
            .unwrap_or(0);
        self.write_fs_stat(output, kind, u32::from(mode), size, mtime_secs)
    }

    pub(super) fn write_canonical_fs_stat(
        &mut self,
        output: &PreparedByteOutput,
        kind: FilesystemMetadataObservationKind,
        metadata: CanonicalFilesystemMetadataRowKind,
    ) -> EvalResult<()> {
        let (mode, size, modification_time) = canonical_metadata_values(metadata);
        self.write_fs_stat(output, kind, mode, size, modification_time)
    }
}
