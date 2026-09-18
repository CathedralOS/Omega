//! Entry, quiescence, publication and retirement of one component era, as receipts
//! rather than state transitions.

use std::collections::BTreeSet;

use installation_evidence::InstalledArtifactOccurrenceDigest;

use crate::{
    CoexistingExecutableTcbReport, CoexistingExecutableTcbSet, ExecutableTcbProfileAcceptance,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentEraLedgerId(u64);

impl ComponentEraLedgerId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, String> {
        if identity == 0 {
            return Err("component-era ledger identity cannot be zero".into());
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramLocalRootEpochLeaseId(u64);

impl ProgramLocalRootEpochLeaseId {
    pub fn from_normalized_identity(identity: u64) -> Result<Self, String> {
        if identity == 0 {
            return Err("program-local root epoch-lease identity cannot be zero".into());
        }
        Ok(Self(identity))
    }

    pub const fn normalized_identity(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentEraEntryState {
    Open,
    Closing,
    Quiescent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraCandidate {
    pub era_identity: u64,
    /// Collision-resistant commitment derived from the exact installed
    /// artifact occurrence retained by runtime publication custody.
    pub artifact_occurrence_digest: InstalledArtifactOccurrenceDigest,
    /// Compact compatibility report coordinate only. Publication and lease
    /// joins require `artifact_occurrence_digest`; this value cannot authorize
    /// an artifact occurrence by itself.
    pub artifact_instance_compatibility_report_identity: u64,
    pub binding_contract_identity: String,
    pub entry_contract_identity: String,
    pub entry_plan_identity: String,
    pub entry_plan_admission_receipt_identity: String,
    pub executable_tcb_acceptance: ExecutableTcbProfileAcceptance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComponentEraRecord {
    candidate: ComponentEraCandidate,
    state: ComponentEraEntryState,
    active_entries: usize,
    program_local_root_epoch_leases: BTreeSet<ProgramLocalRootEpochLeaseId>,
}

/// Opaque lifecycle hold for program-local authority introduced in one exact
/// published component era.
///
/// The lease is deliberately non-clonable. Its private bindings prevent a
/// root installer from substituting a different ledger, era, entry contract,
/// or published entry plan when it later returns the hold.
#[derive(Debug, PartialEq, Eq)]
pub struct ProgramLocalRootEpochLease {
    identity: ProgramLocalRootEpochLeaseId,
    ledger: ComponentEraLedgerId,
    binding_contract_identity: String,
    entry_contract_identity: String,
    era_identity: u64,
    artifact_occurrence_digest: InstalledArtifactOccurrenceDigest,
    artifact_instance_compatibility_report_identity: u64,
    entry_plan_identity: String,
    entry_plan_admission_receipt_identity: String,
    candidate: ComponentEraCandidate,
}

impl ProgramLocalRootEpochLease {
    pub const fn identity(&self) -> ProgramLocalRootEpochLeaseId {
        self.identity
    }

    pub const fn ledger(&self) -> ComponentEraLedgerId {
        self.ledger
    }

    pub fn entry_contract_identity(&self) -> &str {
        &self.entry_contract_identity
    }

    pub const fn era_identity(&self) -> u64 {
        self.era_identity
    }

    pub const fn artifact_occurrence_digest(&self) -> InstalledArtifactOccurrenceDigest {
        self.artifact_occurrence_digest
    }

    pub const fn artifact_instance_compatibility_report_identity(&self) -> u64 {
        self.artifact_instance_compatibility_report_identity
    }
}

/// Runtime proof for one atomic routing publication. The previous era and the
/// candidate are both exact; visibility and closing are independent facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraPublicationReceipt {
    publication_identity: u64,
    binding_contract_identity: String,
    entry_contract_identity: String,
    previous_era_identity: Option<u64>,
    candidate_era_identity: u64,
    candidate_artifact_occurrence_digest: InstalledArtifactOccurrenceDigest,
    candidate_artifact_instance_compatibility_report_identity: u64,
    candidate_entry_plan_identity: String,
    candidate_entry_plan_admission_receipt_identity: String,
    candidate: ComponentEraCandidate,
    new_era_visible: bool,
    previous_era_closed: bool,
}

impl ComponentEraPublicationReceipt {
    pub fn from_runtime(
        publication_identity: u64,
        ledger: &ComponentEraEntryLedger,
        candidate: &ComponentEraCandidate,
        new_era_visible: bool,
        previous_era_closed: bool,
    ) -> Self {
        Self {
            publication_identity,
            binding_contract_identity: ledger.binding_contract_identity.clone(),
            entry_contract_identity: ledger.entry_contract_identity.clone(),
            previous_era_identity: ledger.current_era,
            candidate_era_identity: candidate.era_identity,
            candidate_artifact_occurrence_digest: candidate.artifact_occurrence_digest,
            candidate_artifact_instance_compatibility_report_identity: candidate
                .artifact_instance_compatibility_report_identity,
            candidate_entry_plan_identity: candidate.entry_plan_identity.clone(),
            candidate_entry_plan_admission_receipt_identity: candidate
                .entry_plan_admission_receipt_identity
                .clone(),
            candidate: candidate.clone(),
            new_era_visible,
            previous_era_closed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraEntryReceipt {
    invocation_identity: u64,
    binding_contract_identity: String,
    entry_contract_identity: String,
    resolved_era_identity: u64,
    entry_plan_identity: String,
    entry_linearized: bool,
}

impl ComponentEraEntryReceipt {
    pub fn from_runtime(
        invocation_identity: u64,
        ledger: &ComponentEraEntryLedger,
        resolved_era_identity: u64,
        entry_plan_identity: String,
        entry_linearized: bool,
    ) -> Self {
        Self {
            invocation_identity,
            binding_contract_identity: ledger.binding_contract_identity.clone(),
            entry_contract_identity: ledger.entry_contract_identity.clone(),
            resolved_era_identity,
            entry_plan_identity,
            entry_linearized,
        }
    }
}

#[derive(Debug)]
pub struct ActiveComponentEraEntry {
    invocation_identity: u64,
    binding_contract_identity: String,
    era_identity: u64,
    entry_plan_identity: String,
}

impl ActiveComponentEraEntry {
    pub const fn invocation_identity(&self) -> u64 {
        self.invocation_identity
    }

    pub const fn era_identity(&self) -> u64 {
        self.era_identity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraLeaveReceipt {
    invocation_identity: u64,
    binding_contract_identity: String,
    era_identity: u64,
    entry_plan_identity: String,
    leave_completed: bool,
}

impl ComponentEraLeaveReceipt {
    pub fn from_runtime(entry: &ActiveComponentEraEntry, leave_completed: bool) -> Self {
        Self {
            invocation_identity: entry.invocation_identity,
            binding_contract_identity: entry.binding_contract_identity.clone(),
            era_identity: entry.era_identity,
            entry_plan_identity: entry.entry_plan_identity.clone(),
            leave_completed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraQuiescenceReceipt {
    era_identity: u64,
    binding_contract_identity: String,
    residual_lifetime_cohort_holds: usize,
    all_dispositions_complete: bool,
}

impl ComponentEraQuiescenceReceipt {
    pub fn from_runtime(
        ledger: &ComponentEraEntryLedger,
        era_identity: u64,
        residual_lifetime_cohort_holds: usize,
        all_dispositions_complete: bool,
    ) -> Self {
        Self {
            era_identity,
            binding_contract_identity: ledger.binding_contract_identity.clone(),
            residual_lifetime_cohort_holds,
            all_dispositions_complete,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentEraRetirementReceipt {
    retirement_identity: u64,
    era_identity: u64,
    binding_contract_identity: String,
    lifetime_cohort_released: bool,
}

impl ComponentEraRetirementReceipt {
    pub fn from_runtime(
        retirement_identity: u64,
        ledger: &ComponentEraEntryLedger,
        era_identity: u64,
        lifetime_cohort_released: bool,
    ) -> Self {
        Self {
            retirement_identity,
            era_identity,
            binding_contract_identity: ledger.binding_contract_identity.clone(),
            lifetime_cohort_released,
        }
    }

    pub const fn era_identity(&self) -> u64 {
        self.era_identity
    }
}

/// Authoritative lifecycle state for one exact component binding.
///
/// This ledger is intentionally non-clonable: duplicating its active lease
/// sets would create two authorities that disagree about whether an era may
/// retire.
#[derive(Debug, PartialEq, Eq)]
pub struct ComponentEraEntryLedger {
    identity: ComponentEraLedgerId,
    binding_contract_identity: String,
    entry_contract_identity: String,
    maximum_live_eras: usize,
    current_era: Option<u64>,
    eras: Vec<ComponentEraRecord>,
    consumed_publications: Vec<u64>,
    consumed_invocations: Vec<u64>,
    active_invocations: Vec<u64>,
    consumed_retirements: Vec<u64>,
    issued_program_local_root_epoch_leases: BTreeSet<ProgramLocalRootEpochLeaseId>,
    executable_tcbs: CoexistingExecutableTcbSet,
}

impl ComponentEraEntryLedger {
    pub fn new(
        identity: ComponentEraLedgerId,
        binding_contract_identity: String,
        entry_contract_identity: String,
        maximum_live_eras: usize,
        process_static_tcb: ExecutableTcbProfileAcceptance,
    ) -> Result<Self, String> {
        if binding_contract_identity.trim().is_empty() {
            return Err("component era ledger has no binding-contract identity".into());
        }
        if entry_contract_identity.trim().is_empty() {
            return Err("component era ledger has no entry-contract identity".into());
        }
        if maximum_live_eras == 0 {
            return Err("component era ledger cannot admit zero live eras".into());
        }
        Ok(Self {
            identity,
            binding_contract_identity,
            entry_contract_identity,
            maximum_live_eras,
            current_era: None,
            eras: Vec::new(),
            consumed_publications: Vec::new(),
            consumed_invocations: Vec::new(),
            active_invocations: Vec::new(),
            consumed_retirements: Vec::new(),
            issued_program_local_root_epoch_leases: BTreeSet::new(),
            executable_tcbs: CoexistingExecutableTcbSet::new(process_static_tcb)?,
        })
    }

    pub const fn identity(&self) -> ComponentEraLedgerId {
        self.identity
    }

    pub const fn current_era(&self) -> Option<u64> {
        self.current_era
    }

    pub fn binding_contract_identity(&self) -> &str {
        &self.binding_contract_identity
    }

    pub fn entry_contract_identity(&self) -> &str {
        &self.entry_contract_identity
    }

    pub fn live_eras(&self) -> impl Iterator<Item = (u64, ComponentEraEntryState, usize)> + '_ {
        self.eras.iter().map(|record| {
            (
                record.candidate.era_identity,
                record.state,
                record.active_entries,
            )
        })
    }

    pub fn live_executable_tcb_report(&self) -> CoexistingExecutableTcbReport {
        self.executable_tcbs.live_report()
    }

    pub fn program_local_root_authority_holds(&self, era_identity: u64) -> Option<usize> {
        self.eras
            .iter()
            .find(|record| record.candidate.era_identity == era_identity)
            .map(|record| record.program_local_root_epoch_leases.len())
    }

    pub fn publish(
        &mut self,
        candidate: ComponentEraCandidate,
        receipt: ComponentEraPublicationReceipt,
    ) -> Result<(), Box<EraPublicationError>> {
        let reject = |candidate, receipt, diagnostic: &str| {
            Err(Box::new(EraPublicationError {
                candidate,
                receipt,
                diagnostic: diagnostic.into(),
            }))
        };
        if candidate.era_identity == 0
            || candidate.artifact_instance_compatibility_report_identity == 0
            || candidate.binding_contract_identity.trim().is_empty()
            || candidate.entry_contract_identity.trim().is_empty()
            || candidate.entry_plan_identity.trim().is_empty()
            || candidate
                .entry_plan_admission_receipt_identity
                .trim()
                .is_empty()
        {
            return reject(candidate, receipt, "component era candidate is incomplete");
        }
        if self.eras.len() >= self.maximum_live_eras {
            return reject(
                candidate,
                receipt,
                "component era publication exceeds the live-era retention limit",
            );
        }
        if self
            .eras
            .iter()
            .any(|record| record.candidate.era_identity == candidate.era_identity)
        {
            return reject(candidate, receipt, "component era identity is already live");
        }
        if receipt.publication_identity == 0
            || self
                .consumed_publications
                .contains(&receipt.publication_identity)
        {
            return reject(
                candidate,
                receipt,
                "component era publication receipt is zero or replayed",
            );
        }
        let exact = candidate.binding_contract_identity == self.binding_contract_identity
            && candidate.entry_contract_identity == self.entry_contract_identity
            && receipt.binding_contract_identity == self.binding_contract_identity
            && receipt.entry_contract_identity == self.entry_contract_identity
            && receipt.previous_era_identity == self.current_era
            && receipt.candidate_era_identity == candidate.era_identity
            && receipt.candidate_artifact_occurrence_digest == candidate.artifact_occurrence_digest
            && receipt.candidate_artifact_instance_compatibility_report_identity
                == candidate.artifact_instance_compatibility_report_identity
            && receipt.candidate_entry_plan_identity == candidate.entry_plan_identity
            && receipt.candidate_entry_plan_admission_receipt_identity
                == candidate.entry_plan_admission_receipt_identity
            && receipt.candidate == candidate;
        if !exact || !receipt.new_era_visible {
            return reject(
                candidate,
                receipt,
                "component era publication does not bind and expose the exact candidate",
            );
        }
        if self.current_era.is_some() && !receipt.previous_era_closed {
            return reject(
                candidate,
                receipt,
                "component era publication does not close the previous era to future entry",
            );
        }
        if let Err(diagnostic) = self.executable_tcbs.admit_era(
            candidate.era_identity,
            candidate.executable_tcb_acceptance.clone(),
        ) {
            return reject(candidate, receipt, diagnostic.as_str());
        }
        if let Some(previous) = self.current_era {
            self.eras
                .iter_mut()
                .find(|record| record.candidate.era_identity == previous)
                .expect("current era remains live")
                .state = ComponentEraEntryState::Closing;
        }
        self.current_era = Some(candidate.era_identity);
        self.eras.push(ComponentEraRecord {
            candidate,
            state: ComponentEraEntryState::Open,
            active_entries: 0,
            program_local_root_epoch_leases: BTreeSet::new(),
        });
        self.eras
            .sort_by_key(|record| record.candidate.era_identity);
        self.consumed_publications
            .push(receipt.publication_identity);
        Ok(())
    }

    /// Acquire one non-duplicable lifecycle hold for program-local authority
    /// rooted in the exact current published entry contract.
    ///
    /// A closing/stale era, mismatched contract, or reused lease identity
    /// rejects without changing the ledger.
    pub fn acquire_program_local_root_epoch_lease(
        &mut self,
        identity: ProgramLocalRootEpochLeaseId,
        era_identity: u64,
        entry_contract_identity: &str,
    ) -> Result<ProgramLocalRootEpochLease, ProgramLocalRootEpochLeaseAcquisitionError> {
        let reject = |diagnostic: &str| ProgramLocalRootEpochLeaseAcquisitionError {
            identity,
            era_identity,
            entry_contract_identity: entry_contract_identity.to_owned(),
            diagnostic: diagnostic.into(),
        };
        if self
            .issued_program_local_root_epoch_leases
            .contains(&identity)
        {
            return Err(reject(
                "program-local root epoch-lease identity is already issued or consumed",
            ));
        }
        if self.current_era != Some(era_identity) {
            return Err(reject(
                "program-local root epoch lease requires the exact current component era",
            ));
        }
        let Some(record) = self
            .eras
            .iter_mut()
            .find(|record| record.candidate.era_identity == era_identity)
        else {
            return Err(reject(
                "program-local root epoch lease names an unpublished component era",
            ));
        };
        let exact_contract = !entry_contract_identity.is_empty()
            && entry_contract_identity == self.entry_contract_identity
            && entry_contract_identity == record.candidate.entry_contract_identity;
        if record.state != ComponentEraEntryState::Open || !exact_contract {
            return Err(reject(
                "program-local root epoch lease requires the exact open published entry contract",
            ));
        }

        let inserted = record.program_local_root_epoch_leases.insert(identity);
        debug_assert!(inserted, "globally fresh lease is fresh in its era");
        self.issued_program_local_root_epoch_leases.insert(identity);
        Ok(ProgramLocalRootEpochLease {
            identity,
            ledger: self.identity,
            binding_contract_identity: self.binding_contract_identity.clone(),
            entry_contract_identity: self.entry_contract_identity.clone(),
            era_identity,
            artifact_occurrence_digest: record.candidate.artifact_occurrence_digest,
            artifact_instance_compatibility_report_identity: record
                .candidate
                .artifact_instance_compatibility_report_identity,
            entry_plan_identity: record.candidate.entry_plan_identity.clone(),
            entry_plan_admission_receipt_identity: record
                .candidate
                .entry_plan_admission_receipt_identity
                .clone(),
            candidate: record.candidate.clone(),
        })
    }

    /// Revalidate an issued lease at an establishment instant.
    ///
    /// Holding a lease pins lifecycle retirement, but does not keep an era
    /// open for new introductions. A lease acquired before a routing switch
    /// therefore cannot be used to establish fresh authority after its era has
    /// begun closing.
    pub fn validate_program_local_root_epoch_lease(
        &self,
        lease: &ProgramLocalRootEpochLease,
    ) -> Result<(), String> {
        let Some(record) = self
            .eras
            .iter()
            .find(|record| record.candidate.era_identity == lease.era_identity)
        else {
            return Err("program-local root epoch lease names an era outside this ledger".into());
        };
        let exact = self.current_era == Some(lease.era_identity)
            && record.state == ComponentEraEntryState::Open
            && lease.ledger == self.identity
            && lease.binding_contract_identity == self.binding_contract_identity
            && lease.entry_contract_identity == self.entry_contract_identity
            && lease.entry_contract_identity == record.candidate.entry_contract_identity
            && lease.artifact_occurrence_digest == record.candidate.artifact_occurrence_digest
            && lease.artifact_instance_compatibility_report_identity
                == record
                    .candidate
                    .artifact_instance_compatibility_report_identity
            && lease.entry_plan_identity == record.candidate.entry_plan_identity
            && lease.entry_plan_admission_receipt_identity
                == record.candidate.entry_plan_admission_receipt_identity
            && lease.candidate == record.candidate
            && self
                .issued_program_local_root_epoch_leases
                .contains(&lease.identity)
            && record
                .program_local_root_epoch_leases
                .contains(&lease.identity);
        if !exact {
            return Err(
                "program-local root epoch lease is not live in the exact current open era".into(),
            );
        }
        Ok(())
    }

    /// Return one exact program-local root lifecycle hold.
    ///
    /// Rejection returns the opaque lease intact so the rightful ledger can
    /// still release it; success consumes it and decrements the era hold once.
    pub fn release_program_local_root_epoch_lease(
        &mut self,
        lease: ProgramLocalRootEpochLease,
    ) -> Result<(), ProgramLocalRootEpochLeaseReleaseError> {
        let Some(record) = self
            .eras
            .iter_mut()
            .find(|record| record.candidate.era_identity == lease.era_identity)
        else {
            return Err(ProgramLocalRootEpochLeaseReleaseError {
                lease,
                diagnostic: "program-local root epoch lease names an era outside this ledger"
                    .into(),
            });
        };
        let exact = lease.ledger == self.identity
            && lease.binding_contract_identity == self.binding_contract_identity
            && lease.entry_contract_identity == self.entry_contract_identity
            && lease.entry_contract_identity == record.candidate.entry_contract_identity
            && lease.artifact_occurrence_digest == record.candidate.artifact_occurrence_digest
            && lease.artifact_instance_compatibility_report_identity
                == record
                    .candidate
                    .artifact_instance_compatibility_report_identity
            && lease.entry_plan_identity == record.candidate.entry_plan_identity
            && lease.entry_plan_admission_receipt_identity
                == record.candidate.entry_plan_admission_receipt_identity
            && lease.candidate == record.candidate
            && self
                .issued_program_local_root_epoch_leases
                .contains(&lease.identity)
            && record
                .program_local_root_epoch_leases
                .contains(&lease.identity);
        if !exact {
            return Err(ProgramLocalRootEpochLeaseReleaseError {
                lease,
                diagnostic:
                    "program-local root epoch lease does not belong to this exact published era"
                        .into(),
            });
        }
        let removed = record
            .program_local_root_epoch_leases
            .remove(&lease.identity);
        debug_assert!(removed, "exact live lease was present");
        Ok(())
    }

    pub fn enter(
        &mut self,
        receipt: ComponentEraEntryReceipt,
    ) -> Result<ActiveComponentEraEntry, EraEntryError> {
        let Some(current) = self.current_era else {
            return Err(EraEntryError {
                receipt,
                diagnostic: "component binding has no published era".into(),
            });
        };
        let Some(record) = self
            .eras
            .iter_mut()
            .find(|record| record.candidate.era_identity == current)
        else {
            return Err(EraEntryError {
                receipt,
                diagnostic: "current component era is absent from its ledger".into(),
            });
        };
        let exact = receipt.invocation_identity != 0
            && !self
                .consumed_invocations
                .contains(&receipt.invocation_identity)
            && receipt.binding_contract_identity == self.binding_contract_identity
            && receipt.entry_contract_identity == self.entry_contract_identity
            && receipt.resolved_era_identity == current
            && receipt.entry_plan_identity == record.candidate.entry_plan_identity
            && receipt.entry_linearized
            && record.state == ComponentEraEntryState::Open;
        if !exact {
            return Err(EraEntryError { receipt, diagnostic: "component entry receipt does not linearize exactly once into the current open era".into() });
        }
        record.active_entries += 1;
        self.consumed_invocations.push(receipt.invocation_identity);
        self.active_invocations.push(receipt.invocation_identity);
        Ok(ActiveComponentEraEntry {
            invocation_identity: receipt.invocation_identity,
            binding_contract_identity: self.binding_contract_identity.clone(),
            era_identity: current,
            entry_plan_identity: receipt.entry_plan_identity,
        })
    }

    pub fn leave(
        &mut self,
        entry: ActiveComponentEraEntry,
        receipt: ComponentEraLeaveReceipt,
    ) -> Result<(), Box<EraLeaveError>> {
        let Some(record) = self
            .eras
            .iter_mut()
            .find(|record| record.candidate.era_identity == entry.era_identity)
        else {
            return Err(Box::new(EraLeaveError {
                entry,
                receipt,
                diagnostic: "entered component era is no longer live".into(),
            }));
        };
        let exact = receipt.invocation_identity == entry.invocation_identity
            && receipt.binding_contract_identity == entry.binding_contract_identity
            && receipt.era_identity == entry.era_identity
            && receipt.entry_plan_identity == entry.entry_plan_identity
            && receipt.leave_completed
            && self.active_invocations.contains(&entry.invocation_identity)
            && record.active_entries > 0;
        if !exact {
            return Err(Box::new(EraLeaveError {
                entry,
                receipt,
                diagnostic: "component leave receipt does not complete the exact active entry"
                    .into(),
            }));
        }
        record.active_entries -= 1;
        self.active_invocations
            .retain(|identity| *identity != entry.invocation_identity);
        Ok(())
    }

    pub fn establish_quiescence(
        &mut self,
        receipt: ComponentEraQuiescenceReceipt,
    ) -> Result<(), EraQuiescenceError> {
        let Some(record) = self
            .eras
            .iter_mut()
            .find(|record| record.candidate.era_identity == receipt.era_identity)
        else {
            return Err(EraQuiescenceError {
                receipt,
                diagnostic: "component era is not live".into(),
            });
        };
        let exact = receipt.binding_contract_identity == self.binding_contract_identity
            && record.state == ComponentEraEntryState::Closing
            && record.active_entries == 0
            && record.program_local_root_epoch_leases.is_empty()
            && receipt.residual_lifetime_cohort_holds == 0
            && receipt.all_dispositions_complete;
        if !exact {
            return Err(EraQuiescenceError { receipt, diagnostic: "component era quiescence requires closing, zero entries, zero program-local root authority holds, and complete cohort disposition".into() });
        }
        record.state = ComponentEraEntryState::Quiescent;
        Ok(())
    }

    pub fn retire(
        &mut self,
        receipt: ComponentEraRetirementReceipt,
    ) -> Result<(), EraRetirementError> {
        let Some(index) = self
            .eras
            .iter()
            .position(|record| record.candidate.era_identity == receipt.era_identity)
        else {
            return Err(EraRetirementError {
                receipt,
                diagnostic: "component era is not live".into(),
            });
        };
        if !self.eras[index].program_local_root_epoch_leases.is_empty() {
            return Err(EraRetirementError {
                receipt,
                diagnostic: "component era retirement rejects while program-local root epoch leases remain live"
                    .into(),
            });
        }
        let exact = receipt.retirement_identity != 0
            && !self
                .consumed_retirements
                .contains(&receipt.retirement_identity)
            && receipt.binding_contract_identity == self.binding_contract_identity
            && receipt.lifetime_cohort_released
            && self.eras[index].state == ComponentEraEntryState::Quiescent
            && self.current_era != Some(receipt.era_identity);
        if !exact {
            return Err(EraRetirementError { receipt, diagnostic: "component era retirement requires a noncurrent quiescent released cohort and fresh receipt".into() });
        }
        self.executable_tcbs
            .retire_era_after_quiescence(receipt.era_identity)
            .map_err(|diagnostic| EraRetirementError {
                receipt: receipt.clone(),
                diagnostic,
            })?;
        self.eras.remove(index);
        self.consumed_retirements.push(receipt.retirement_identity);
        Ok(())
    }
}

macro_rules! recoverable_error {
    ($name:ident, $value:ident, $type:ty) => {
        #[derive(Debug)]
        pub struct $name {
            $value: $type,
            diagnostic: String,
        }
        impl $name {
            pub const fn diagnostic(&self) -> &str {
                self.diagnostic.as_str()
            }
            pub fn into_value(self) -> $type {
                self.$value
            }
        }
    };
}

#[derive(Debug)]
pub struct EraPublicationError {
    candidate: ComponentEraCandidate,
    receipt: ComponentEraPublicationReceipt,
    diagnostic: String,
}
impl EraPublicationError {
    pub const fn diagnostic(&self) -> &str {
        self.diagnostic.as_str()
    }
    pub fn into_parts(self) -> (ComponentEraCandidate, ComponentEraPublicationReceipt) {
        (self.candidate, self.receipt)
    }
}
recoverable_error!(EraEntryError, receipt, ComponentEraEntryReceipt);

#[derive(Debug)]
pub struct EraLeaveError {
    entry: ActiveComponentEraEntry,
    receipt: ComponentEraLeaveReceipt,
    diagnostic: String,
}
impl EraLeaveError {
    pub const fn diagnostic(&self) -> &str {
        self.diagnostic.as_str()
    }
    pub fn into_parts(self) -> (ActiveComponentEraEntry, ComponentEraLeaveReceipt) {
        (self.entry, self.receipt)
    }
}
recoverable_error!(EraQuiescenceError, receipt, ComponentEraQuiescenceReceipt);
recoverable_error!(EraRetirementError, receipt, ComponentEraRetirementReceipt);

#[derive(Debug)]
pub struct ProgramLocalRootEpochLeaseAcquisitionError {
    identity: ProgramLocalRootEpochLeaseId,
    era_identity: u64,
    entry_contract_identity: String,
    diagnostic: String,
}

impl ProgramLocalRootEpochLeaseAcquisitionError {
    pub const fn identity(&self) -> ProgramLocalRootEpochLeaseId {
        self.identity
    }

    pub const fn era_identity(&self) -> u64 {
        self.era_identity
    }

    pub fn entry_contract_identity(&self) -> &str {
        &self.entry_contract_identity
    }

    pub const fn diagnostic(&self) -> &str {
        self.diagnostic.as_str()
    }
}

#[derive(Debug)]
pub struct ProgramLocalRootEpochLeaseReleaseError {
    lease: ProgramLocalRootEpochLease,
    diagnostic: String,
}

impl ProgramLocalRootEpochLeaseReleaseError {
    pub const fn diagnostic(&self) -> &str {
        self.diagnostic.as_str()
    }

    pub fn into_lease(self) -> ProgramLocalRootEpochLease {
        self.lease
    }
}

#[cfg(test)]
mod custody;
#[cfg(test)]
mod tests;
