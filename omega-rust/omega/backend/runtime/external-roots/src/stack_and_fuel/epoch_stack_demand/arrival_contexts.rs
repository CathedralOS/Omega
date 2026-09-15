//! Admitted opaque arrival contexts and validated x86_64 gate profile
//! rosters.

use crate::{
    ExternalRootDiagnostic, ExternalRootId, ProviderStackSummary, RootProviderId,
    StackValidationReceiptId, X86_64GateProfileValidationReceiptId,
};
use calling_conventions::{
    ArrivalContextId, EntryControl, MachineRegime, ValidatedBoundaryEntryPlan,
    ValidatedEntryStackRealization, X86_64InstalledGateTssRealization,
};
use executable_installation::{ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId};
use layout_plans::EntryStubId;

/// Structurally closed input to epoch composition.
///
/// This is not root-admission evidence. `body_wcsu_bytes` and the realization
/// still need provenance before the resulting demand can enter a resource
/// ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochStackCompositionInput {
    pub root: ExternalRootId,
    pub provider: RootProviderId,
    pub realization: ValidatedEntryStackRealization,
    pub body_wcsu_bytes: u64,
    pub body_wcsu_alignment: u64,
}

/// Provider-admitted claim that names the complete admissible arrival-context
/// set for one exact opaque entry realization.
///
/// A receipt identity alone cannot establish completeness: without the bound
/// set, the provider could omit a context whose stack transition is more
/// demanding. Private identity fields prevent replay for another root,
/// artifact, entry, target, or public boundary contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedOpaqueArrivalContextSet {
    root: ExternalRootId,
    provider: RootProviderId,
    architecture: target::Architecture,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
    boundary_contract_report_fingerprint: u64,
    boundary_contract_commitment: [u8; 32],
    pub(crate) contexts: Vec<ArrivalContextId>,
    validation_receipt: StackValidationReceiptId,
}

impl AdmittedOpaqueArrivalContextSet {
    pub fn contexts(&self) -> &[ArrivalContextId] {
        &self.contexts
    }

    pub const fn validation_receipt(&self) -> StackValidationReceiptId {
        self.validation_receipt
    }

    #[cfg(test)]
    pub(crate) fn with_boundary_contract_commitment_for_test(
        mut self,
        commitment: [u8; 32],
    ) -> Self {
        self.boundary_contract_commitment = commitment;
        self
    }

    fn matches_installed_code_entry(
        &self,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> bool {
        self.architecture == installed_code.architecture()
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
            && self.entry == entry
    }

    pub(crate) fn matches_installed_entry(
        &self,
        summary: &ProviderStackSummary,
        boundary: &ValidatedBoundaryEntryPlan,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> bool {
        self.root == summary.root
            && self.provider == summary.provider
            && self.matches_installed_code_entry(installed_code, entry)
            && self.boundary_contract_report_fingerprint == boundary.contract_report_fingerprint()
            && self.boundary_contract_commitment != [0; 32]
            && self.boundary_contract_commitment == boundary.contract_commitment_digest()
    }
}

/// Opaque table-owner validation of the complete x86-64 arrival-context
/// roster for one exact installed gate/profile occurrence.
///
/// The public gate/TSS detail carrier is deliberately not a completeness
/// proof. Production must compare its complete arrival rows with this sealed
/// roster before deriving target facts, so removing, padding, or changing one
/// context cannot narrow architectural stack demand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedX86_64InstalledGateProfileRoster {
    architecture: target::Architecture,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
    boundary_contract_report_fingerprint: u64,
    boundary_contract_commitment: [u8; 32],
    pub(crate) realization: X86_64InstalledGateTssRealization,
    pub(crate) validation_receipt: X86_64GateProfileValidationReceiptId,
}

impl ValidatedX86_64InstalledGateProfileRoster {
    pub const fn realization(&self) -> &X86_64InstalledGateTssRealization {
        &self.realization
    }

    pub const fn validation_receipt(&self) -> X86_64GateProfileValidationReceiptId {
        self.validation_receipt
    }

    pub(crate) fn matches_installed_entry(
        &self,
        boundary: &ValidatedBoundaryEntryPlan,
        installed_code: &InstalledCode,
        entry: EntryStubId,
    ) -> bool {
        self.architecture == installed_code.architecture()
            && self.installed_code == installed_code.identity()
            && self.installed_code_context == installed_code.receipt_context()
            && self.artifact == installed_code.artifact()
            && self.entry == entry
            && self.boundary_contract_report_fingerprint == boundary.contract_report_fingerprint()
            && self.boundary_contract_commitment != [0; 32]
            && self.boundary_contract_commitment == boundary.contract_commitment_digest()
    }
}

/// Retain the exact complete context roster established by the x86 table and
/// target-profile validator. The returned carrier is opaque; later gate/TSS
/// details must equal this complete set rather than asserting completeness for
/// themselves.
pub fn validate_x86_64_installed_gate_profile_roster(
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    mut realization: X86_64InstalledGateTssRealization,
    validation_receipt: X86_64GateProfileValidationReceiptId,
) -> Result<ValidatedX86_64InstalledGateProfileRoster, ExternalRootDiagnostic> {
    installed_code.selected_entry_target(entry).map_err(|_| {
        ExternalRootDiagnostic(
            "x86-64 installed gate/profile validation names no exact installed entry".into(),
        )
    })?;
    if installed_code.architecture() != target::Architecture::X86_64
        || boundary.plan().state.initial_regime != MachineRegime::X86Long64
        || boundary.plan().call.entry_control != EntryControl::InterruptReturn
    {
        return Err(ExternalRootDiagnostic(
            "x86-64 installed gate/profile validation requires an x86 long-mode InterruptReturn entry"
                .into(),
        ));
    }
    canonicalize_x86_64_installed_gate_tss_realization(&mut realization);
    if realization.arrivals.is_empty() {
        return Err(ExternalRootDiagnostic(
            "x86-64 installed gate/profile validation contains no arrival context".into(),
        ));
    }
    if realization
        .arrivals
        .windows(2)
        .any(|pair| pair[0].context == pair[1].context)
    {
        return Err(ExternalRootDiagnostic(
            "x86-64 installed gate/profile validation repeats an arrival context".into(),
        ));
    }
    Ok(ValidatedX86_64InstalledGateProfileRoster {
        architecture: installed_code.architecture(),
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
        boundary_contract_report_fingerprint: boundary.contract_report_fingerprint(),
        boundary_contract_commitment: boundary.contract_commitment_digest(),
        realization,
        validation_receipt,
    })
}

pub(crate) fn canonicalize_x86_64_installed_gate_tss_realization(
    realization: &mut X86_64InstalledGateTssRealization,
) {
    realization
        .tss
        .privilege_stacks
        .sort_by_key(|stack| stack.entry_privilege);
    realization
        .tss
        .interrupt_stacks
        .sort_by_key(|stack| stack.slot);
    realization.arrivals.sort_by_key(|arrival| arrival.context);
}

/// Admit one opaque provider's complete arrival-context claim for an exact
/// installed entry. The set is canonicalized here and must later equal the
/// contexts carried by the provider's epoch realization.
pub fn admit_opaque_arrival_context_set(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    mut contexts: Vec<ArrivalContextId>,
    validation_receipt: StackValidationReceiptId,
) -> Result<AdmittedOpaqueArrivalContextSet, ExternalRootDiagnostic> {
    installed_code.selected_entry_target(entry).map_err(|_| {
        ExternalRootDiagnostic(
            "opaque arrival-context admission names no exact installed entry".into(),
        )
    })?;
    if boundary.plan().state.initial_regime.architecture() != installed_code.architecture() {
        return Err(ExternalRootDiagnostic(
            "opaque arrival-context admission target differs from the installed artifact architecture"
                .into(),
        ));
    }
    if summary.stack != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "opaque arrival-context admission drifted from the boundary stack disposition".into(),
        ));
    }
    contexts.sort();
    if contexts.is_empty() {
        return Err(ExternalRootDiagnostic(
            "opaque arrival-context admission contains no context".into(),
        ));
    }
    if contexts.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ExternalRootDiagnostic(
            "opaque arrival-context admission repeats a context identity".into(),
        ));
    }
    Ok(AdmittedOpaqueArrivalContextSet {
        root: summary.root,
        provider: summary.provider,
        architecture: installed_code.architecture(),
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
        boundary_contract_report_fingerprint: boundary.contract_report_fingerprint(),
        boundary_contract_commitment: boundary.contract_commitment_digest(),
        contexts,
        validation_receipt,
    })
}
