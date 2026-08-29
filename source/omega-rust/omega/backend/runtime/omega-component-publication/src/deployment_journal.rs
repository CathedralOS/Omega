use omega_effects::{
    ComponentEraCandidate, ComponentEraEntryState, ComponentEraPublicationReceipt,
};
use omega_executable_installation::InstalledCodeContext;
use omega_image_emission::{
    decode_installation_record, encode_installation_record, installation_fingerprint,
};

use crate::{InstalledRunnableComponent, RunnableComponentEraLedger};

const MAGIC: &[u8; 8] = b"OMGCJNL1";
pub const COMPONENT_DEPLOYMENT_JOURNAL_FORMAT_VERSION: u32 = 1;
const MAX_RECORD_BYTES: usize = 16 * 1024 * 1024;
const MAX_FIELD_BYTES: usize = 1024 * 1024;
const MAX_ADMISSIONS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentDeploymentJournalPhase {
    Prepared,
    Activated,
    Finalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentDeploymentEraOccurrence {
    era_identity: u64,
    installed_code_report_identity: u64,
    artifact_report_identity: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentDeploymentLiveEraSnapshot {
    occurrence: ComponentDeploymentEraOccurrence,
    state: ComponentEraEntryState,
    active_entries: usize,
}

impl ComponentDeploymentLiveEraSnapshot {
    pub const fn occurrence(self) -> ComponentDeploymentEraOccurrence {
        self.occurrence
    }
    pub const fn state(self) -> ComponentEraEntryState {
        self.state
    }
    pub const fn active_entries(self) -> usize {
        self.active_entries
    }
}

impl ComponentDeploymentEraOccurrence {
    pub fn new(
        era_identity: u64,
        installed_code_report_identity: u64,
        artifact_report_identity: u64,
    ) -> Result<Self, ComponentDeploymentJournalError> {
        if era_identity == 0 || installed_code_report_identity == 0 || artifact_report_identity == 0
        {
            return Err(ComponentDeploymentJournalError::new(
                "deployment-era occurrence and report identities cannot be zero",
            ));
        }
        Ok(Self {
            era_identity,
            installed_code_report_identity,
            artifact_report_identity,
        })
    }
    pub const fn era_identity(self) -> u64 {
        self.era_identity
    }
    pub const fn installed_code_report_identity(self) -> u64 {
        self.installed_code_report_identity
    }
    pub const fn artifact_report_identity(self) -> u64 {
        self.artifact_report_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentDeploymentAdmissionRecord {
    class: String,
    subject: String,
    identity: String,
}

impl ComponentDeploymentAdmissionRecord {
    pub fn new(
        class: impl Into<String>,
        subject: impl Into<String>,
        identity: impl Into<String>,
    ) -> Result<Self, ComponentDeploymentJournalError> {
        let value = Self {
            class: class.into(),
            subject: subject.into(),
            identity: identity.into(),
        };
        validate_text(&value.class, "admission class")?;
        validate_text(&value.subject, "admission subject")?;
        validate_text(&value.identity, "admission identity")?;
        Ok(value)
    }
    pub fn class(&self) -> &str {
        &self.class
    }
    pub fn subject(&self) -> &str {
        &self.subject
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
}

/// Replay input describing the envelope that accepted a candidate. Decoding
/// this snapshot never reconstructs envelope or admission authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDeploymentAcceptanceSnapshot {
    envelope_identity: String,
    canonical_envelope: Vec<u8>,
    admissions: Vec<ComponentDeploymentAdmissionRecord>,
}

impl ComponentDeploymentAcceptanceSnapshot {
    pub fn new(
        envelope_identity: impl Into<String>,
        canonical_envelope: Vec<u8>,
        mut admissions: Vec<ComponentDeploymentAdmissionRecord>,
    ) -> Result<Self, ComponentDeploymentJournalError> {
        let envelope_identity = envelope_identity.into();
        validate_text(&envelope_identity, "accepting-envelope identity")?;
        validate_bytes(&canonical_envelope, "accepting-envelope evidence")?;
        if admissions.len() > MAX_ADMISSIONS {
            return Err(ComponentDeploymentJournalError::new(
                "deployment admission set exceeds its row ceiling",
            ));
        }
        admissions.sort();
        if admissions.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ComponentDeploymentJournalError::new(
                "deployment admission set contains a duplicate exact row",
            ));
        }
        Ok(Self {
            envelope_identity,
            canonical_envelope,
            admissions,
        })
    }
    pub fn envelope_identity(&self) -> &str {
        &self.envelope_identity
    }
    pub fn canonical_envelope(&self) -> &[u8] {
        &self.canonical_envelope
    }
    pub fn admissions(&self) -> &[ComponentDeploymentAdmissionRecord] {
        &self.admissions
    }
}

/// Canonical report/reconciliation record. It contains exact evidence and
/// history, but no live installation or publication custody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDeploymentJournalRecord {
    journal_identity: u64,
    phase: ComponentDeploymentJournalPhase,
    binding_contract_identity: String,
    entry_contract_identity: String,
    prior: Option<ComponentDeploymentEraOccurrence>,
    live_eras_before: Vec<ComponentDeploymentLiveEraSnapshot>,
    candidate: ComponentDeploymentEraOccurrence,
    entry_plan_identity: String,
    entry_plan_admission_receipt_identity: String,
    acceptance: ComponentDeploymentAcceptanceSnapshot,
    installation_fingerprint: [u8; 32],
    installation_record: Vec<u8>,
}

impl ComponentDeploymentJournalRecord {
    pub const fn journal_identity(&self) -> u64 {
        self.journal_identity
    }
    pub const fn phase(&self) -> ComponentDeploymentJournalPhase {
        self.phase
    }
    pub fn binding_contract_identity(&self) -> &str {
        &self.binding_contract_identity
    }
    pub fn entry_contract_identity(&self) -> &str {
        &self.entry_contract_identity
    }
    pub const fn prior(&self) -> Option<ComponentDeploymentEraOccurrence> {
        self.prior
    }
    pub fn live_eras_before(&self) -> &[ComponentDeploymentLiveEraSnapshot] {
        &self.live_eras_before
    }
    pub const fn candidate(&self) -> ComponentDeploymentEraOccurrence {
        self.candidate
    }
    pub fn entry_plan_identity(&self) -> &str {
        &self.entry_plan_identity
    }
    pub fn entry_plan_admission_receipt_identity(&self) -> &str {
        &self.entry_plan_admission_receipt_identity
    }
    pub const fn acceptance(&self) -> &ComponentDeploymentAcceptanceSnapshot {
        &self.acceptance
    }
    pub const fn installation_fingerprint(&self) -> &[u8; 32] {
        &self.installation_fingerprint
    }
    pub fn installation_record(&self) -> &[u8] {
        &self.installation_record
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentDeploymentJournalError {
    diagnostic: String,
}
impl ComponentDeploymentJournalError {
    fn new(diagnostic: impl Into<String>) -> Self {
        Self {
            diagnostic: diagnostic.into(),
        }
    }
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
}
impl std::fmt::Display for ComponentDeploymentJournalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(f)
    }
}
impl std::error::Error for ComponentDeploymentJournalError {}

#[derive(Debug)]
#[must_use = "prepared deployment retains publication custody"]
pub struct PreparedComponentDeploymentJournal {
    record: ComponentDeploymentJournalRecord,
    candidate: ComponentEraCandidate,
    receipt: ComponentEraPublicationReceipt,
    installed_context: InstalledCodeContext,
    runnable: InstalledRunnableComponent,
}

pub fn prepare_component_deployment(
    journal_identity: u64,
    ledger: &RunnableComponentEraLedger,
    candidate: ComponentEraCandidate,
    receipt: ComponentEraPublicationReceipt,
    runnable: InstalledRunnableComponent,
    acceptance: ComponentDeploymentAcceptanceSnapshot,
) -> Result<PreparedComponentDeploymentJournal, Box<ComponentDeploymentPreparationError>> {
    if journal_identity == 0 || candidate.era_identity == 0 {
        return Err(preparation_error(
            candidate,
            receipt,
            runnable,
            acceptance,
            "journal and candidate-era identities cannot be zero",
        ));
    }
    if candidate.artifact_occurrence_digest != runnable.installed().occurrence_digest()
        || candidate.artifact_instance_compatibility_report_identity
            != runnable.installed_code().normalized_identity()
    {
        return Err(preparation_error(
            candidate,
            receipt,
            runnable,
            acceptance,
            "journal candidate names a different installed-code occurrence",
        ));
    }
    let mut live_eras_before = Vec::new();
    for (era_identity, state, active_entries) in ledger.live_eras() {
        let Some(retained) = ledger.retained_component(era_identity) else {
            return Err(preparation_error(
                candidate,
                receipt,
                runnable,
                acceptance,
                "live component era has no retained runnable occurrence",
            ));
        };
        let occurrence = match ComponentDeploymentEraOccurrence::new(
            era_identity,
            retained.installed_code().normalized_identity(),
            retained.artifact().normalized_identity(),
        ) {
            Ok(value) => value,
            Err(error) => {
                return Err(preparation_error(
                    candidate,
                    receipt,
                    runnable,
                    acceptance,
                    error.diagnostic,
                ));
            }
        };
        live_eras_before.push(ComponentDeploymentLiveEraSnapshot {
            occurrence,
            state,
            active_entries,
        });
    }
    live_eras_before.sort_by_key(|row| row.occurrence.era_identity);
    let prior = if let Some(era_identity) = ledger.current_era() {
        let Some(retained) = ledger.retained_component(era_identity) else {
            return Err(preparation_error(
                candidate,
                receipt,
                runnable,
                acceptance,
                "current component era has no retained runnable occurrence",
            ));
        };
        match ComponentDeploymentEraOccurrence::new(
            era_identity,
            retained.installed_code().normalized_identity(),
            retained.artifact().normalized_identity(),
        ) {
            Ok(value) => Some(value),
            Err(error) => {
                return Err(preparation_error(
                    candidate,
                    receipt,
                    runnable,
                    acceptance,
                    error.diagnostic,
                ));
            }
        }
    } else {
        None
    };
    let installation_record =
        match encode_installation_record(runnable.installed_artifact().installation()) {
            Ok(value) => value,
            Err(error) => {
                return Err(preparation_error(
                    candidate,
                    receipt,
                    runnable,
                    acceptance,
                    format!("cannot encode canonical installation evidence: {error}"),
                ));
            }
        };
    let fingerprint = match installation_fingerprint(runnable.installed_artifact().installation()) {
        Ok(value) => value,
        Err(error) => {
            return Err(preparation_error(
                candidate,
                receipt,
                runnable,
                acceptance,
                format!("cannot fingerprint canonical installation evidence: {error}"),
            ));
        }
    };
    let candidate_occurrence = match ComponentDeploymentEraOccurrence::new(
        candidate.era_identity,
        candidate.artifact_instance_compatibility_report_identity,
        runnable.artifact().normalized_identity(),
    ) {
        Ok(value) => value,
        Err(error) => {
            return Err(preparation_error(
                candidate,
                receipt,
                runnable,
                acceptance,
                error.diagnostic,
            ));
        }
    };
    let record = ComponentDeploymentJournalRecord {
        journal_identity,
        phase: ComponentDeploymentJournalPhase::Prepared,
        binding_contract_identity: candidate.binding_contract_identity.clone(),
        entry_contract_identity: candidate.entry_contract_identity.clone(),
        prior,
        live_eras_before,
        candidate: candidate_occurrence,
        entry_plan_identity: candidate.entry_plan_identity.clone(),
        entry_plan_admission_receipt_identity: candidate
            .entry_plan_admission_receipt_identity
            .clone(),
        acceptance,
        installation_fingerprint: *fingerprint.as_bytes(),
        installation_record,
    };
    let installed_context = runnable.installed().receipt_context();
    Ok(PreparedComponentDeploymentJournal {
        record,
        candidate,
        receipt,
        installed_context,
        runnable,
    })
}

fn preparation_error(
    candidate: ComponentEraCandidate,
    receipt: ComponentEraPublicationReceipt,
    runnable: InstalledRunnableComponent,
    acceptance: ComponentDeploymentAcceptanceSnapshot,
    diagnostic: impl Into<String>,
) -> Box<ComponentDeploymentPreparationError> {
    Box::new(ComponentDeploymentPreparationError {
        candidate,
        receipt,
        runnable,
        acceptance,
        diagnostic: diagnostic.into(),
    })
}

impl PreparedComponentDeploymentJournal {
    pub const fn record(&self) -> &ComponentDeploymentJournalRecord {
        &self.record
    }
    pub fn activate(
        self,
        durable_predecessor: &ComponentDeploymentJournalRecord,
        ledger: &mut RunnableComponentEraLedger,
    ) -> Result<ActivatedComponentDeploymentJournal, Box<ComponentDeploymentActivationError>> {
        if durable_predecessor != &self.record
            || durable_predecessor.phase != ComponentDeploymentJournalPhase::Prepared
        {
            return Err(Box::new(ComponentDeploymentActivationError {
                prepared: self,
                diagnostic: "activation requires the exact durable Prepared predecessor".into(),
            }));
        }
        let Self {
            mut record,
            candidate,
            receipt,
            installed_context,
            runnable,
        } = self;
        if let Err(error) = ledger.publish(candidate, receipt, runnable) {
            let diagnostic = error.diagnostic().to_owned();
            let (candidate, receipt, runnable) = error.into_parts();
            return Err(Box::new(ComponentDeploymentActivationError {
                prepared: PreparedComponentDeploymentJournal {
                    record,
                    candidate,
                    receipt,
                    installed_context,
                    runnable,
                },
                diagnostic,
            }));
        }
        debug_assert_eq!(ledger.current_era(), Some(record.candidate.era_identity));
        debug_assert!(
            ledger
                .retained_component(record.candidate.era_identity)
                .is_some()
        );
        debug_assert_eq!(
            ledger
                .retained_component(record.candidate.era_identity)
                .map(|retained| retained.installed().receipt_context()),
            Some(installed_context.clone())
        );
        record.phase = ComponentDeploymentJournalPhase::Activated;
        Ok(ActivatedComponentDeploymentJournal {
            record,
            installed_context,
        })
    }
}

#[derive(Debug)]
#[must_use = "activated deployment must be durably finalized or reconciled"]
pub struct ActivatedComponentDeploymentJournal {
    record: ComponentDeploymentJournalRecord,
    installed_context: InstalledCodeContext,
}
impl ActivatedComponentDeploymentJournal {
    pub const fn record(&self) -> &ComponentDeploymentJournalRecord {
        &self.record
    }
    pub fn finalize(
        mut self,
        durable_predecessor: &ComponentDeploymentJournalRecord,
        ledger: &RunnableComponentEraLedger,
    ) -> Result<FinalizedComponentDeploymentJournal, ComponentDeploymentFinalizationError> {
        if durable_predecessor != &self.record
            || durable_predecessor.phase != ComponentDeploymentJournalPhase::Activated
        {
            return Err(ComponentDeploymentFinalizationError::new(
                self,
                "finalization requires the exact durable Activated predecessor",
            ));
        }
        if ledger.current_era() != Some(self.record.candidate.era_identity) {
            return Err(ComponentDeploymentFinalizationError::new(
                self,
                "finalization candidate is not the exact current published era",
            ));
        }
        let Some(retained) = ledger.retained_component(self.record.candidate.era_identity) else {
            return Err(ComponentDeploymentFinalizationError::new(
                self,
                "finalization candidate has no retained runnable custody",
            ));
        };
        if retained.installed_code().normalized_identity()
            != self.record.candidate.installed_code_report_identity
            || retained.artifact().normalized_identity()
                != self.record.candidate.artifact_report_identity
        {
            return Err(ComponentDeploymentFinalizationError::new(
                self,
                "finalization retained occurrence report identities disagree with the journal candidate",
            ));
        }
        if retained.installed().receipt_context() != self.installed_context {
            return Err(ComponentDeploymentFinalizationError::new(
                self,
                "finalization retained occurrence does not match the exact activated installed-code evidence",
            ));
        }
        let retained_installation_record =
            match encode_installation_record(retained.installed_artifact().installation()) {
                Ok(record) => record,
                Err(error) => {
                    return Err(ComponentDeploymentFinalizationError::new(
                        self,
                        format!(
                            "finalization cannot replay retained installation evidence: {error}"
                        ),
                    ));
                }
            };
        if retained_installation_record != self.record.installation_record {
            return Err(ComponentDeploymentFinalizationError::new(
                self,
                "finalization retained occurrence has different canonical installation evidence",
            ));
        }
        self.record.phase = ComponentDeploymentJournalPhase::Finalized;
        Ok(FinalizedComponentDeploymentJournal {
            record: self.record,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedComponentDeploymentJournal {
    record: ComponentDeploymentJournalRecord,
}
impl FinalizedComponentDeploymentJournal {
    pub const fn record(&self) -> &ComponentDeploymentJournalRecord {
        &self.record
    }
}

#[derive(Debug)]
pub struct ComponentDeploymentPreparationError {
    candidate: ComponentEraCandidate,
    receipt: ComponentEraPublicationReceipt,
    runnable: InstalledRunnableComponent,
    acceptance: ComponentDeploymentAcceptanceSnapshot,
    diagnostic: String,
}
impl ComponentDeploymentPreparationError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
    pub fn into_parts(
        self,
    ) -> (
        ComponentEraCandidate,
        ComponentEraPublicationReceipt,
        InstalledRunnableComponent,
        ComponentDeploymentAcceptanceSnapshot,
    ) {
        (self.candidate, self.receipt, self.runnable, self.acceptance)
    }
}
impl std::fmt::Display for ComponentDeploymentPreparationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(f)
    }
}
impl std::error::Error for ComponentDeploymentPreparationError {}

#[derive(Debug)]
pub struct ComponentDeploymentActivationError {
    prepared: PreparedComponentDeploymentJournal,
    diagnostic: String,
}
impl ComponentDeploymentActivationError {
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
    pub fn into_prepared(self) -> PreparedComponentDeploymentJournal {
        self.prepared
    }
}
impl std::fmt::Display for ComponentDeploymentActivationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(f)
    }
}
impl std::error::Error for ComponentDeploymentActivationError {}

#[derive(Debug)]
pub struct ComponentDeploymentFinalizationError {
    activated: ActivatedComponentDeploymentJournal,
    diagnostic: String,
}
impl ComponentDeploymentFinalizationError {
    fn new(activated: ActivatedComponentDeploymentJournal, diagnostic: impl Into<String>) -> Self {
        Self {
            activated,
            diagnostic: diagnostic.into(),
        }
    }
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }
    pub fn into_activated(self) -> ActivatedComponentDeploymentJournal {
        self.activated
    }
}
impl std::fmt::Display for ComponentDeploymentFinalizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.diagnostic.fmt(f)
    }
}
impl std::error::Error for ComponentDeploymentFinalizationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentDeploymentRecoveryChoice {
    RollBackToPrior,
    RollForwardCandidate,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentDeploymentRestartReconciliation {
    PolicyRequired {
        phase: ComponentDeploymentJournalPhase,
        choices: Vec<ComponentDeploymentRecoveryChoice>,
    },
    Complete {
        candidate: ComponentDeploymentEraOccurrence,
    },
}

/// Exposes unresolved policy without choosing rollback or roll-forward.
pub fn reconcile_component_deployment_restart(
    record: &ComponentDeploymentJournalRecord,
    expected_journal_identity: u64,
    expected_binding_contract_identity: &str,
    expected_entry_contract_identity: &str,
) -> Result<ComponentDeploymentRestartReconciliation, ComponentDeploymentJournalError> {
    if record.journal_identity != expected_journal_identity
        || record.binding_contract_identity != expected_binding_contract_identity
        || record.entry_contract_identity != expected_entry_contract_identity
    {
        return Err(ComponentDeploymentJournalError::new(
            "deployment restart record names the wrong journal or service slot",
        ));
    }
    Ok(match record.phase {
        ComponentDeploymentJournalPhase::Prepared | ComponentDeploymentJournalPhase::Activated => {
            let mut choices = vec![ComponentDeploymentRecoveryChoice::RollForwardCandidate];
            if record.prior.is_some() {
                choices.insert(0, ComponentDeploymentRecoveryChoice::RollBackToPrior);
            }
            ComponentDeploymentRestartReconciliation::PolicyRequired {
                phase: record.phase,
                choices,
            }
        }
        ComponentDeploymentJournalPhase::Finalized => {
            ComponentDeploymentRestartReconciliation::Complete {
                candidate: record.candidate,
            }
        }
    })
}

pub fn encode_component_deployment_journal(
    record: &ComponentDeploymentJournalRecord,
) -> Result<Vec<u8>, ComponentDeploymentJournalError> {
    validate_record(record)?;
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    put_u32(&mut out, COMPONENT_DEPLOYMENT_JOURNAL_FORMAT_VERSION);
    put_u64(&mut out, record.journal_identity);
    out.push(match record.phase {
        ComponentDeploymentJournalPhase::Prepared => 0,
        ComponentDeploymentJournalPhase::Activated => 1,
        ComponentDeploymentJournalPhase::Finalized => 2,
    });
    put_text(&mut out, &record.binding_contract_identity)?;
    put_text(&mut out, &record.entry_contract_identity)?;
    out.push(u8::from(record.prior.is_some()));
    if let Some(prior) = record.prior {
        put_occurrence(&mut out, prior);
    }
    put_u32(&mut out, record.live_eras_before.len() as u32);
    for row in &record.live_eras_before {
        put_occurrence(&mut out, row.occurrence);
        out.push(match row.state {
            ComponentEraEntryState::Open => 0,
            ComponentEraEntryState::Closing => 1,
            ComponentEraEntryState::Quiescent => 2,
        });
        put_u64(
            &mut out,
            u64::try_from(row.active_entries).map_err(|_| {
                ComponentDeploymentJournalError::new("live-era active-entry count exceeds u64")
            })?,
        );
    }
    put_occurrence(&mut out, record.candidate);
    put_text(&mut out, &record.entry_plan_identity)?;
    put_text(&mut out, &record.entry_plan_admission_receipt_identity)?;
    put_text(&mut out, &record.acceptance.envelope_identity)?;
    put_bytes(&mut out, &record.acceptance.canonical_envelope)?;
    put_u32(&mut out, record.acceptance.admissions.len() as u32);
    for row in &record.acceptance.admissions {
        put_text(&mut out, &row.class)?;
        put_text(&mut out, &row.subject)?;
        put_text(&mut out, &row.identity)?;
    }
    out.extend_from_slice(&record.installation_fingerprint);
    put_bytes(&mut out, &record.installation_record)?;
    if out.len() > MAX_RECORD_BYTES {
        return Err(ComponentDeploymentJournalError::new(
            "deployment journal exceeds its canonical byte ceiling",
        ));
    }
    Ok(out)
}

pub fn decode_component_deployment_journal(
    bytes: &[u8],
) -> Result<ComponentDeploymentJournalRecord, ComponentDeploymentJournalError> {
    if bytes.len() > MAX_RECORD_BYTES {
        return Err(ComponentDeploymentJournalError::new(
            "deployment journal exceeds its canonical byte ceiling",
        ));
    }
    let mut cursor = Cursor { bytes, at: 0 };
    if cursor.take(MAGIC.len())? != MAGIC {
        return Err(ComponentDeploymentJournalError::new(
            "deployment journal has the wrong magic",
        ));
    }
    if cursor.u32()? != COMPONENT_DEPLOYMENT_JOURNAL_FORMAT_VERSION {
        return Err(ComponentDeploymentJournalError::new(
            "unsupported deployment journal format version",
        ));
    }
    let journal_identity = cursor.u64()?;
    let phase = match cursor.byte()? {
        0 => ComponentDeploymentJournalPhase::Prepared,
        1 => ComponentDeploymentJournalPhase::Activated,
        2 => ComponentDeploymentJournalPhase::Finalized,
        _ => {
            return Err(ComponentDeploymentJournalError::new(
                "invalid deployment journal phase",
            ));
        }
    };
    let binding_contract_identity = cursor.text()?;
    let entry_contract_identity = cursor.text()?;
    let prior = match cursor.byte()? {
        0 => None,
        1 => Some(cursor.occurrence()?),
        _ => {
            return Err(ComponentDeploymentJournalError::new(
                "invalid prior-era presence tag",
            ));
        }
    };
    let live_count = cursor.u32()? as usize;
    if live_count > MAX_ADMISSIONS {
        return Err(ComponentDeploymentJournalError::new(
            "deployment live-era roster exceeds its row ceiling",
        ));
    }
    let mut live_eras_before = Vec::with_capacity(live_count);
    for _ in 0..live_count {
        let occurrence = cursor.occurrence()?;
        let state = match cursor.byte()? {
            0 => ComponentEraEntryState::Open,
            1 => ComponentEraEntryState::Closing,
            2 => ComponentEraEntryState::Quiescent,
            _ => {
                return Err(ComponentDeploymentJournalError::new(
                    "invalid live-era state tag",
                ));
            }
        };
        let active_entries = usize::try_from(cursor.u64()?).map_err(|_| {
            ComponentDeploymentJournalError::new("live-era active-entry count exceeds usize")
        })?;
        live_eras_before.push(ComponentDeploymentLiveEraSnapshot {
            occurrence,
            state,
            active_entries,
        });
    }
    let candidate = cursor.occurrence()?;
    let entry_plan_identity = cursor.text()?;
    let entry_plan_admission_receipt_identity = cursor.text()?;
    let envelope_identity = cursor.text()?;
    let canonical_envelope = cursor.bytes()?;
    let count = cursor.u32()? as usize;
    if count > MAX_ADMISSIONS {
        return Err(ComponentDeploymentJournalError::new(
            "deployment admission set exceeds its row ceiling",
        ));
    }
    let mut admissions = Vec::with_capacity(count);
    for _ in 0..count {
        admissions.push(ComponentDeploymentAdmissionRecord::new(
            cursor.text()?,
            cursor.text()?,
            cursor.text()?,
        )?);
    }
    let mut installation_fingerprint = [0; 32];
    installation_fingerprint.copy_from_slice(cursor.take(32)?);
    let installation_record = cursor.bytes()?;
    if cursor.at != bytes.len() {
        return Err(ComponentDeploymentJournalError::new(
            "deployment journal contains trailing bytes",
        ));
    }
    let acceptance = ComponentDeploymentAcceptanceSnapshot::new(
        envelope_identity,
        canonical_envelope,
        admissions,
    )?;
    let record = ComponentDeploymentJournalRecord {
        journal_identity,
        phase,
        binding_contract_identity,
        entry_contract_identity,
        prior,
        live_eras_before,
        candidate,
        entry_plan_identity,
        entry_plan_admission_receipt_identity,
        acceptance,
        installation_fingerprint,
        installation_record,
    };
    validate_record(&record)?;
    if encode_component_deployment_journal(&record)? != bytes {
        return Err(ComponentDeploymentJournalError::new(
            "deployment journal is not canonically encoded",
        ));
    }
    Ok(record)
}

fn validate_record(
    record: &ComponentDeploymentJournalRecord,
) -> Result<(), ComponentDeploymentJournalError> {
    if record.journal_identity == 0 {
        return Err(ComponentDeploymentJournalError::new(
            "deployment journal identity cannot be zero",
        ));
    }
    validate_text(
        &record.binding_contract_identity,
        "binding-contract identity",
    )?;
    if record
        .live_eras_before
        .windows(2)
        .any(|pair| pair[0].occurrence.era_identity >= pair[1].occurrence.era_identity)
    {
        return Err(ComponentDeploymentJournalError::new(
            "deployment live-era roster is not strictly ordered",
        ));
    }
    validate_text(&record.entry_contract_identity, "entry-contract identity")?;
    validate_text(&record.entry_plan_identity, "entry-plan identity")?;
    validate_text(
        &record.entry_plan_admission_receipt_identity,
        "entry-plan admission identity",
    )?;
    let decoded = decode_installation_record(&record.installation_record).map_err(|error| {
        ComponentDeploymentJournalError::new(format!(
            "deployment journal installation evidence does not decode: {error}"
        ))
    })?;
    let canonical = encode_installation_record(&decoded).map_err(|error| {
        ComponentDeploymentJournalError::new(format!(
            "deployment journal installation evidence cannot re-encode: {error}"
        ))
    })?;
    if canonical != record.installation_record {
        return Err(ComponentDeploymentJournalError::new(
            "deployment journal installation evidence is not canonical",
        ));
    }
    let fingerprint = installation_fingerprint(&decoded).map_err(|error| {
        ComponentDeploymentJournalError::new(format!(
            "deployment journal installation fingerprint cannot replay: {error}"
        ))
    })?;
    if fingerprint.as_bytes() != &record.installation_fingerprint {
        return Err(ComponentDeploymentJournalError::new(
            "deployment journal installation fingerprint does not match its evidence",
        ));
    }
    Ok(())
}

fn validate_text(value: &str, label: &str) -> Result<(), ComponentDeploymentJournalError> {
    if value.is_empty() || value.len() > MAX_FIELD_BYTES {
        return Err(ComponentDeploymentJournalError::new(format!(
            "{label} is empty or exceeds its byte ceiling"
        )));
    }
    Ok(())
}
fn validate_bytes(value: &[u8], label: &str) -> Result<(), ComponentDeploymentJournalError> {
    if value.is_empty() || value.len() > MAX_FIELD_BYTES {
        return Err(ComponentDeploymentJournalError::new(format!(
            "{label} is empty or exceeds its byte ceiling"
        )));
    }
    Ok(())
}
fn put_occurrence(out: &mut Vec<u8>, value: ComponentDeploymentEraOccurrence) {
    put_u64(out, value.era_identity);
    put_u64(out, value.installed_code_report_identity);
    put_u64(out, value.artifact_report_identity);
}
fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put_text(out: &mut Vec<u8>, value: &str) -> Result<(), ComponentDeploymentJournalError> {
    validate_text(value, "journal text field")?;
    put_bytes(out, value.as_bytes())
}
fn put_bytes(out: &mut Vec<u8>, value: &[u8]) -> Result<(), ComponentDeploymentJournalError> {
    validate_bytes(value, "journal byte field")?;
    put_u32(out, value.len() as u32);
    out.extend_from_slice(value);
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], ComponentDeploymentJournalError> {
        let end = self.at.checked_add(count).ok_or_else(|| {
            ComponentDeploymentJournalError::new("deployment journal offset overflow")
        })?;
        let value = self
            .bytes
            .get(self.at..end)
            .ok_or_else(|| ComponentDeploymentJournalError::new("truncated deployment journal"))?;
        self.at = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, ComponentDeploymentJournalError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, ComponentDeploymentJournalError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four bytes"),
        ))
    }
    fn u64(&mut self) -> Result<u64, ComponentDeploymentJournalError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight bytes"),
        ))
    }
    fn bytes(&mut self) -> Result<Vec<u8>, ComponentDeploymentJournalError> {
        let count = self.u32()? as usize;
        if count > MAX_FIELD_BYTES {
            return Err(ComponentDeploymentJournalError::new(
                "deployment journal field exceeds its byte ceiling",
            ));
        }
        Ok(self.take(count)?.to_vec())
    }
    fn text(&mut self) -> Result<String, ComponentDeploymentJournalError> {
        String::from_utf8(self.bytes()?).map_err(|_| {
            ComponentDeploymentJournalError::new("deployment journal text is not UTF-8")
        })
    }
    fn occurrence(
        &mut self,
    ) -> Result<ComponentDeploymentEraOccurrence, ComponentDeploymentJournalError> {
        ComponentDeploymentEraOccurrence::new(self.u64()?, self.u64()?, self.u64()?)
    }
}
