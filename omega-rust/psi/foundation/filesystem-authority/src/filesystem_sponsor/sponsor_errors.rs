//! The filesystem sponsor error.

use std::error::Error;
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesystemSponsorError {
    PathMustBeAbsolute(PathBuf),
    PathEscapesFilesystemRoot(PathBuf),
    PathOutsideSessionRoot(PathBuf),
    SessionRootIsNotAnEntry,
    CrossAccountOperation,
    ParentEntryMissing(PathBuf),
    ParentIsNotDirectory(PathBuf),
    EntryAlreadyExists(PathBuf),
    EntryNotFound(PathBuf),
    EntryIsNotRegularObject(PathBuf),
    DirectoryNotEmpty(PathBuf),
    InvalidDirectoryRename(PathBuf),
    OpenDescriptorNotFound,
    TransactionAlreadyPrepared,
    TransactionNoLongerCurrent,
    EntryLimitExceeded { limit: u64, attempted: u64 },
    TotalLogicalBytesLimitExceeded { limit: u64, attempted: u64 },
    ObjectExtentLimitExceeded { limit: u64, attempted: u64 },
    PartialWriteExceedsPrepared { prepared: u64, actual: u64 },
    ArithmeticOverflow,
    AccountIdentityExhausted,
    AccountPoisoned,
}

impl fmt::Display for FilesystemSponsorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathMustBeAbsolute(path) => {
                write!(
                    formatter,
                    "filesystem sponsor path is not absolute: {}",
                    path.display()
                )
            }
            Self::PathEscapesFilesystemRoot(path) => write!(
                formatter,
                "filesystem sponsor path escapes the filesystem root: {}",
                path.display()
            ),
            Self::PathOutsideSessionRoot(path) => write!(
                formatter,
                "filesystem sponsor path is outside its session root: {}",
                path.display()
            ),
            Self::SessionRootIsNotAnEntry => {
                formatter.write_str("the session root is excluded from sponsored entries")
            }
            Self::CrossAccountOperation => {
                formatter.write_str("filesystem operation crosses sponsor accounts")
            }
            Self::ParentEntryMissing(path) => {
                write!(formatter, "parent entry does not exist: {}", path.display())
            }
            Self::ParentIsNotDirectory(path) => {
                write!(
                    formatter,
                    "parent entry is not a directory: {}",
                    path.display()
                )
            }
            Self::EntryAlreadyExists(path) => {
                write!(formatter, "entry already exists: {}", path.display())
            }
            Self::EntryNotFound(path) => {
                write!(formatter, "entry does not exist: {}", path.display())
            }
            Self::EntryIsNotRegularObject(path) => {
                write!(
                    formatter,
                    "entry is not a regular object: {}",
                    path.display()
                )
            }
            Self::DirectoryNotEmpty(path) => {
                write!(formatter, "directory is not empty: {}", path.display())
            }
            Self::InvalidDirectoryRename(path) => write!(
                formatter,
                "directory cannot be renamed beneath itself: {}",
                path.display()
            ),
            Self::OpenDescriptorNotFound => formatter.write_str("open descriptor does not exist"),
            Self::TransactionAlreadyPrepared => {
                formatter.write_str("another filesystem accounting transaction is prepared")
            }
            Self::TransactionNoLongerCurrent => {
                formatter.write_str("filesystem accounting transaction is no longer current")
            }
            Self::EntryLimitExceeded { limit, attempted } => write!(
                formatter,
                "filesystem entry limit {limit} would be exceeded by {attempted} entries"
            ),
            Self::TotalLogicalBytesLimitExceeded { limit, attempted } => write!(
                formatter,
                "filesystem logical-byte limit {limit} would be exceeded by {attempted} bytes"
            ),
            Self::ObjectExtentLimitExceeded { limit, attempted } => write!(
                formatter,
                "filesystem object-extent limit {limit} would be exceeded by {attempted} bytes"
            ),
            Self::PartialWriteExceedsPrepared { prepared, actual } => write!(
                formatter,
                "provider reported {actual} written bytes after preparing at most {prepared}"
            ),
            Self::ArithmeticOverflow => {
                formatter.write_str("filesystem accounting arithmetic overflowed")
            }
            Self::AccountIdentityExhausted => {
                formatter.write_str("filesystem sponsor account identity space is exhausted")
            }
            Self::AccountPoisoned => formatter.write_str("filesystem sponsor account is poisoned"),
        }
    }
}

impl Error for FilesystemSponsorError {}

impl FilesystemSponsorError {
    pub const fn is_limit_exceeded(&self) -> bool {
        matches!(
            self,
            Self::EntryLimitExceeded { .. }
                | Self::TotalLogicalBytesLimitExceeded { .. }
                | Self::ObjectExtentLimitExceeded { .. }
                | Self::ArithmeticOverflow
        )
    }
}
