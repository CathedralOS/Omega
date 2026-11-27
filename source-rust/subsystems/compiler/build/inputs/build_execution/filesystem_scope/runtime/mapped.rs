//! Resolve absolute guest paths against this activation’s directory grants.
use super::super::runtime_directory::BuildDirectory;
use super::*;

pub(super) struct MappedFile<'path> {
    pub ordinal: usize,
    pub directory: BuildDirectory,
    pub suffix: &'path [u8],
    pub guest_relative: &'path [u8],
}

impl BuildFilesystemRuntime {
    pub(super) fn mapped_file<'path>(
        &mut self,
        path: &'path [u8],
        access: FilesystemGrantAccess,
        operation: FilesystemHostOperation,
    ) -> Result<Result<MappedFile<'path>, std::io::ErrorKind>, String> {
        use platform_custody::directory_mappings::DirectoryAccess;
        use std::io::ErrorKind;
        let relative = path
            .strip_prefix(b"/")
            .ok_or("mapped path must be absolute")?;
        let mapping = self
            .effects
            .scope
            .directories
            .resolve_file(path)
            .map(|(mapping, file)| (mapping.backing().clone(), mapping.access(), file));
        let provider = mapping.as_ref().map_or(
            FilesystemObservationProvider::RealScoped,
            |(directory, _, _)| directory.provider(),
        );
        let ordinal = self.begin_path_attempt(
            operation,
            BuildFilesystemRoot::MappedDirectories,
            provider,
            relative,
            access,
            None,
        )?;
        match mapping {
            Ok((directory, granted, suffix))
                if access == FilesystemGrantAccess::Read
                    || granted == DirectoryAccess::ReadWrite =>
            {
                Ok(Ok(MappedFile {
                    ordinal,
                    directory,
                    suffix,
                    guest_relative: relative,
                }))
            }
            Err(error) if error.kind() != ErrorKind::PermissionDenied => {
                Ok(Err(self.mapped_error(ordinal, error)))
            }
            _ => {
                self.refuse_access(ordinal, access);
                Ok(Err(ErrorKind::PermissionDenied))
            }
        }
    }
    pub(super) fn mapped_error(
        &mut self,
        ordinal: usize,
        error: std::io::Error,
    ) -> std::io::ErrorKind {
        let kind = error.kind();
        let errno = match kind {
            std::io::ErrorKind::NotFound => 2,
            std::io::ErrorKind::PermissionDenied => 13,
            std::io::ErrorKind::NotADirectory => 20,
            std::io::ErrorKind::IsADirectory => 21,
            std::io::ErrorKind::InvalidInput => 22,
            _ => error.raw_os_error().unwrap_or(5),
        };
        self.complete_attempt(ordinal, -1, errno);
        kind
    }
    /// An absolute path is resolved exclusively within this activation's caller-issued
    /// directories. Reads and writes still consume the evaluated call's work budget.
    pub(super) fn mapped_operation(
        &mut self,
        path: &[u8],
        access: FilesystemGrantAccess,
        operation: FilesystemHostOperation,
        perform: impl FnOnce(
            &BuildDirectory,
            &[u8],
            &EvaluationInvocationAccount,
        ) -> Result<std::io::Result<usize>, String>,
    ) -> Result<Result<usize, std::io::ErrorKind>, String> {
        let resolved = match self.mapped_file(path, access, operation)? {
            Ok(file) => file,
            Err(kind) => return Ok(Err(kind)),
        };
        let ordinal = resolved.ordinal;
        let result = perform(&resolved.directory, resolved.suffix, &self.account)?;
        match result {
            Ok(count) => {
                self.complete_attempt(
                    ordinal,
                    i64::try_from(count).map_err(|_| "mapped size exceeds i64")?,
                    0,
                );
                Ok(Ok(count))
            }
            Err(error) => Ok(Err(self.mapped_error(ordinal, error))),
        }
    }
}
