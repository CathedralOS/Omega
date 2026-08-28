//! Durable, policy-free storage for canonical deployment-journal records.
//!
//! Cathedral chooses the journal path and recovery policy. This adapter only
//! publishes one new canonical phase record without overwriting an older row,
//! flushes the file and containing directory, and independently replays bytes.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    ComponentDeploymentJournalRecord, decode_component_deployment_journal,
    encode_component_deployment_journal,
};

static STAGING_IDENTITY: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentDeploymentJournalStorageState {
    Unpublished,
    PublishedCleanupOrSyncIncomplete,
}

/// Rejection retains the exact record and requested path. Once the target
/// hard link exists, the state explicitly reports that publication may be
/// visible even when staging cleanup or directory synchronization failed.
#[derive(Debug)]
pub struct ComponentDeploymentJournalStorageError {
    record: ComponentDeploymentJournalRecord,
    path: PathBuf,
    state: ComponentDeploymentJournalStorageState,
    diagnostic: String,
}

impl ComponentDeploymentJournalStorageError {
    pub const fn record(&self) -> &ComponentDeploymentJournalRecord {
        &self.record
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn state(&self) -> ComponentDeploymentJournalStorageState {
        self.state
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        ComponentDeploymentJournalRecord,
        PathBuf,
        ComponentDeploymentJournalStorageState,
    ) {
        (self.record, self.path, self.state)
    }
}

impl std::fmt::Display for ComponentDeploymentJournalStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ComponentDeploymentJournalStorageError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDeploymentJournalStorageDiagnostic(String);

impl ComponentDeploymentJournalStorageDiagnostic {
    pub fn diagnostic(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentDeploymentJournalStorageDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ComponentDeploymentJournalStorageDiagnostic {}

/// Exact canonical journal bytes durably published at one caller-selected
/// path. This is report/restart evidence, not live component or recovery
/// authority, and is deliberately non-clonable.
#[derive(Debug)]
#[must_use = "durable journal custody should be retained through restart reconciliation"]
pub struct DurablyStoredComponentDeploymentJournal {
    record: ComponentDeploymentJournalRecord,
    path: PathBuf,
    byte_count: usize,
    byte_fingerprint: u64,
}

impl DurablyStoredComponentDeploymentJournal {
    pub const fn record(&self) -> &ComponentDeploymentJournalRecord {
        &self.record
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn byte_count(&self) -> usize {
        self.byte_count
    }

    pub const fn byte_fingerprint(&self) -> u64 {
        self.byte_fingerprint
    }

    pub fn validate(&self) -> Result<(), ComponentDeploymentJournalStorageDiagnostic> {
        validate_regular_file(&self.path)?;
        let bytes = std::fs::read(&self.path).map_err(|error| {
            storage_diagnostic(format!("cannot read durable deployment journal: {error}"))
        })?;
        let expected = encode_component_deployment_journal(&self.record).map_err(|error| {
            storage_diagnostic(format!(
                "cannot re-encode retained deployment journal: {error}"
            ))
        })?;
        let decoded = decode_component_deployment_journal(&bytes).map_err(|error| {
            storage_diagnostic(format!(
                "durable deployment journal does not decode: {error}"
            ))
        })?;
        if decoded != self.record
            || bytes != expected
            || bytes.len() != self.byte_count
            || fingerprint_bytes(&bytes) != self.byte_fingerprint
        {
            return Err(storage_diagnostic(
                "durable deployment journal bytes or retained identity drifted",
            ));
        }
        Ok(())
    }

    pub fn into_parts(self) -> (ComponentDeploymentJournalRecord, PathBuf) {
        (self.record, self.path)
    }
}

/// Publish one new canonical phase record at an absent caller-selected path.
///
/// The same-directory hard-link transition is atomic and refuses replacement.
/// The staged file is synchronized before publication; the directory is
/// synchronized after the target appears and the staging name is removed.
pub fn durably_store_component_deployment_journal(
    record: ComponentDeploymentJournalRecord,
    path: PathBuf,
) -> Result<DurablyStoredComponentDeploymentJournal, ComponentDeploymentJournalStorageError> {
    let bytes = match encode_component_deployment_journal(&record) {
        Ok(bytes) => bytes,
        Err(error) => {
            return Err(storage_error(
                record,
                path,
                ComponentDeploymentJournalStorageState::Unpublished,
                format!("cannot encode deployment journal for storage: {error}"),
            ));
        }
    };
    let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
    else {
        return Err(storage_error(
            record,
            path,
            ComponentDeploymentJournalStorageState::Unpublished,
            "deployment journal path has no containing directory",
        ));
    };
    if path.file_name().is_none() {
        return Err(storage_error(
            record,
            path,
            ComponentDeploymentJournalStorageState::Unpublished,
            "deployment journal path has no filename",
        ));
    }
    let parent_metadata = match std::fs::symlink_metadata(&parent) {
        Ok(metadata) => metadata,
        Err(error) => {
            return Err(storage_error(
                record,
                path,
                ComponentDeploymentJournalStorageState::Unpublished,
                format!("cannot inspect deployment journal directory: {error}"),
            ));
        }
    };
    if !parent_metadata.is_dir() || parent_metadata.file_type().is_symlink() {
        return Err(storage_error(
            record,
            path,
            ComponentDeploymentJournalStorageState::Unpublished,
            "deployment journal parent must be a direct directory",
        ));
    }
    if std::fs::symlink_metadata(&path).is_ok() {
        return Err(storage_error(
            record,
            path,
            ComponentDeploymentJournalStorageState::Unpublished,
            "deployment journal destination already exists",
        ));
    }

    let (mut staged, staged_path) = match create_staging_file(&parent) {
        Ok(staged) => staged,
        Err(error) => {
            return Err(storage_error(
                record,
                path,
                ComponentDeploymentJournalStorageState::Unpublished,
                error,
            ));
        }
    };
    let unpublished_failure = |diagnostic: String| {
        let _ = std::fs::remove_file(&staged_path);
        storage_error(
            record.clone(),
            path.clone(),
            ComponentDeploymentJournalStorageState::Unpublished,
            diagnostic,
        )
    };
    if let Err(error) = staged.write_all(&bytes) {
        return Err(unpublished_failure(format!(
            "cannot stage deployment journal bytes: {error}"
        )));
    }
    if let Err(error) = staged.sync_all() {
        return Err(unpublished_failure(format!(
            "cannot synchronize staged deployment journal: {error}"
        )));
    }
    drop(staged);
    if let Err(error) = std::fs::hard_link(&staged_path, &path) {
        return Err(unpublished_failure(format!(
            "cannot atomically publish deployment journal without replacement: {error}"
        )));
    }
    let cleanup_error = std::fs::remove_file(&staged_path).err();
    let directory_sync_error = sync_parent_directory(&parent).err();
    if cleanup_error.is_some() || directory_sync_error.is_some() {
        let diagnostic = match (cleanup_error, directory_sync_error) {
            (Some(cleanup), Some(sync)) => format!(
                "deployment journal is visible but staging cleanup ({cleanup}) and directory synchronization ({sync}) are incomplete"
            ),
            (Some(cleanup), None) => format!(
                "deployment journal is durable but staging cleanup is incomplete: {cleanup}"
            ),
            (None, Some(sync)) => format!(
                "deployment journal is visible but directory synchronization is incomplete: {sync}"
            ),
            (None, None) => unreachable!(),
        };
        return Err(storage_error(
            record,
            path,
            ComponentDeploymentJournalStorageState::PublishedCleanupOrSyncIncomplete,
            diagnostic,
        ));
    }

    Ok(DurablyStoredComponentDeploymentJournal {
        record,
        path,
        byte_count: bytes.len(),
        byte_fingerprint: fingerprint_bytes(&bytes),
    })
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> std::io::Result<()> {
    std::fs::File::open(parent)?.sync_all()
}

// Stable Rust does not expose a portable directory-synchronization operation
// on non-Unix hosts. The staged file itself is synchronized before publication;
// this follows the repository's record-file durability policy for the remaining
// directory-entry step.
#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> std::io::Result<()> {
    Ok(())
}

pub fn load_durable_component_deployment_journal(
    path: PathBuf,
) -> Result<DurablyStoredComponentDeploymentJournal, ComponentDeploymentJournalStorageDiagnostic> {
    validate_regular_file(&path)?;
    let bytes = std::fs::read(&path).map_err(|error| {
        storage_diagnostic(format!("cannot read durable deployment journal: {error}"))
    })?;
    let record = decode_component_deployment_journal(&bytes).map_err(|error| {
        storage_diagnostic(format!(
            "durable deployment journal does not decode: {error}"
        ))
    })?;
    let stored = DurablyStoredComponentDeploymentJournal {
        record,
        path,
        byte_count: bytes.len(),
        byte_fingerprint: fingerprint_bytes(&bytes),
    };
    stored.validate()?;
    Ok(stored)
}

fn create_staging_file(parent: &Path) -> Result<(std::fs::File, PathBuf), String> {
    for _ in 0..64 {
        let identity = STAGING_IDENTITY.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            ".omega-deployment-journal-{}-{identity}.staged",
            std::process::id()
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!(
                    "cannot create deployment journal staging file: {error}"
                ));
            }
        }
    }
    Err("deployment journal staging namespace is exhausted".into())
}

fn validate_regular_file(path: &Path) -> Result<(), ComponentDeploymentJournalStorageDiagnostic> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        storage_diagnostic(format!(
            "cannot inspect durable deployment journal: {error}"
        ))
    })?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(storage_diagnostic(
            "durable deployment journal path is not a direct regular file",
        ));
    }
    Ok(())
}

fn storage_error(
    record: ComponentDeploymentJournalRecord,
    path: PathBuf,
    state: ComponentDeploymentJournalStorageState,
    diagnostic: impl Into<String>,
) -> ComponentDeploymentJournalStorageError {
    ComponentDeploymentJournalStorageError {
        record,
        path,
        state,
        diagnostic: diagnostic.into(),
    }
}

fn storage_diagnostic(
    diagnostic: impl Into<String>,
) -> ComponentDeploymentJournalStorageDiagnostic {
    ComponentDeploymentJournalStorageDiagnostic(diagnostic.into())
}

fn fingerprint_bytes(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}
