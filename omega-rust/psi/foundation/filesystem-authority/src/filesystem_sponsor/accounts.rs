//! Resource counters and live writer custody, not a filesystem namespace.

use super::{FilesystemSponsorError, FilesystemSponsorLimits, FilesystemSponsorPath};
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
pub(crate) struct FilesystemAccount {
    pub(crate) id: u64,
    pub(crate) session_root: PathBuf,
    pub(crate) limits: FilesystemSponsorLimits,
    pub(crate) entries: u64,
    pub(crate) logical_bytes: u64,
    // IDs are not reused. Closing releases the path and extent; sealed files
    // exist only in the host tree, not a namespace or object table.
    pub(crate) writers: Vec<Option<StagingWriter>>,
    pub(crate) prepared_transaction: Option<u64>,
    pub(crate) next_transaction_id: u64,
}

#[derive(Debug)]
pub(crate) struct StagingWriter {
    pub(crate) relative_path: PathBuf,
    pub(crate) extent: u64,
}

impl FilesystemAccount {
    pub(crate) fn admit_growth(
        &self,
        entries: u64,
        bytes: u64,
        extent: u64,
    ) -> Result<(), FilesystemSponsorError> {
        let attempted = checked_add(self.entries, entries)?;
        if attempted > self.limits.maximum_entries {
            return Err(FilesystemSponsorError::EntryLimitExceeded {
                limit: self.limits.maximum_entries,
                attempted,
            });
        }
        let attempted = checked_add(self.logical_bytes, bytes)?;
        if attempted > self.limits.maximum_total_logical_bytes {
            return Err(FilesystemSponsorError::TotalLogicalBytesLimitExceeded {
                limit: self.limits.maximum_total_logical_bytes,
                attempted,
            });
        }
        if extent > self.limits.maximum_object_extent {
            return Err(FilesystemSponsorError::ObjectExtentLimitExceeded {
                limit: self.limits.maximum_object_extent,
                attempted: extent,
            });
        }
        Ok(())
    }
}

pub(crate) fn check_account_path(
    account_id: u64,
    path: &FilesystemSponsorPath,
) -> Result<(), FilesystemSponsorError> {
    if path.account_id != account_id {
        return Err(FilesystemSponsorError::CrossAccountOperation);
    }
    Ok(())
}

pub(crate) fn normalize_absolute(path: &Path) -> Result<PathBuf, FilesystemSponsorError> {
    if !path.is_absolute() {
        return Err(FilesystemSponsorError::PathMustBeAbsolute(
            path.to_path_buf(),
        ));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(FilesystemSponsorError::PathEscapesFilesystemRoot(
                        path.to_path_buf(),
                    ));
                }
            }
            Component::Normal(name) => normalized.push(name),
        }
    }
    Ok(normalized)
}

pub(crate) fn checked_add(left: u64, right: u64) -> Result<u64, FilesystemSponsorError> {
    left.checked_add(right)
        .ok_or(FilesystemSponsorError::ArithmeticOverflow)
}
