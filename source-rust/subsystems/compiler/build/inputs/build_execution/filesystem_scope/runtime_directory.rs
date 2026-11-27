//! Extra runtime directories can retain a live handle or a read-only subtree
//! of the package bytes admitted to the parent Build.
use super::BuildMachineFilesystemScope;
use build_output::{CapturedBuildSourceInput, SourceSnapshotEntry, SourceSnapshotError};
use build_model::accounting::EvaluationInvocationAccount;
use filesystem_authority::FilesystemObservationProvider;
use platform_custody::{bound_directory::BoundDirectory, directory_mappings::DirectoryMappings};
use std::io::{self, Read, Write};
use std::sync::Arc;

pub type BuildDirectoryMappings = DirectoryMappings<BuildDirectory>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildDirectory(Backing);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Backing {
    Live(BoundDirectory),
    Staging {
        directory: BoundDirectory,
        scope: Arc<BuildMachineFilesystemScope>,
    },
    Captured {
        source: CapturedBuildSourceInput,
        prefix: Vec<u8>,
    },
}

impl BuildDirectory {
    pub(crate) fn caller(directory: BoundDirectory) -> Self {
        Self(Backing::Live(directory))
    }

    pub(super) fn output(directory: BoundDirectory, scope: BuildMachineFilesystemScope) -> Self {
        Self(Backing::Staging {
            directory,
            scope: Arc::new(scope),
        })
    }

    pub(super) fn captured(source: &CapturedBuildSourceInput, path: &[u8]) -> io::Result<Self> {
        if source.lookup(path).map_err(snapshot_error)? != SourceSnapshotEntry::Directory {
            return Err(io::ErrorKind::NotADirectory.into());
        }
        Ok(Self(Backing::Captured {
            source: source.clone(),
            prefix: path.into(),
        }))
    }

    pub(super) fn subtree(&self, relative: &[u8]) -> io::Result<Self> {
        if relative.is_empty() {
            return Ok(self.clone());
        }
        match &self.0 {
            Backing::Live(directory) => Ok(Self::caller(directory.subtree(relative)?)),
            Backing::Captured { source, prefix } => {
                let mut path = prefix.clone();
                if !path.is_empty() {
                    path.push(b'/');
                }
                path.extend_from_slice(relative);
                Self::captured(source, &path)
            }
            Backing::Staging { directory, scope } => {
                let directory = directory.subtree(relative)?;
                let mut scope = (**scope).clone();
                scope
                    .build_dir
                    .push(std::str::from_utf8(relative).map_err(|_| io::ErrorKind::InvalidInput)?);
                Ok(Self::output(directory, scope))
            }
        }
    }

    pub(super) fn provider(&self) -> FilesystemObservationProvider {
        match self.0 {
            Backing::Captured { .. } => FilesystemObservationProvider::Captured,
            _ => FilesystemObservationProvider::RealScoped,
        }
    }

    pub(super) fn read(&self, path: &[u8], buffer: &mut [u8]) -> io::Result<usize> {
        match &self.0 {
            Backing::Live(directory) | Backing::Staging { directory, .. } => {
                directory.open_read(path)?.read(buffer)
            }
            Backing::Captured { source, prefix } => {
                let combined;
                let path = if prefix.is_empty() {
                    path
                } else {
                    combined = [prefix.as_slice(), b"/", path].concat();
                    &combined
                };
                let bytes = source
                    .read(path, 0, buffer.len() as u64)
                    .map_err(snapshot_error)?;
                buffer[..bytes.len()].copy_from_slice(bytes);
                Ok(bytes.len())
            }
        }
    }

    pub(super) fn write(
        &self,
        account: &EvaluationInvocationAccount,
        path: &[u8],
        bytes: &[u8],
    ) -> Result<io::Result<usize>, String> {
        let result = match &self.0 {
            Backing::Live(directory) => directory
                .open_write(path)
                .and_then(|mut f| f.write_all(bytes)),
            Backing::Staging { scope, directory } => scope
                .runtime_at(directory.clone(), account.clone())
                .write_staged_file(path, bytes)?
                .map_err(io::Error::from),
            Backing::Captured { .. } => Err(io::ErrorKind::PermissionDenied.into()),
        };
        Ok(result.map(|()| bytes.len()))
    }

    pub(super) fn create_dir(
        &self,
        account: &EvaluationInvocationAccount,
        path: &[u8],
    ) -> Result<io::Result<usize>, String> {
        let result = match &self.0 {
            Backing::Live(directory) => directory.create_dir(path),
            Backing::Staging { scope, directory } => scope
                .runtime_at(directory.clone(), account.clone())
                .create_staged_directory(path)?
                .map_err(io::Error::from),
            Backing::Captured { .. } => Err(io::ErrorKind::PermissionDenied.into()),
        };
        Ok(result.map(|()| 0))
    }
}

fn snapshot_error(error: SourceSnapshotError) -> io::Error {
    use io::ErrorKind;
    match error {
        SourceSnapshotError::NotFound => ErrorKind::NotFound,
        SourceSnapshotError::NotDirectory => ErrorKind::NotADirectory,
        SourceSnapshotError::NotFile => ErrorKind::IsADirectory,
        SourceSnapshotError::SymlinkTraversal => ErrorKind::PermissionDenied,
        _ => ErrorKind::InvalidInput,
    }
    .into()
}
