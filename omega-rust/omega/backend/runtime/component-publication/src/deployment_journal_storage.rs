//! Durable, policy-free storage for canonical deployment-journal records.
//!
//! Cathedral chooses the journal path and recovery policy. This adapter only
//! publishes one new canonical phase record without overwriting an older row,
//! flushes the file and containing directory, and independently replays bytes.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use executable_installation::InstalledCodeContext;
use image_emission::encode_installation_record;

use crate::{
    ComponentDeploymentEraOccurrence, ComponentDeploymentJournalRecord,
    ComponentDeploymentRecoveryChoice, ComponentDeploymentRestartReconciliation,
    RunnableComponentEraLedger, decode_component_deployment_journal,
    encode_component_deployment_journal, reconcile_component_deployment_restart,
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
    /// Non-authoritative compact coordinate for restart reports. Validation
    /// authorizes only through exact canonical bytes plus decoded-record replay.
    byte_compatibility_report_fingerprint: u64,
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

    pub const fn byte_compatibility_report_fingerprint(&self) -> u64 {
        self.byte_compatibility_report_fingerprint
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
            || non_authoritative_byte_compatibility_fingerprint(&bytes)
                != self.byte_compatibility_report_fingerprint
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

/// Caller-selected restart decision joined to one exact current runtime
/// occurrence. The durable record remains report evidence; the retained
/// installed context comes only from the live ledger and is deliberately not
/// reconstructed by decoding journal bytes.
#[derive(Debug)]
#[must_use = "restart recovery custody must be consumed by the selected runtime policy"]
pub struct ComponentDeploymentRuntimeRecoveryContinuation {
    durable: DurablyStoredComponentDeploymentJournal,
    choice: ComponentDeploymentRecoveryChoice,
    occurrence: ComponentDeploymentEraOccurrence,
    ledger: RunnableComponentEraLedger,
    installed_context: InstalledCodeContext,
}

impl ComponentDeploymentRuntimeRecoveryContinuation {
    pub const fn durable(&self) -> &DurablyStoredComponentDeploymentJournal {
        &self.durable
    }

    pub const fn choice(&self) -> ComponentDeploymentRecoveryChoice {
        self.choice
    }

    pub const fn occurrence(&self) -> ComponentDeploymentEraOccurrence {
        self.occurrence
    }

    pub const fn ledger(&self) -> &RunnableComponentEraLedger {
        &self.ledger
    }

    /// Rejoin the continuation to the same exact live ledger/component
    /// occurrence before the caller-selected policy consumes it.
    pub fn validate(&self) -> Result<(), ComponentDeploymentRuntimeRecoveryDiagnostic> {
        self.durable.validate().map_err(|error| {
            recovery_diagnostic(format!(
                "durable deployment journal no longer validates: {error}"
            ))
        })?;
        validate_live_recovery_occurrence(
            &self.ledger,
            self.durable.record(),
            self.occurrence,
            Some(&self.installed_context),
        )
        .map_err(recovery_diagnostic)?;
        Ok(())
    }

    pub fn into_parts(
        self,
    ) -> (
        DurablyStoredComponentDeploymentJournal,
        ComponentDeploymentRecoveryChoice,
        RunnableComponentEraLedger,
    ) {
        (self.durable, self.choice, self.ledger)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDeploymentRuntimeRecoveryDiagnostic(String);

impl ComponentDeploymentRuntimeRecoveryDiagnostic {
    pub fn diagnostic(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentDeploymentRuntimeRecoveryDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for ComponentDeploymentRuntimeRecoveryDiagnostic {}

/// Rejoin a caller-supplied Cathedral recovery choice to validated durable
/// evidence and the exact current live runtime occurrence. This adapter never
/// chooses, publishes, retires, or redirects an era.
pub fn join_component_deployment_restart_to_runtime(
    durable: DurablyStoredComponentDeploymentJournal,
    choice: ComponentDeploymentRecoveryChoice,
    expected_journal_identity: u64,
    expected_binding_contract_identity: &str,
    expected_entry_contract_identity: &str,
    ledger: RunnableComponentEraLedger,
) -> Result<ComponentDeploymentRuntimeRecoveryContinuation, ComponentDeploymentRuntimeRecoveryError>
{
    if let Err(error) = durable.validate() {
        return Err(recovery_error(
            durable,
            choice,
            ledger,
            format!("durable deployment journal does not validate: {error}"),
        ));
    }
    let reconciliation = match reconcile_component_deployment_restart(
        durable.record(),
        expected_journal_identity,
        expected_binding_contract_identity,
        expected_entry_contract_identity,
    ) {
        Ok(value) => value,
        Err(error) => {
            return Err(recovery_error(durable, choice, ledger, error.to_string()));
        }
    };
    let ComponentDeploymentRestartReconciliation::PolicyRequired { choices, .. } = reconciliation
    else {
        return Err(recovery_error(
            durable,
            choice,
            ledger,
            "finalized deployment journal requires no rollback or roll-forward policy",
        ));
    };
    if !choices.contains(&choice) {
        return Err(recovery_error(
            durable,
            choice,
            ledger,
            "caller-selected deployment recovery choice is not available for this journal",
        ));
    }
    if ledger.binding_contract_identity() != expected_binding_contract_identity
        || ledger.entry_contract_identity() != expected_entry_contract_identity
    {
        return Err(recovery_error(
            durable,
            choice,
            ledger,
            "live component ledger names a different service slot",
        ));
    }
    let occurrence = match choice {
        ComponentDeploymentRecoveryChoice::RollBackToPrior => durable
            .record()
            .prior()
            .expect("reconciliation offered rollback only with a prior era"),
        ComponentDeploymentRecoveryChoice::RollForwardCandidate => durable.record().candidate(),
    };
    let installed_context =
        match validate_live_recovery_occurrence(&ledger, durable.record(), occurrence, None) {
            Ok(value) => value,
            Err(diagnostic) => return Err(recovery_error(durable, choice, ledger, diagnostic)),
        };
    Ok(ComponentDeploymentRuntimeRecoveryContinuation {
        durable,
        choice,
        occurrence,
        ledger,
        installed_context,
    })
}

fn validate_live_recovery_occurrence(
    ledger: &RunnableComponentEraLedger,
    record: &ComponentDeploymentJournalRecord,
    occurrence: ComponentDeploymentEraOccurrence,
    expected_context: Option<&InstalledCodeContext>,
) -> Result<InstalledCodeContext, String> {
    if ledger.binding_contract_identity() != record.binding_contract_identity()
        || ledger.entry_contract_identity() != record.entry_contract_identity()
    {
        return Err("recovery continuation names a different live component ledger".into());
    }
    if ledger.current_era() != Some(occurrence.era_identity()) {
        return Err("caller-selected recovery era is not the current live era".into());
    }
    let retained = ledger
        .retained_component(occurrence.era_identity())
        .ok_or_else(|| {
            "caller-selected recovery era has no retained runnable component".to_owned()
        })?;
    let installed_context = retained.installed().receipt_context();
    if installed_context.occurrence_digest().as_bytes() != &occurrence.artifact_occurrence_digest()
        || retained.installed_code().normalized_identity()
            != occurrence.installed_code_report_identity()
        || retained.artifact().normalized_identity() != occurrence.artifact_report_identity()
    {
        return Err(
            "live runtime component does not match the journal's exact era occurrence".into(),
        );
    }
    if expected_context.is_some_and(|expected| expected != &installed_context) {
        return Err("live runtime component occurrence changed after recovery join".into());
    }
    if occurrence == record.candidate() {
        let installation = encode_installation_record(retained.installed_artifact().installation())
            .map_err(|error| {
                format!("cannot replay live candidate installation evidence: {error}")
            })?;
        if installation != record.installation_record() {
            return Err(
                "live candidate has different canonical installation evidence than the journal"
                    .into(),
            );
        }
    }
    Ok(installed_context)
}

#[derive(Debug)]
pub struct ComponentDeploymentRuntimeRecoveryError {
    durable: DurablyStoredComponentDeploymentJournal,
    choice: ComponentDeploymentRecoveryChoice,
    ledger: RunnableComponentEraLedger,
    diagnostic: String,
}

impl ComponentDeploymentRuntimeRecoveryError {
    pub const fn durable(&self) -> &DurablyStoredComponentDeploymentJournal {
        &self.durable
    }

    pub const fn choice(&self) -> ComponentDeploymentRecoveryChoice {
        self.choice
    }

    pub const fn ledger(&self) -> &RunnableComponentEraLedger {
        &self.ledger
    }

    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    pub fn into_parts(
        self,
    ) -> (
        DurablyStoredComponentDeploymentJournal,
        ComponentDeploymentRecoveryChoice,
        RunnableComponentEraLedger,
    ) {
        (self.durable, self.choice, self.ledger)
    }
}

impl std::fmt::Display for ComponentDeploymentRuntimeRecoveryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(formatter)
    }
}

impl std::error::Error for ComponentDeploymentRuntimeRecoveryError {}

fn recovery_error(
    durable: DurablyStoredComponentDeploymentJournal,
    choice: ComponentDeploymentRecoveryChoice,
    ledger: RunnableComponentEraLedger,
    diagnostic: impl Into<String>,
) -> ComponentDeploymentRuntimeRecoveryError {
    ComponentDeploymentRuntimeRecoveryError {
        durable,
        choice,
        ledger,
        diagnostic: diagnostic.into(),
    }
}

fn recovery_diagnostic(
    diagnostic: impl Into<String>,
) -> ComponentDeploymentRuntimeRecoveryDiagnostic {
    ComponentDeploymentRuntimeRecoveryDiagnostic(diagnostic.into())
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
        byte_compatibility_report_fingerprint: non_authoritative_byte_compatibility_fingerprint(
            &bytes,
        ),
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
        byte_compatibility_report_fingerprint: non_authoritative_byte_compatibility_fingerprint(
            &bytes,
        ),
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

fn non_authoritative_byte_compatibility_fingerprint(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}
