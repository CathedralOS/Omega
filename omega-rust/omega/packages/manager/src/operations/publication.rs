//! Recoverable publication of the two accepted project files.

mod checked;
mod directory;
mod error;
mod journal;
mod transaction;

pub use checked::{PublishReviewedPackageChangeError, publish_reviewed_package_change};
pub use error::PackagePublicationError;
pub use transaction::PackageFileTransaction;

/// Bounded project-file and recovery-journal I/O. These are storage limits,
/// not compiler policy or a claim about total allocator usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackagePublicationLimits {
    pub maximum_file_bytes: usize,
    pub maximum_journal_bytes: usize,
}

impl Default for PackagePublicationLimits {
    fn default() -> Self {
        Self {
            maximum_file_bytes: 128 * 1024 * 1024,
            maximum_journal_bytes: 512 * 1024 * 1024 + 256,
        }
    }
}

impl PackagePublicationLimits {
    fn files(self) -> platform_custody::record_file::RecordFileLimits {
        platform_custody::record_file::RecordFileLimits {
            maximum_bytes: self.maximum_file_bytes,
        }
    }

    fn journal(self) -> platform_custody::record_file::RecordFileLimits {
        platform_custody::record_file::RecordFileLimits {
            maximum_bytes: self.maximum_journal_bytes,
        }
    }
}

const BUILD_FILE: &str = "build.omg";
const LOCK_FILE: &str = "omega.lock";
const JOURNAL_FILE: &str = "pending";
const TRANSACTION_LOCK: &str = "transaction.lock";
const STATE_DIRECTORY: &str = "package-manager";

#[cfg(test)]
mod tests;
