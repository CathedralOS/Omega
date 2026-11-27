//! Installed Filesystem handles retain one admitted reader until close.
use super::*;
use std::io::ErrorKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildFileError {
    BadDescriptor,
    Io(ErrorKind),
}

impl BuildFilesystemRuntime {
    pub fn open_file(&mut self, path: &[u8]) -> Result<Result<u64, ErrorKind>, String> {
        if path.starts_with(b"/") {
            let file = match self.mapped_file(
                path,
                FilesystemGrantAccess::Read,
                FilesystemHostOperation::Open,
            )? {
                Ok(file) => file,
                Err(kind) => return Ok(Err(kind)),
            };
            let reader = match file.directory.open_read(file.suffix) {
                Ok(reader) => reader,
                Err(error) => return Ok(Err(self.mapped_error(file.ordinal, error))),
            };
            return self
                .retain_reader(
                    file.ordinal,
                    BuildFilesystemRoot::MappedDirectories,
                    file.guest_relative,
                    reader,
                )
                .map(|fd| Ok(fd as u64));
        }
        if !filesystem_authority::source_snapshot_path_is_canonical(path, false) {
            return Ok(Err(ErrorKind::InvalidInput));
        }
        // The package remains read-only even if an explicitly granted Output
        // operation used the same spelling in the independent staging tree.
        let absent = matches!(
            self.snapshot(BUILD_SOURCE_ROOT_IDENTITY)?.lookup(path),
            Err(build_output::SourceSnapshotError::NotFound)
        );
        let has_writer = |file: &OpenFile| {
            matches!(file.storage, FileStorage::Output { .. }) && file.path == path
        };
        let root = if absent && self.sealed.iter().any(|(name, _)| name == path) {
            BUILD_OUTPUT_ROOT_IDENTITY
        } else if absent && self.descriptors.iter().flatten().any(has_writer) {
            return Ok(Err(ErrorKind::PermissionDenied));
        } else {
            BUILD_SOURCE_ROOT_IDENTITY
        };
        let fd = self.open(root, path, 0)?;
        Ok(if fd >= 0 {
            Ok(fd as u64)
        } else {
            Err(self.last_io_error()?)
        })
    }

    fn last_io_error(&self) -> Result<ErrorKind, String> {
        let errno = self
            .effects
            .attempts
            .last()
            .ok_or("missing filesystem failure observation")?
            .post_error;
        Ok(match errno {
            2 => ErrorKind::NotFound,
            13 | 40 => ErrorKind::PermissionDenied,
            20 | 22 => ErrorKind::InvalidInput,
            21 => ErrorKind::IsADirectory,
            code => std::io::Error::from_raw_os_error(code).kind(),
        })
    }

    fn reader_slot(&self, handle: u64) -> Option<(usize, i32)> {
        let fd = i32::try_from(handle).ok()?;
        let slot = usize::try_from(fd.checked_sub(3)?).ok()?;
        let row = self.descriptors.get(slot)?.as_ref()?;
        matches!(row.storage, FileStorage::Reader(_)).then_some((slot, fd))
    }

    fn bad_descriptor(
        &mut self,
        operation: FilesystemHostOperation,
    ) -> Result<BuildFileError, String> {
        let ordinal = self.begin_observation(
            operation,
            FilesystemObservationProvider::Virtual,
            None,
            Some(FilesystemLogicalHandleInputResolution::Unknown),
        )?;
        self.complete_attempt(ordinal, -1, 9);
        Ok(BuildFileError::BadDescriptor)
    }

    pub fn read_handle(
        &mut self,
        handle: u64,
        buffer: &mut [u8],
        count: u64,
    ) -> Result<Result<u64, BuildFileError>, String> {
        let Some((slot, fd)) = self.reader_slot(handle) else {
            return Ok(Err(self.bad_descriptor(FilesystemHostOperation::Read)?));
        };
        if count > buffer.len() as u64 {
            return Ok(Err(BuildFileError::Io(ErrorKind::InvalidInput)));
        }
        let read = self.read_slot(slot, fd, buffer, count)?;
        Ok(if read >= 0 {
            Ok(read as u64)
        } else {
            Err(BuildFileError::Io(self.last_io_error()?))
        })
    }

    pub fn close_handle(&mut self, handle: u64) -> Result<Result<(), BuildFileError>, String> {
        let Some((slot, fd)) = self.reader_slot(handle) else {
            return Ok(Err(self.bad_descriptor(FilesystemHostOperation::Close)?));
        };
        self.close_slot(slot, fd)?;
        Ok(Ok(()))
    }
}
