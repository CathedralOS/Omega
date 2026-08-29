//! Stable executable metadata and bounded content identities.

use crate::resolution::source::SourceResolveError;
use crate::resolution::source::git::execution::format_sha256;
use crate::resolution::source::limits::GIT_EXECUTABLE_BYTE_LIMIT;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::resolution::source) struct GitExecutableMetadataIdentity {
    length: u64,
    modified: SystemTime,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    mode: u32,
}

pub(super) fn hash_git_executable(path: &Path) -> Result<String, SourceResolveError> {
    let metadata =
        std::fs::metadata(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "path is not a regular file".to_owned(),
        });
    }
    if metadata.len() > GIT_EXECUTABLE_BYTE_LIMIT {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: format!(
                "file exceeds the {GIT_EXECUTABLE_BYTE_LIMIT}-byte executable ceiling"
            ),
        });
    }
    let mut file = File::open(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    let mut hasher = Sha256::new();
    let mut observed = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count =
            file.read(&mut buffer)
                .map_err(|error| SourceResolveError::GitExecutableInvalid {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                })?;
        if count == 0 {
            break;
        }
        observed = observed.saturating_add(count as u64);
        if observed > GIT_EXECUTABLE_BYTE_LIMIT {
            return Err(SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: format!(
                    "file exceeds the {GIT_EXECUTABLE_BYTE_LIMIT}-byte executable ceiling"
                ),
            });
        }
        hasher.update(&buffer[..count]);
    }
    if observed != metadata.len() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "file length changed while it was hashed".to_owned(),
        });
    }
    Ok(format_sha256(&hasher.finalize()))
}

pub(super) fn observe_git_executable_metadata(
    path: &Path,
) -> Result<GitExecutableMetadataIdentity, SourceResolveError> {
    let metadata =
        std::fs::metadata(path).map_err(|error| SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
    if !metadata.is_file() {
        return Err(SourceResolveError::GitExecutableInvalid {
            path: path.to_path_buf(),
            message: "path is not a regular file".to_owned(),
        });
    }
    let modified =
        metadata
            .modified()
            .map_err(|error| SourceResolveError::GitExecutableInvalid {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        Ok(GitExecutableMetadataIdentity {
            length: metadata.len(),
            modified,
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
        })
    }
    #[cfg(windows)]
    {
        Ok(GitExecutableMetadataIdentity {
            length: metadata.len(),
            modified,
        })
    }
}
