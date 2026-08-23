use std::collections::BTreeSet;

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
    /// Exact installed artifact occurrence governed by this lifecycle era.
    /// This remains distinct from the replaceable era identity.
    pub artifact_instance_identity: u64,
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
    artifact_instance_identity: u64,
    entry_plan_identity: String,
    entry_plan_admission_receipt_identity: String,
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

    pub const fn artifact_instance_identity(&self) -> u64 {
        self.artifact_instance_identity
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
    candidate_artifact_instance_identity: u64,
    candidate_entry_plan_identity: String,
    candidate_entry_plan_admission_receipt_identity: String,
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
            candidate_artifact_instance_identity: candidate.artifact_instance_identity,
            candidate_entry_plan_identity: candidate.entry_plan_identity.clone(),
            candidate_entry_plan_admission_receipt_identity: candidate
                .entry_plan_admission_receipt_identity
                .clone(),
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
            || candidate.artifact_instance_identity == 0
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
            && receipt.candidate_artifact_instance_identity == candidate.artifact_instance_identity
            && receipt.candidate_entry_plan_identity == candidate.entry_plan_identity
            && receipt.candidate_entry_plan_admission_receipt_identity
                == candidate.entry_plan_admission_receipt_identity;
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
            artifact_instance_identity: record.candidate.artifact_instance_identity,
            entry_plan_identity: record.candidate.entry_plan_identity.clone(),
            entry_plan_admission_receipt_identity: record
                .candidate
                .entry_plan_admission_receipt_identity
                .clone(),
        })
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
            && lease.artifact_instance_identity == record.candidate.artifact_instance_identity
            && lease.entry_plan_identity == record.candidate.entry_plan_identity
            && lease.entry_plan_admission_receipt_identity
                == record.candidate.entry_plan_admission_receipt_identity
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
mod tests {
    use super::*;
    use crate::{
        ExecutableTcbManifest, ExecutableTcbProfile, ExecutionScope, IncompleteScopePolicy,
        ScopeCompleteness, evaluate_executable_tcb_profile,
    };

    fn acceptance(name: &str, closure: u64) -> ExecutableTcbProfileAcceptance {
        evaluate_executable_tcb_profile(
            &ExecutableTcbManifest {
                known_entries: Vec::new(),
                completeness: ScopeCompleteness::Complete {
                    scope: ExecutionScope::CallerAddressSpace,
                    selected_provider_closure_identity: closure,
                    opaque_closure_evidence: Vec::new(),
                    runtime_closure_evidence: Vec::new(),
                },
            },
            &ExecutableTcbProfile {
                name: name.into(),
                scope: ExecutionScope::CallerAddressSpace,
                allow_static_current_artifact_checked_bodies: true,
                exact_allowances: Vec::new(),
                incomplete_scope: IncompleteScopePolicy::Reject,
            },
        )
        .expect("profile acceptance")
    }

    fn ledger_with_identity(identity: u64, maximum: usize) -> ComponentEraEntryLedger {
        ComponentEraEntryLedger::new(
            ComponentEraLedgerId::from_normalized_identity(identity).expect("ledger identity"),
            "CodecBinding/v1".into(),
            "CodecEntry/v1".into(),
            maximum,
            acceptance("platform", 1),
        )
        .expect("ledger")
    }

    fn ledger(maximum: usize) -> ComponentEraEntryLedger {
        ledger_with_identity(1, maximum)
    }

    fn lease_id(identity: u64) -> ProgramLocalRootEpochLeaseId {
        ProgramLocalRootEpochLeaseId::from_normalized_identity(identity)
            .expect("epoch lease identity")
    }

    fn candidate(era: u64) -> ComponentEraCandidate {
        ComponentEraCandidate {
            era_identity: era,
            artifact_instance_identity: era + 1_000,
            binding_contract_identity: "CodecBinding/v1".into(),
            entry_contract_identity: "CodecEntry/v1".into(),
            entry_plan_identity: format!("entry-plan:{era}"),
            entry_plan_admission_receipt_identity: format!("receipt:entry-plan:{era}"),
            executable_tcb_acceptance: acceptance(format!("era-{era}").as_str(), era),
        }
    }

    fn publish(ledger: &mut ComponentEraEntryLedger, era: u64, receipt: u64) {
        let candidate = candidate(era);
        let publication = ComponentEraPublicationReceipt::from_runtime(
            receipt,
            ledger,
            &candidate,
            true,
            ledger.current_era().is_some(),
        );
        ledger.publish(candidate, publication).expect("publication");
    }

    #[test]
    fn routing_switch_closes_old_entry_but_retains_its_active_invocation() {
        let mut ledger = ledger(2);
        publish(&mut ledger, 10, 100);
        let entry_receipt =
            ComponentEraEntryReceipt::from_runtime(500, &ledger, 10, "entry-plan:10".into(), true);
        let old_entry = ledger.enter(entry_receipt).expect("v1 entry");
        publish(&mut ledger, 20, 101);
        assert_eq!(ledger.current_era(), Some(20));
        assert_eq!(
            ledger.live_eras().collect::<Vec<_>>(),
            vec![
                (10, ComponentEraEntryState::Closing, 1),
                (20, ComponentEraEntryState::Open, 0),
            ]
        );
        let stale =
            ComponentEraEntryReceipt::from_runtime(501, &ledger, 10, "entry-plan:10".into(), true);
        assert!(ledger.enter(stale).is_err());

        let leave = ComponentEraLeaveReceipt::from_runtime(&old_entry, true);
        ledger
            .leave(old_entry, leave)
            .expect("old entry leaves its own era");
        let replayed =
            ComponentEraEntryReceipt::from_runtime(500, &ledger, 20, "entry-plan:20".into(), true);
        assert!(ledger.enter(replayed).is_err());
        let blocked = ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 1, true);
        assert!(ledger.establish_quiescence(blocked).is_err());
        let quiescent = ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 0, true);
        ledger
            .establish_quiescence(quiescent)
            .expect("all holds disposed");
        let retirement = ComponentEraRetirementReceipt::from_runtime(700, &ledger, 10, true);
        ledger
            .retire(retirement)
            .expect("quiescent old era retires");
        assert_eq!(
            ledger.live_eras().collect::<Vec<_>>(),
            vec![(20, ComponentEraEntryState::Open, 0)]
        );
        assert_eq!(
            ledger
                .live_executable_tcb_report()
                .completeness()
                .sources()
                .len(),
            2
        );
    }

    #[test]
    fn publication_enforces_retention_limit_and_receipt_identity() {
        let mut ledger = ledger(1);
        publish(&mut ledger, 10, 100);
        let next = candidate(20);
        let receipt = ComponentEraPublicationReceipt::from_runtime(101, &ledger, &next, true, true);
        let error = ledger.publish(next, receipt).expect_err("live-era limit");
        assert!(error.diagnostic().contains("retention limit"));
        let (next, _) = (*error).into_parts();
        assert_eq!(next.era_identity, 20);
    }

    #[test]
    fn leave_and_retirement_receipts_cannot_drift_or_replay() {
        let mut ledger = ledger(2);
        publish(&mut ledger, 10, 100);
        let entry = ledger
            .enter(ComponentEraEntryReceipt::from_runtime(
                500,
                &ledger,
                10,
                "entry-plan:10".into(),
                true,
            ))
            .expect("entry");
        let mut drifted = ComponentEraLeaveReceipt::from_runtime(&entry, true);
        drifted.era_identity = 11;
        let error = ledger.leave(entry, drifted).expect_err("leave drift");
        let (entry, _) = (*error).into_parts();
        let exact_leave = ComponentEraLeaveReceipt::from_runtime(&entry, true);
        ledger.leave(entry, exact_leave).expect("exact leave");
        publish(&mut ledger, 20, 101);
        ledger
            .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
                &ledger, 10, 0, true,
            ))
            .expect("quiescence");
        ledger
            .retire(ComponentEraRetirementReceipt::from_runtime(
                700, &ledger, 10, true,
            ))
            .expect("first retirement");
        let replay = ComponentEraRetirementReceipt::from_runtime(700, &ledger, 20, true);
        assert!(ledger.retire(replay).is_err());
    }

    #[test]
    fn program_local_root_epoch_lease_requires_current_open_exact_contract_and_artifact() {
        let mut ledger = ledger(2);
        let mut incomplete = candidate(10);
        incomplete.artifact_instance_identity = 0;
        let receipt =
            ComponentEraPublicationReceipt::from_runtime(99, &ledger, &incomplete, true, false);
        assert!(ledger.publish(incomplete, receipt).is_err());

        let candidate = candidate(10);
        let mut drifted =
            ComponentEraPublicationReceipt::from_runtime(100, &ledger, &candidate, true, false);
        drifted.candidate_artifact_instance_identity += 1;
        let error = ledger
            .publish(candidate, drifted)
            .expect_err("artifact-instance drift");
        let (candidate, _) = (*error).into_parts();
        let exact =
            ComponentEraPublicationReceipt::from_runtime(100, &ledger, &candidate, true, false);
        ledger.publish(candidate, exact).expect("exact publication");

        let wrong_contract = ledger
            .acquire_program_local_root_epoch_lease(lease_id(900), 10, "OtherEntry/v1")
            .expect_err("entry contract mismatch");
        assert!(wrong_contract.diagnostic().contains("exact open"));
        assert_eq!(ledger.program_local_root_authority_holds(10), Some(0));
        let stale = ledger
            .acquire_program_local_root_epoch_lease(lease_id(901), 9, "CodecEntry/v1")
            .expect_err("stale era");
        assert!(stale.diagnostic().contains("exact current"));

        let lease = ledger
            .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
            .expect("exact current era lease");
        assert_eq!(lease.era_identity(), 10);
        assert_eq!(lease.artifact_instance_identity(), 1_010);
        assert_eq!(ledger.program_local_root_authority_holds(10), Some(1));

        publish(&mut ledger, 20, 101);
        let closing = ledger
            .acquire_program_local_root_epoch_lease(lease_id(902), 10, "CodecEntry/v1")
            .expect_err("closing era");
        assert!(closing.diagnostic().contains("exact current"));
        ledger
            .release_program_local_root_epoch_lease(lease)
            .expect("closing era retains the exact releasable hold");
        let current = ledger
            .acquire_program_local_root_epoch_lease(lease_id(902), 20, "CodecEntry/v1")
            .expect("new current era");
        ledger
            .release_program_local_root_epoch_lease(current)
            .expect("release current era hold");
    }

    #[test]
    fn live_program_local_root_epoch_lease_blocks_quiescence_and_retirement() {
        let mut ledger = ledger(2);
        publish(&mut ledger, 10, 100);
        let lease = ledger
            .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
            .expect("lease");
        publish(&mut ledger, 20, 101);

        let retirement = ComponentEraRetirementReceipt::from_runtime(700, &ledger, 10, true);
        let blocked = ledger.retire(retirement).expect_err("live epoch lease");
        assert!(blocked.diagnostic().contains("epoch leases remain live"));
        let quiescence = ComponentEraQuiescenceReceipt::from_runtime(&ledger, 10, 0, true);
        let blocked = ledger
            .establish_quiescence(quiescence)
            .expect_err("authority hold");
        assert!(blocked.diagnostic().contains("authority holds"));

        ledger
            .release_program_local_root_epoch_lease(lease)
            .expect("exact release");
        assert_eq!(ledger.program_local_root_authority_holds(10), Some(0));
        ledger
            .establish_quiescence(ComponentEraQuiescenceReceipt::from_runtime(
                &ledger, 10, 0, true,
            ))
            .expect("quiescence after release");
        ledger
            .retire(ComponentEraRetirementReceipt::from_runtime(
                700, &ledger, 10, true,
            ))
            .expect("retirement after release");
    }

    #[test]
    fn epoch_lease_release_rejects_ledger_substitution_and_identity_replay() {
        let mut first = ledger_with_identity(1, 1);
        let mut second = ledger_with_identity(2, 1);
        publish(&mut first, 10, 100);
        publish(&mut second, 10, 200);

        let lease = first
            .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
            .expect("first ledger lease");
        let error = second
            .release_program_local_root_epoch_lease(lease)
            .expect_err("cross-ledger substitution");
        assert!(error.diagnostic().contains("exact published era"));
        assert_eq!(first.program_local_root_authority_holds(10), Some(1));
        assert_eq!(second.program_local_root_authority_holds(10), Some(0));
        let lease = error.into_lease();
        first
            .release_program_local_root_epoch_lease(lease)
            .expect("rightful ledger release");

        let replay = first
            .acquire_program_local_root_epoch_lease(lease_id(900), 10, "CodecEntry/v1")
            .expect_err("lease identity replay");
        assert!(replay.diagnostic().contains("already issued or consumed"));
    }
}
