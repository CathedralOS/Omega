//! The journal is the commit intent; interrupted writes complete forward.

use platform_custody::record_file::{RecordFileLock, RecordFileRoot};
use std::path::Path;

use super::directory::ProjectDirectories;
use super::error::io_error;
use super::journal::PackageFileJournal;
use super::{
    BUILD_FILE, JOURNAL_FILE, LOCK_FILE, PackagePublicationError, PackagePublicationLimits,
    TRANSACTION_LOCK,
};

/// Filesystem coordination for accepted package changes, not compiler review.
/// The caller must supply the reviewed bytes and recheck candidate source.
///
/// Hold this guard while reading the accepted pair. Recover pending intent
/// before using either file; two independent renames are not a simultaneous
/// update for readers that ignore the guard. The persistent lock file is never
/// unlinked: its OS lock, not its existence, represents a running operation.
pub struct PackageFileTransaction {
    directories: ProjectDirectories,
    project: RecordFileRoot,
    journal: RecordFileRoot,
    lock: RecordFileLock,
    limits: PackagePublicationLimits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PublicationStep {
    IntentRecorded,
    BuildReplaced,
    LockReplaced,
}

impl PackageFileTransaction {
    /// Reader entry for projects with existing transaction state. Ordinary
    /// source-only compilation does not create control files in a read-only
    /// checkout. Package commands use `open` before reading their lock baseline.
    pub fn open_if_present(
        root: &Path,
        limits: PackagePublicationLimits,
    ) -> Result<Option<Self>, PackagePublicationError> {
        let state_path = root.join("build").join(super::STATE_DIRECTORY);
        match std::fs::symlink_metadata(&state_path) {
            Ok(_) => Self::open(root, limits).map(Some),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(io_error(&state_path, error)),
        }
    }

    pub fn open(
        root: &Path,
        limits: PackagePublicationLimits,
    ) -> Result<Self, PackagePublicationError> {
        let directories = ProjectDirectories::open(root)?;
        let project = RecordFileRoot::from_directory(
            directories
                .root
                .try_clone()
                .map_err(|error| io_error(root, error))?,
            directories.root_path.clone(),
        )?;
        let journal = RecordFileRoot::from_directory(
            directories
                .state
                .try_clone()
                .map_err(|error| io_error(root, error))?,
            directories.state_path(),
        )?;
        let lock = journal
            .try_lock(Path::new(TRANSACTION_LOCK))?
            .ok_or(PackagePublicationError::Busy)?;
        let result = Self {
            directories,
            project,
            journal,
            lock,
            limits,
        };
        result.verify()?;
        Ok(result)
    }

    pub fn project_root(&self) -> &Path {
        &self.directories.root_path
    }

    /// Command-owned proposal/findings files share the retained state directory
    /// and project mutex, but are not the accepted pair's commit-intent journal.
    pub(crate) fn command_state_files(&self) -> Result<RecordFileRoot, PackagePublicationError> {
        self.verify()?;
        RecordFileRoot::from_directory(
            self.directories
                .state
                .try_clone()
                .map_err(|error| io_error(self.project_root(), error))?,
            self.directories.state_path(),
        )
        .map_err(PackagePublicationError::from)
    }

    /// Whether accepted-file loading must first finish an earlier publication.
    pub fn has_pending(&self) -> Result<bool, PackagePublicationError> {
        self.verify()?;
        Ok(self
            .journal
            .read_optional(Path::new(JOURNAL_FILE), self.limits.journal())?
            .is_some())
    }

    /// Read a reconciled accepted pair while holding the project mutex.
    pub fn read_pair(&self) -> Result<(Vec<u8>, Option<Vec<u8>>), PackagePublicationError> {
        if self.has_pending()? {
            return Err(PackagePublicationError::RecoveryRequired);
        }
        let mut build = self
            .project
            .read(Path::new(BUILD_FILE), self.limits.files())?;
        let mut lock = self
            .project
            .read_optional(Path::new(LOCK_FILE), self.limits.files())?;
        let build_bytes = self.copy(build.bytes())?;
        let lock_bytes = lock
            .as_ref()
            .map(|read| self.copy(read.bytes()))
            .transpose()?;
        build.verify_current(self.limits.files())?;
        if let Some(lock) = &mut lock {
            lock.verify_current(self.limits.files())?;
        }
        self.verify()?;
        Ok((build_bytes, lock_bytes))
    }

    /// Publish an already-reviewed pair after exact old-file checks. An absent
    /// old lock is distinct from an empty file. Any error after durable intent
    /// is `Pending`: callers must recover, not assume no mutation occurred.
    pub fn publish(
        &mut self,
        before_build: &[u8],
        after_build: &[u8],
        before_lock: Option<&[u8]>,
        after_lock: &[u8],
    ) -> Result<(), PackagePublicationError> {
        self.publish_with_checkpoint(before_build, after_build, before_lock, after_lock, |_| {
            Ok(())
        })
    }

    pub(super) fn publish_with_checkpoint(
        &mut self,
        before_build: &[u8],
        after_build: &[u8],
        before_lock: Option<&[u8]>,
        after_lock: &[u8],
        mut checkpoint: impl FnMut(PublicationStep) -> Result<(), PackagePublicationError>,
    ) -> Result<(), PackagePublicationError> {
        if self.has_pending()? {
            return Err(PackagePublicationError::RecoveryRequired);
        }
        self.expect_file(BUILD_FILE, Some(before_build))?;
        self.expect_file(LOCK_FILE, before_lock)?;
        let journal = PackageFileJournal {
            before_build: self.copy(before_build)?,
            after_build: self.copy(after_build)?,
            before_lock: before_lock.map(|bytes| self.copy(bytes)).transpose()?,
            after_lock: self.copy(after_lock)?,
        };
        let encoded = journal.encode(self.limits)?;
        self.verify()?;
        // Encoding may be substantial. Check both originals again before intent.
        self.expect_file(BUILD_FILE, Some(before_build))?;
        self.expect_file(LOCK_FILE, before_lock)?;
        self.journal
            .write_new(Path::new(JOURNAL_FILE), &encoded, self.limits.journal())
            .map_err(|error| {
                if matches!(
                    error,
                    platform_custody::record_file::RecordFileError::PublishedButUnconfirmed { .. }
                ) {
                    PackagePublicationError::Pending(Box::new(error.into()))
                } else {
                    error.into()
                }
            })?;
        let result = (|| {
            checkpoint(PublicationStep::IntentRecorded)?;
            self.apply(&journal, &mut checkpoint)?;
            self.clear_journal(&encoded)
        })();
        result.map_err(|error| PackagePublicationError::Pending(Box::new(error)))
    }

    /// Finish recorded commit intent, without selecting versions or approving
    /// capabilities. Returns false if there was no journal. Unexpected project
    /// bytes stop recovery without overwriting them or deleting the journal.
    pub fn recover(&mut self) -> Result<bool, PackagePublicationError> {
        self.verify()?;
        let Some(record) = self
            .journal
            .read_optional(Path::new(JOURNAL_FILE), self.limits.journal())?
        else {
            return Ok(false);
        };
        let result = (|| {
            let journal = PackageFileJournal::recover(record.bytes(), self.limits)?;
            self.apply(&journal, &mut |_| Ok(()))?;
            record.remove(self.limits.journal())?;
            self.verify()?;
            Ok(true)
        })();
        result.map_err(|error| PackagePublicationError::Pending(Box::new(error)))
    }

    fn apply(
        &self,
        journal: &PackageFileJournal,
        checkpoint: &mut impl FnMut(PublicationStep) -> Result<(), PackagePublicationError>,
    ) -> Result<(), PackagePublicationError> {
        self.verify()?;
        // Check the whole pair before touching either file, including recovery.
        self.expect_either(
            BUILD_FILE,
            Some(&journal.before_build),
            &journal.after_build,
        )?;
        self.expect_either(
            LOCK_FILE,
            journal.before_lock.as_deref(),
            &journal.after_lock,
        )?;
        self.replace(
            BUILD_FILE,
            Some(&journal.before_build),
            &journal.after_build,
        )?;
        checkpoint(PublicationStep::BuildReplaced)?;
        self.replace(
            LOCK_FILE,
            journal.before_lock.as_deref(),
            &journal.after_lock,
        )?;
        checkpoint(PublicationStep::LockReplaced)?;
        self.expect_file(BUILD_FILE, Some(&journal.after_build))?;
        self.expect_file(LOCK_FILE, Some(&journal.after_lock))?;
        self.verify()
    }

    fn replace(
        &self,
        name: &'static str,
        before: Option<&[u8]>,
        after: &[u8],
    ) -> Result<(), PackagePublicationError> {
        self.verify()?;
        let read = self
            .project
            .read_optional(Path::new(name), self.limits.files())?;
        let actual = read.as_ref().map(|read| read.bytes());
        if actual == Some(after) {
            return Ok(());
        }
        if actual != before {
            return Err(PackagePublicationError::ConcurrentEdit { file: name });
        }
        if let Some(read) = read {
            read.replace_in(after, self.limits.files(), &self.journal)?;
        } else {
            self.project.write_new_in(
                Path::new(name),
                after,
                self.limits.files(),
                &self.journal,
            )?;
        }
        Ok(())
    }

    fn expect_file(
        &self,
        name: &'static str,
        expected: Option<&[u8]>,
    ) -> Result<(), PackagePublicationError> {
        let read = self
            .project
            .read_optional(Path::new(name), self.limits.files())?;
        if read.as_ref().map(|read| read.bytes()) != expected {
            return Err(PackagePublicationError::ConcurrentEdit { file: name });
        }
        Ok(())
    }

    fn expect_either(
        &self,
        name: &'static str,
        before: Option<&[u8]>,
        after: &[u8],
    ) -> Result<(), PackagePublicationError> {
        let read = self
            .project
            .read_optional(Path::new(name), self.limits.files())?;
        let actual = read.as_ref().map(|read| read.bytes());
        if actual != before && actual != Some(after) {
            return Err(PackagePublicationError::ConcurrentEdit { file: name });
        }
        Ok(())
    }

    fn clear_journal(&self, expected: &[u8]) -> Result<(), PackagePublicationError> {
        let read = self
            .journal
            .read(Path::new(JOURNAL_FILE), self.limits.journal())?;
        if read.bytes() != expected {
            return Err(PackagePublicationError::InvalidJournal(
                "journal changed during publication",
            ));
        }
        read.remove(self.limits.journal())?;
        self.verify()
    }

    fn verify(&self) -> Result<(), PackagePublicationError> {
        self.directories.verify()?;
        self.lock.verify_current()?;
        Ok(())
    }

    fn copy(&self, bytes: &[u8]) -> Result<Vec<u8>, PackagePublicationError> {
        if bytes.len() > self.limits.maximum_file_bytes {
            return Err(PackagePublicationError::ByteLimitExceeded);
        }
        let mut copied = Vec::new();
        copied
            .try_reserve_exact(bytes.len())
            .map_err(|_| PackagePublicationError::AllocationFailed)?;
        copied.extend_from_slice(bytes);
        Ok(copied)
    }
}
