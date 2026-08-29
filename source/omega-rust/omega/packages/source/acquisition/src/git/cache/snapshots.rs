//! Retained custody for the immutable Git snapshot publication collection.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use cap_std::fs::{Dir as CapabilityDirectory, Metadata as CapabilityMetadata};

use crate::SourceResolveError;
use crate::custody::publication::direct_cache_child_name;
use crate::custody::tree::CacheCustodyKind;
use crate::limits::GIT_CACHE_SNAPSHOTS;
use crate::local::capture::io_error;

use super::custody::verify_retained_git_directory_identity;
use super::identity::cache_invalid;

pub(crate) struct RetainedGitSnapshots {
    pub(crate) path: PathBuf,
    pub(crate) entry: CapabilityDirectory,
    pub(crate) directory: CapabilityDirectory,
    pub(crate) identity: CapabilityMetadata,
}

impl RetainedGitSnapshots {
    pub(crate) fn verify_identity(&self) -> Result<(), SourceResolveError> {
        verify_retained_git_directory_identity(
            &self.entry,
            OsStr::new(GIT_CACHE_SNAPSHOTS),
            &self.directory,
            &self.identity,
            &self.path,
            "Git snapshot collection no longer identifies the retained directory",
        )
    }

    pub(crate) fn publication_exists(
        &self,
        publication: &Path,
    ) -> Result<bool, SourceResolveError> {
        self.verify_identity()?;
        let name = direct_cache_child_name(CacheCustodyKind::Git, &self.path, publication)?;
        match self.directory.symlink_metadata(name) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                Err(cache_invalid(
                    publication,
                    "Git snapshot publication is not a concrete directory",
                ))
            }
            Ok(_) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(io_error(publication, error)),
        }
    }
}
