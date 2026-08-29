use super::record::{
    ReviewOnlyRootPolicyRecordError, ReviewOnlyRootPolicyRecordLimits,
    recover_review_only_root_policy_resolution,
};
use super::resolution::ReviewOnlyRootPolicyResolution;
use crate::ReviewOnlyCapabilityConflictSet;
use crate::records::atomic_file::{
    RecordFileError, RecordFileLimits, RecordFileRoot, is_portable_record_file_name,
};
use std::fmt;
use std::path::{Path, PathBuf};

const ROOT_POLICY_NAME_MAXIMUM_BYTES: usize = 255;
/// Canonical command-selected filename of an authored root-policy record.
///
/// This is one direct child of an explicitly supplied directory capability;
/// nested paths are intentionally unrepresentable. Trusted command
/// orchestration is responsible for opening the root-owned policy directory.
/// The package manager does not discover it from dependency source, and this
/// type deliberately does not prescribe the final command UX or filename.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReviewOnlyRootPolicyName(String);

impl ReviewOnlyRootPolicyName {
    pub fn parse(value: &str) -> Result<Self, ReviewOnlyRootPolicyNameError> {
        if !is_portable_record_file_name(value, ROOT_POLICY_NAME_MAXIMUM_BYTES) {
            return Err(ReviewOnlyRootPolicyNameError::InvalidName);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewOnlyRootPolicyNameError {
    InvalidName,
}

impl fmt::Display for ReviewOnlyRootPolicyNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("root-policy filename is not canonical and portable")
    }
}

impl std::error::Error for ReviewOnlyRootPolicyNameError {}

/// Explicit directory-capability root for authored review-only policy state.
///
/// Trusted command orchestration must supply the actual root-owned policy
/// directory; this library cannot infer that role from an arbitrary capability.
/// Persisted bytes still have no standing until recovery matches them against
/// the current compiler-derived conflict set.
#[derive(Debug)]
pub struct ReviewOnlyRootPolicyDirectory {
    root: RecordFileRoot,
}

impl ReviewOnlyRootPolicyDirectory {
    /// Bind an already-open root-owned policy directory.
    ///
    /// `display_path` is diagnostic text only; filesystem operations use only
    /// `directory`. Trusted command orchestration is responsible for acquiring
    /// the capability from the actual invocation root.
    pub fn from_capability(
        directory: cap_std::fs::Dir,
        display_path: impl Into<PathBuf>,
    ) -> Result<Self, ReviewOnlyRootPolicyFileError> {
        let root = RecordFileRoot::from_directory(directory, display_path.into())
            .map_err(map_root_policy_file_error)?;
        Ok(Self { root })
    }

    /// Persist one complete resolution as a new authored project-policy file.
    ///
    /// Existing files are never overwritten. This does not authorize lock or
    /// `build.omg` mutation.
    pub fn persist_new_resolution(
        &self,
        name: &ReviewOnlyRootPolicyName,
        resolution: &ReviewOnlyRootPolicyResolution,
        limits: ReviewOnlyRootPolicyRecordLimits,
    ) -> Result<(), ReviewOnlyRootPolicyFileError> {
        let bytes = resolution
            .encode_canonical(limits)
            .map_err(ReviewOnlyRootPolicyFileError::Record)?;
        self.root
            .write_new(
                name.as_path(),
                &bytes,
                RecordFileLimits {
                    maximum_bytes: limits.maximum_bytes(),
                },
            )
            .map_err(map_root_policy_file_error)
    }

    /// Recover authored policy only against the exact current candidate.
    pub fn recover_resolution(
        &self,
        name: &ReviewOnlyRootPolicyName,
        conflicts: &ReviewOnlyCapabilityConflictSet,
        limits: ReviewOnlyRootPolicyRecordLimits,
    ) -> Result<ReviewOnlyRootPolicyResolution, ReviewOnlyRootPolicyFileError> {
        let mut read = self
            .root
            .read(
                name.as_path(),
                RecordFileLimits {
                    maximum_bytes: limits.maximum_bytes(),
                },
            )
            .map_err(map_root_policy_file_error)?;
        let resolution =
            recover_review_only_root_policy_resolution(conflicts, read.bytes(), limits)
                .map_err(ReviewOnlyRootPolicyFileError::Record)?;
        read.verify_current(RecordFileLimits {
            maximum_bytes: limits.maximum_bytes(),
        })
        .map_err(map_root_policy_file_error)?;
        Ok(resolution)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewOnlyRootPolicyFileError {
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
    Record(ReviewOnlyRootPolicyRecordError),
}

impl fmt::Display for ReviewOnlyRootPolicyFileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(
                    formatter,
                    "root-policy file `{}`: {message}",
                    path.display()
                )
            }
            Self::InvalidDestination { path } => write!(
                formatter,
                "root-policy destination `{}` is invalid",
                path.display()
            ),
            Self::NotRegularFile { path } => write!(
                formatter,
                "root-policy path `{}` is not a regular confined file",
                path.display()
            ),
            Self::DestinationExists { path } => write!(
                formatter,
                "root-policy destination `{}` already exists",
                path.display()
            ),
            Self::DirectoryCustodyChanged { path } => write!(
                formatter,
                "root-policy directory custody changed at `{}`",
                path.display()
            ),
            Self::PublishedButUnconfirmed { path, message } => write!(
                formatter,
                "root-policy destination `{}` was published but could not be confirmed: {message}",
                path.display()
            ),
            Self::ContentsChanged { path } => write!(
                formatter,
                "root-policy file `{}` changed while it was being recovered",
                path.display()
            ),
            Self::ByteLimitExceeded { actual, maximum } => write!(
                formatter,
                "root-policy file uses {actual} bytes; the limit is {maximum}"
            ),
            Self::LengthOverflow => formatter.write_str("root-policy file length overflow"),
            Self::AllocationFailed => formatter.write_str("root-policy file allocation failed"),
            Self::StageNameSpaceExhausted { directory } => write!(
                formatter,
                "root-policy staging names are exhausted beneath `{}`",
                directory.display()
            ),
            Self::Record(error) => write!(formatter, "invalid root-policy record: {error}"),
        }
    }
}

impl std::error::Error for ReviewOnlyRootPolicyFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Record(error) => Some(error),
            _ => None,
        }
    }
}

fn map_root_policy_file_error(error: RecordFileError) -> ReviewOnlyRootPolicyFileError {
    match error {
        RecordFileError::Io { path, message } => {
            ReviewOnlyRootPolicyFileError::Io { path, message }
        }
        RecordFileError::InvalidDestination { path } => {
            ReviewOnlyRootPolicyFileError::InvalidDestination { path }
        }
        RecordFileError::NotRegularFile { path } => {
            ReviewOnlyRootPolicyFileError::NotRegularFile { path }
        }
        RecordFileError::DestinationExists { path } => {
            ReviewOnlyRootPolicyFileError::DestinationExists { path }
        }
        RecordFileError::PublishedButUnconfirmed { path, message } => {
            ReviewOnlyRootPolicyFileError::PublishedButUnconfirmed { path, message }
        }
        RecordFileError::ContentsChanged { path } => {
            ReviewOnlyRootPolicyFileError::ContentsChanged { path }
        }
        RecordFileError::ByteLimitExceeded { actual, maximum } => {
            ReviewOnlyRootPolicyFileError::ByteLimitExceeded { actual, maximum }
        }
        RecordFileError::LengthOverflow => ReviewOnlyRootPolicyFileError::LengthOverflow,
        RecordFileError::AllocationFailed => ReviewOnlyRootPolicyFileError::AllocationFailed,
        RecordFileError::StageNameSpaceExhausted { directory } => {
            ReviewOnlyRootPolicyFileError::StageNameSpaceExhausted { directory }
        }
    }
}
