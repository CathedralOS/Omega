//! Private rooted persistence for review-only baseline capsules.

use super::{
    BASELINE_NAME_MAXIMUM_BYTES, ReviewOnlyBaselineCapsule, ReviewOnlyBaselineError,
    ReviewOnlyBaselineLimits,
};
use platform_custody::record_file::{
    RecordFileError, RecordFileLimits, RecordFileRoot, is_portable_record_file_name,
};
use std::fmt;
use std::path::{Path, PathBuf};

/// One portable direct-child filename beneath an explicit project-owned
/// review-state directory capability. It is routing only and never enters the
/// capsule's semantic identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyBaselineName(String);

impl ReviewOnlyBaselineName {
    pub fn parse(value: &str) -> Result<Self, ReviewOnlyBaselineNameError> {
        if !is_portable_record_file_name(value, BASELINE_NAME_MAXIMUM_BYTES) {
            return Err(ReviewOnlyBaselineNameError::InvalidName);
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }
}

/// Closed rejection for a non-portable review-baseline leaf name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOnlyBaselineNameError {
    InvalidName,
}

impl fmt::Display for ReviewOnlyBaselineNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("review-baseline filename is not canonical and portable")
    }
}

impl std::error::Error for ReviewOnlyBaselineNameError {}

/// Explicit directory-capability root for review-only baseline state.
///
/// Trusted command orchestration supplies the project-owned directory. This
/// type never discovers storage from dependency source and cannot promote a
/// recovered capsule into accepted lock or package authority.
#[derive(Debug)]
pub struct ReviewOnlyBaselineDirectory {
    root: RecordFileRoot,
}

impl ReviewOnlyBaselineDirectory {
    /// Bind an already-open project-owned review-state directory.
    ///
    /// `display_path` is diagnostic text only. Every filesystem operation is
    /// performed relative to `directory`.
    pub fn from_capability(
        directory: cap_std::fs::Dir,
        display_path: impl Into<PathBuf>,
    ) -> Result<Self, ReviewOnlyBaselineFileError> {
        let root = RecordFileRoot::from_directory(directory, display_path.into())
            .map_err(map_baseline_file_error)?;
        Ok(Self { root })
    }

    /// Persist a complete capsule as a new immutable review-state file.
    /// Existing destinations are never overwritten.
    pub fn persist_new_capsule(
        &self,
        name: &ReviewOnlyBaselineName,
        capsule: &ReviewOnlyBaselineCapsule,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<(), ReviewOnlyBaselineFileError> {
        let bytes = capsule
            .encode(limits)
            .map_err(ReviewOnlyBaselineFileError::Capsule)?;
        self.root
            .write_new(
                name.as_path(),
                &bytes,
                RecordFileLimits {
                    maximum_bytes: limits.maximum_capsule_bytes(),
                },
            )
            .map_err(map_baseline_file_error)
    }

    /// Recover one capsule through the retained file handle, then recheck the
    /// exact bytes and direct-child pathname before returning it.
    pub fn recover_capsule(
        &self,
        name: &ReviewOnlyBaselineName,
        limits: ReviewOnlyBaselineLimits,
    ) -> Result<ReviewOnlyBaselineCapsule, ReviewOnlyBaselineFileError> {
        let record_limits = RecordFileLimits {
            maximum_bytes: limits.maximum_capsule_bytes(),
        };
        let mut read = self
            .root
            .read(name.as_path(), record_limits)
            .map_err(map_baseline_file_error)?;
        let capsule = ReviewOnlyBaselineCapsule::decode(read.bytes(), limits)
            .map_err(ReviewOnlyBaselineFileError::Capsule)?;
        read.verify_current(record_limits)
            .map_err(map_baseline_file_error)?;
        Ok(capsule)
    }
}

/// Closed filesystem and capsule-recovery failures for review-only baseline
/// custody. Attacker-controlled record bytes never enter these messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOnlyBaselineFileError {
    Io { path: PathBuf, message: String },
    InvalidDestination { path: PathBuf },
    NotRegularFile { path: PathBuf },
    DestinationExists { path: PathBuf },
    DirectoryCustodyChanged { path: PathBuf },
    PublishedButUnconfirmed { path: PathBuf, message: String },
    ContentsChanged { path: PathBuf },
    ByteLimitExceeded { actual: u64, maximum: usize },
    LengthOverflow,
    AllocationFailed,
    StageNameSpaceExhausted { directory: PathBuf },
    Capsule(ReviewOnlyBaselineError),
}

impl fmt::Display for ReviewOnlyBaselineFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(
                formatter,
                "review-baseline file `{}`: {message}",
                path.display()
            ),
            Self::InvalidDestination { path } => write!(
                formatter,
                "review-baseline destination `{}` is invalid",
                path.display()
            ),
            Self::NotRegularFile { path } => write!(
                formatter,
                "review-baseline path `{}` is not a regular confined file",
                path.display()
            ),
            Self::DestinationExists { path } => write!(
                formatter,
                "review-baseline destination `{}` already exists",
                path.display()
            ),
            Self::DirectoryCustodyChanged { path } => write!(
                formatter,
                "review-baseline directory custody changed at `{}`",
                path.display()
            ),
            Self::PublishedButUnconfirmed { path, message } => write!(
                formatter,
                "review-baseline destination `{}` was published but could not be confirmed: {message}",
                path.display()
            ),
            Self::ContentsChanged { path } => write!(
                formatter,
                "review-baseline file `{}` changed while it was being recovered",
                path.display()
            ),
            Self::ByteLimitExceeded { actual, maximum } => write!(
                formatter,
                "review-baseline file uses {actual} bytes; the limit is {maximum}"
            ),
            Self::LengthOverflow => formatter.write_str("review-baseline file length overflow"),
            Self::AllocationFailed => formatter.write_str("review-baseline file allocation failed"),
            Self::StageNameSpaceExhausted { directory } => write!(
                formatter,
                "review-baseline staging names are exhausted beneath `{}`",
                directory.display()
            ),
            Self::Capsule(error) => write!(formatter, "invalid review-baseline capsule: {error}"),
        }
    }
}

impl std::error::Error for ReviewOnlyBaselineFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Capsule(error) => Some(error),
            _ => None,
        }
    }
}

fn map_baseline_file_error(error: RecordFileError) -> ReviewOnlyBaselineFileError {
    match error {
        RecordFileError::Io { path, message } => ReviewOnlyBaselineFileError::Io { path, message },
        RecordFileError::InvalidDestination { path } => {
            ReviewOnlyBaselineFileError::InvalidDestination { path }
        }
        RecordFileError::NotRegularFile { path } => {
            ReviewOnlyBaselineFileError::NotRegularFile { path }
        }
        RecordFileError::DestinationExists { path } => {
            ReviewOnlyBaselineFileError::DestinationExists { path }
        }
        RecordFileError::PublishedButUnconfirmed { path, message } => {
            ReviewOnlyBaselineFileError::PublishedButUnconfirmed { path, message }
        }
        RecordFileError::ContentsChanged { path } => {
            ReviewOnlyBaselineFileError::ContentsChanged { path }
        }
        RecordFileError::ByteLimitExceeded { actual, maximum } => {
            ReviewOnlyBaselineFileError::ByteLimitExceeded { actual, maximum }
        }
        RecordFileError::LengthOverflow => ReviewOnlyBaselineFileError::LengthOverflow,
        RecordFileError::AllocationFailed => ReviewOnlyBaselineFileError::AllocationFailed,
        RecordFileError::StageNameSpaceExhausted { directory } => {
            ReviewOnlyBaselineFileError::StageNameSpaceExhausted { directory }
        }
    }
}
