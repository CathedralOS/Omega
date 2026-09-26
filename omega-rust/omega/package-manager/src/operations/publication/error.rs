use std::fmt;
use std::path::PathBuf;

use platform_custody::record_file::RecordFileError;

#[derive(Debug)]
pub enum PackagePublicationError {
    Io {
        path: PathBuf,
        message: String,
    },
    File(RecordFileError),
    Busy,
    DirectoryChanged,
    RecoveryRequired,
    ConcurrentEdit {
        file: &'static str,
    },
    InvalidJournal(&'static str),
    ByteLimitExceeded,
    AllocationFailed,
    /// The intended pair is journaled; publication may be partially complete.
    /// Preserve the journal and recover rather than reporting no mutation.
    Pending(Box<PackagePublicationError>),
}

impl fmt::Display for PackagePublicationError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(output, "{}: {message}", path.display()),
            Self::File(error) => write!(output, "project file operation failed: {error:?}"),
            Self::Busy => output.write_str("another package operation holds the project lock"),
            Self::DirectoryChanged => output.write_str("project transaction directory changed"),
            Self::RecoveryRequired => {
                output.write_str("recover the pending package publication before starting another")
            }
            Self::ConcurrentEdit { file } => write!(
                output,
                "{file} changed outside this package transaction; no unrelated bytes will be overwritten"
            ),
            Self::InvalidJournal(reason) => {
                write!(output, "invalid package recovery journal: {reason}")
            }
            Self::ByteLimitExceeded => {
                output.write_str("package publication exceeds its file or journal byte limit")
            }
            Self::AllocationFailed => output.write_str("package publication allocation failed"),
            Self::Pending(error) => {
                write!(output, "package publication is pending recovery: {error}")
            }
        }
    }
}

impl std::error::Error for PackagePublicationError {}

impl From<RecordFileError> for PackagePublicationError {
    fn from(error: RecordFileError) -> Self {
        Self::File(error)
    }
}

pub(super) fn io_error(path: &std::path::Path, error: std::io::Error) -> PackagePublicationError {
    PackagePublicationError::Io {
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}
