//! Retained custody for the immutable Git snapshot publication collection.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::resolution::source::{
    CacheCustodyKind, CapabilityDirectory, CapabilityMetadata, GIT_CACHE_SNAPSHOTS,
    SourceResolveError, direct_cache_child_name, io_error,
};

use super::cache_invalid;
use super::custody::verify_retained_git_directory_identity;

pub(in crate::resolution::source) struct RetainedGitSnapshots {
    pub(in crate::resolution::source) path: PathBuf,
    pub(in crate::resolution::source) entry: CapabilityDirectory,
    pub(in crate::resolution::source) directory: CapabilityDirectory,
    pub(in crate::resolution::source) identity: CapabilityMetadata,
}

impl RetainedGitSnapshots {
    pub(in crate::resolution::source) fn verify_identity(&self) -> Result<(), SourceResolveError> {
        verify_retained_git_directory_identity(
            &self.entry,
            OsStr::new(GIT_CACHE_SNAPSHOTS),
            &self.directory,
            &self.identity,
            &self.path,
            "Git snapshot collection no longer identifies the retained directory",
        )
    }

    pub(in crate::resolution::source) fn publication_exists(
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
