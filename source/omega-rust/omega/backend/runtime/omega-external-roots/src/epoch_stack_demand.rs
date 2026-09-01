//! Pure composition of validated external-entry stack epochs.
//!
//! This module computes context-, phase-, and domain-aware stack demand. It
//! deliberately does not turn structurally validated plan data into admission
//! evidence: callers must separately bind each realization to sealed target
//! facts, emitted adapter bytes, or an admitted opaque-provider receipt.

use std::collections::{BTreeMap, BTreeSet};

use omega_calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, ArrivalContextStackDomain, CallSignature,
    CallingPolicy, EntryControl, EntryStack, EntryStackEpoch, EntryStackRealization,
    EntryStackStage, InstalledEntryFactIdentity, MachineRegime, Preemption, StackDomainRef,
    StackOccupancy, ValidatedBoundaryEntryPlan, ValidatedEntryStackDomainClosure,
    ValidatedEntryStackRealization, ValidatedX86_64InstalledHardwareEntryFacts, ValueShape,
    X86_64HardwareStackSelection, X86_64InstalledArrivalContext, X86_64InstalledGateTssRealization,
    X86_64InstalledHardwareEntryFacts, X86_64TargetDerivedHardwareArrival,
    X86_64TargetProfileIdentity, derive_x86_64_hardware_arrival,
    evaluate_ordinary_boundary_entry_plan, validate_entry_stack_domain_closure,
    validate_entry_stack_realization, validate_x86_64_installed_hardware_entry_facts,
};
use omega_executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use omega_isa_x86_64::{
    X86_64SemanticUnitWrapperEncodingRequest, X86_64SemanticUnitWrapperResolution,
    validate_x86_64_resolved_semantic_unit_wrapper, validate_x86_64_semantic_unit_wrapper_template,
};
use psi_layout_plans::EntryStubId;

use super::stack_demand::fingerprint_stack_local_evidence;
use super::{
    ExternalRootDiagnostic, ExternalRootId, Fnv1a, ProviderStackSummary, RootProviderId,
    StackDomain, StackLocalEvidence, StackNestingRelation, StackValidationReceiptId,
    X86_64GateProfileValidationReceiptId,
};

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
    architecture: omega_target::Architecture,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
    boundary_contract_report_fingerprint: u64,
    boundary_contract_commitment: [u8; 32],
    contexts: Vec<ArrivalContextId>,
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

    fn matches_installed_entry(
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
    architecture: omega_target::Architecture,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
    boundary_contract_report_fingerprint: u64,
    boundary_contract_commitment: [u8; 32],
    realization: X86_64InstalledGateTssRealization,
    validation_receipt: X86_64GateProfileValidationReceiptId,
}

impl ValidatedX86_64InstalledGateProfileRoster {
    pub const fn realization(&self) -> &X86_64InstalledGateTssRealization {
        &self.realization
    }

    pub const fn validation_receipt(&self) -> X86_64GateProfileValidationReceiptId {
        self.validation_receipt
    }

    fn matches_installed_entry(
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
    if installed_code.architecture() != omega_target::Architecture::X86_64
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

fn canonicalize_x86_64_installed_gate_tss_realization(
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

/// Provenance of the architectural arrival portion of an entry realization.
/// Arrival and adapter origins are independent: a target-derived hardware
/// frame may be followed by no adapter, a generated adapter, or an opaque one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrivalStackRealizationOrigin {
    NoHardwareArrival,
    X86_64TargetRule,
    OpaqueProvider,
}

/// Provenance of stack epochs introduced after architectural arrival and
/// before the Terminal body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterStackRealizationOrigin {
    None,
    GeneratedProgramStorageSemanticWrapper,
    OpaqueProvider,
}

/// Untrusted emitted-operation rows for the one receiver-free x86 semantic
/// ProgramStorage wrapper. Admission independently replays both the canonical
/// template and its resolved private-continuation call before retaining them.
#[derive(Debug, Clone, Copy)]
pub struct X86_64GeneratedProgramStorageAdapterEmission<'a> {
    pub request: X86_64SemanticUnitWrapperEncodingRequest,
    pub template_bytes: &'a [u8],
    pub resolved_bytes: &'a [u8],
    pub wrapper_section_offset: u64,
    pub continuation_section_offset: u64,
}

/// Exact replayed generated-adapter evidence retained behind root admission.
/// The compact fingerprint is report-only; the canonical request, resolved
/// bytes, and call coordinates remain the authority-bearing facts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedProgramStorageAdapterStackEvidence {
    request: X86_64SemanticUnitWrapperEncodingRequest,
    resolved_bytes: Vec<u8>,
    resolution: X86_64SemanticUnitWrapperResolution,
    non_authoritative_report_fingerprint: u64,
}

impl GeneratedProgramStorageAdapterStackEvidence {
    pub const fn request(&self) -> X86_64SemanticUnitWrapperEncodingRequest {
        self.request
    }

    pub fn resolved_bytes(&self) -> &[u8] {
        &self.resolved_bytes
    }

    pub const fn resolution(&self) -> X86_64SemanticUnitWrapperResolution {
        self.resolution
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }
}

/// Complete auditable provenance for one bound entry realization.
///
/// The body evidence is retained separately by `BoundEpochStackCompositionInput`.
/// This carrier records where architectural arrival and adapter epochs came
/// from, avoiding the old whole-plan origin that could not represent a sealed
/// target frame followed by independently generated or admitted adapter work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryStackRealizationEvidence {
    root: ExternalRootId,
    provider: RootProviderId,
    architecture: omega_target::Architecture,
    installed_code: InstalledCodeId,
    installed_code_context: InstalledCodeContext,
    artifact: ArtifactId,
    entry: EntryStubId,
    boundary_contract_report_fingerprint: u64,
    boundary_contract_commitment: [u8; 32],
    body_domains: ValidatedEntryStackDomainClosure,
    realization: ValidatedEntryStackRealization,
    arrival_origin: ArrivalStackRealizationOrigin,
    adapter_origin: AdapterStackRealizationOrigin,
    target_rule_report_fingerprint: Option<u64>,
    target_installation_validation_receipt: Option<X86_64GateProfileValidationReceiptId>,
    generated_adapter: Option<GeneratedProgramStorageAdapterStackEvidence>,
    validation_receipt: Option<StackValidationReceiptId>,
}

impl EntryStackRealizationEvidence {
    pub const fn arrival_origin(&self) -> ArrivalStackRealizationOrigin {
        self.arrival_origin
    }

    pub const fn adapter_origin(&self) -> AdapterStackRealizationOrigin {
        self.adapter_origin
    }

    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn provider(&self) -> RootProviderId {
        self.provider
    }

    pub const fn architecture(&self) -> omega_target::Architecture {
        self.architecture
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    pub const fn entry(&self) -> EntryStubId {
        self.entry
    }

    pub const fn boundary_contract_report_fingerprint(&self) -> u64 {
        self.boundary_contract_report_fingerprint
    }

    pub const fn boundary_contract_commitment(&self) -> [u8; 32] {
        self.boundary_contract_commitment
    }

    pub fn body_domains(&self) -> Vec<(ArrivalContextId, StackDomainRef)> {
        self.body_domains
            .contexts()
            .iter()
            .map(|context| (context.context, context.domain))
            .collect()
    }

    pub const fn realization(&self) -> &ValidatedEntryStackRealization {
        &self.realization
    }

    pub const fn target_rule_report_fingerprint(&self) -> Option<u64> {
        self.target_rule_report_fingerprint
    }

    pub const fn target_installation_validation_receipt(
        &self,
    ) -> Option<X86_64GateProfileValidationReceiptId> {
        self.target_installation_validation_receipt
    }

    pub const fn generated_adapter(&self) -> Option<&GeneratedProgramStorageAdapterStackEvidence> {
        self.generated_adapter.as_ref()
    }

    pub const fn validation_receipt(&self) -> Option<StackValidationReceiptId> {
        self.validation_receipt
    }

    pub(super) fn matches_installed_code_entry(
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
}

/// Epoch input whose body demand and complete entry realization are both bound
/// to the same exact installed root. Private fields prevent structurally valid but
/// unaudited epoch data from entering admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEpochStackCompositionInput {
    pure: EpochStackCompositionInput,
    body_evidence: StackLocalEvidence,
    realization_evidence: EntryStackRealizationEvidence,
}

impl BoundEpochStackCompositionInput {
    pub const fn root(&self) -> ExternalRootId {
        self.pure.root
    }

    pub const fn provider(&self) -> RootProviderId {
        self.pure.provider
    }

    pub const fn pure(&self) -> &EpochStackCompositionInput {
        &self.pure
    }

    pub const fn body_evidence(&self) -> &StackLocalEvidence {
        &self.body_evidence
    }

    pub const fn realization_evidence(&self) -> &EntryStackRealizationEvidence {
        &self.realization_evidence
    }
}

/// Bind an admitted opaque adapter realization to one exact provider summary,
/// installed entry, and public boundary plan.
pub fn bind_opaque_adapter_stack_realization(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    realization: ValidatedEntryStackRealization,
    arrival_contexts: AdmittedOpaqueArrivalContextSet,
) -> Result<BoundEpochStackCompositionInput, ExternalRootDiagnostic> {
    if !arrival_contexts.matches_installed_entry(summary, boundary, installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "opaque arrival-context evidence names a different installed root".into(),
        ));
    }
    let realized_contexts = realization
        .realization()
        .contexts
        .iter()
        .map(|context| context.context)
        .collect::<Vec<_>>();
    if realized_contexts != arrival_contexts.contexts {
        return Err(ExternalRootDiagnostic(
            "opaque arrival-context evidence and epoch realization contain different context sets"
                .into(),
        ));
    }
    let body_domains = body_domain_closure(boundary.plan().state.stack, &realization)?;
    validate_bound_entry_stack_realization(
        summary,
        boundary,
        installed_code,
        entry,
        &body_domains,
        &realization,
    )?;

    let realization_evidence = EntryStackRealizationEvidence {
        root: summary.root,
        provider: summary.provider,
        architecture: installed_code.architecture(),
        installed_code: installed_code.identity(),
        installed_code_context: installed_code.receipt_context(),
        artifact: installed_code.artifact(),
        entry,
        boundary_contract_report_fingerprint: boundary.contract_report_fingerprint(),
        boundary_contract_commitment: boundary.contract_commitment_digest(),
        body_domains,
        realization: realization.clone(),
        arrival_origin: ArrivalStackRealizationOrigin::OpaqueProvider,
        adapter_origin: AdapterStackRealizationOrigin::OpaqueProvider,
        target_rule_report_fingerprint: None,
        target_installation_validation_receipt: None,
        generated_adapter: None,
        validation_receipt: Some(arrival_contexts.validation_receipt()),
    };
    debug_assert!(realization_evidence.matches_installed_code_entry(installed_code, entry));
    Ok(BoundEpochStackCompositionInput {
        pure: EpochStackCompositionInput {
            root: summary.root,
            provider: summary.provider,
            realization,
            body_wcsu_bytes: summary.local_wcsu_bytes(),
            body_wcsu_alignment: summary.wcsu_alignment(),
        },
        body_evidence: summary.local_evidence.clone(),
        realization_evidence,
    })
}

/// Derive the complete stack realization for a compiler-emitted direct entry.
/// The only epoch is the exact Terminal body's epoch; no caller-authored epoch
/// or provider receipt is accepted on this path.
pub fn bind_direct_generated_entry_stack_realization(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    body_domains: ValidatedEntryStackDomainClosure,
) -> Result<BoundEpochStackCompositionInput, ExternalRootDiagnostic> {
    let StackLocalEvidence::TerminalEntry(binding) = &summary.local_evidence else {
        return Err(ExternalRootDiagnostic(
            "direct generated entry stack realization requires emitter-derived Terminal body evidence"
                .into(),
        ));
    };
    if !binding.matches_installed_entry(installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "direct generated entry body evidence names a different installed entry".into(),
        ));
    }
    if body_domains.boundary_stack() != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "direct generated entry stack-domain closure drifted from the boundary stack disposition"
                .into(),
        ));
    }
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: body_domains
            .contexts()
            .iter()
            .map(|context| ArrivalContextRealization {
                context: context.context,
                epochs: vec![EntryStackEpoch {
                    stage: EntryStackStage::Body,
                    active_domain: context.domain,
                    occupancy_by_domain: Vec::new(),
                    nesting: boundary.plan().state.preemption,
                }],
            })
            .collect(),
    })
    .map_err(|error| {
        ExternalRootDiagnostic(format!(
            "direct generated entry stack realization is invalid: {}",
            error.0
        ))
    })?;
    validate_bound_entry_stack_realization(
        summary,
        boundary,
        installed_code,
        entry,
        &body_domains,
        &realization,
    )?;
    Ok(BoundEpochStackCompositionInput {
        pure: EpochStackCompositionInput {
            root: summary.root,
            provider: summary.provider,
            realization: realization.clone(),
            body_wcsu_bytes: summary.local_wcsu_bytes(),
            body_wcsu_alignment: summary.wcsu_alignment(),
        },
        body_evidence: summary.local_evidence.clone(),
        realization_evidence: EntryStackRealizationEvidence {
            root: summary.root,
            provider: summary.provider,
            architecture: installed_code.architecture(),
            installed_code: installed_code.identity(),
            installed_code_context: installed_code.receipt_context(),
            artifact: installed_code.artifact(),
            entry,
            boundary_contract_report_fingerprint: boundary.contract_report_fingerprint(),
            boundary_contract_commitment: boundary.contract_commitment_digest(),
            body_domains,
            realization,
            arrival_origin: ArrivalStackRealizationOrigin::NoHardwareArrival,
            adapter_origin: AdapterStackRealizationOrigin::None,
            target_rule_report_fingerprint: None,
            target_installation_validation_receipt: None,
            generated_adapter: None,
            validation_receipt: None,
        },
    })
}

/// Exact production result of the installed gate/profile/TSS join.
///
/// This wrapper retains the opaque installed occurrence and table-validation
/// receipt beside the target-neutral facts and sealed architectural result.
/// A caller cannot strip that provenance and still enter the direct-entry
/// binder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledX86_64TargetDerivedHardwareArrival {
    installed_code_context: InstalledCodeContext,
    validation_receipt: X86_64GateProfileValidationReceiptId,
    facts: ValidatedX86_64InstalledHardwareEntryFacts,
    target_arrival: X86_64TargetDerivedHardwareArrival,
}

impl InstalledX86_64TargetDerivedHardwareArrival {
    pub const fn facts(&self) -> &ValidatedX86_64InstalledHardwareEntryFacts {
        &self.facts
    }

    pub const fn target_arrival(&self) -> &X86_64TargetDerivedHardwareArrival {
        &self.target_arrival
    }

    pub const fn validation_receipt(&self) -> X86_64GateProfileValidationReceiptId {
        self.validation_receipt
    }

    pub const fn report_fingerprint(&self) -> u64 {
        self.target_arrival.report_fingerprint()
    }

    #[cfg(test)]
    pub(crate) fn with_target_arrival_for_test(
        mut self,
        target_arrival: X86_64TargetDerivedHardwareArrival,
    ) -> Self {
        self.target_arrival = target_arrival;
        self
    }
}

/// Produce the sealed x86-64 target-fact carrier from one exact installed
/// gate/TSS realization.
///
/// The public gate/TSS details must exactly equal the independently sealed
/// table/profile roster. The consumer cannot author the selected hardware
/// stack, boundary nesting, compiler artifact identity, or architectural frame
/// size. Those values are derived here from the validated boundary plan and
/// exact `InstalledCode` occurrence before the ordinary target-rule validator
/// accepts the result.
pub fn produce_x86_64_installed_hardware_entry_facts(
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    entry_offset: u64,
    complete_roster: &ValidatedX86_64InstalledGateProfileRoster,
    mut realization: X86_64InstalledGateTssRealization,
) -> Result<InstalledX86_64TargetDerivedHardwareArrival, ExternalRootDiagnostic> {
    if installed_code.architecture() != omega_target::Architecture::X86_64
        || boundary.plan().state.initial_regime != MachineRegime::X86Long64
        || boundary.plan().call.entry_control != EntryControl::InterruptReturn
    {
        return Err(ExternalRootDiagnostic(
            "installed x86-64 gate/TSS realization requires an x86 long-mode InterruptReturn entry"
                .into(),
        ));
    }
    if !installed_code.binds_entry_offset(entry, entry_offset) {
        return Err(ExternalRootDiagnostic(
            "installed x86-64 gate names a different installed entry offset".into(),
        ));
    }
    if !complete_roster.matches_installed_entry(boundary, installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "x86-64 installed gate/profile roster names a different exact installed occurrence, entry, or boundary"
                .into(),
        ));
    }
    if realization.gate.entry_identity == 0
        || realization.gate.entry_identity != entry.normalized_identity()
    {
        return Err(ExternalRootDiagnostic(
            "installed x86-64 gate names a different compiler entry identity".into(),
        ));
    }
    if realization.gate.entry_privilege > 3 {
        return Err(ExternalRootDiagnostic(
            "installed x86-64 gate has an entry privilege outside 0..=3".into(),
        ));
    }
    if realization
        .gate
        .interrupt_stack_table_slot
        .is_some_and(|slot| !(1..=7).contains(&slot))
    {
        return Err(ExternalRootDiagnostic(
            "installed x86-64 gate selects an invalid interrupt-stack-table slot".into(),
        ));
    }

    let mut privilege_levels = BTreeSet::new();
    for stack in &realization.tss.privilege_stacks {
        if stack.entry_privilege > 2
            || stack.dedicated_class == 0
            || !privilege_levels.insert(stack.entry_privilege)
        {
            return Err(ExternalRootDiagnostic(
                "installed x86-64 TSS has an invalid or repeated privilege-stack selection".into(),
            ));
        }
    }
    let mut interrupt_slots = BTreeSet::new();
    for stack in &realization.tss.interrupt_stacks {
        if !(1..=7).contains(&stack.slot)
            || stack.dedicated_class == 0
            || !interrupt_slots.insert(stack.slot)
        {
            return Err(ExternalRootDiagnostic(
                "installed x86-64 TSS has an invalid or repeated interrupt-stack selection".into(),
            ));
        }
    }
    canonicalize_x86_64_installed_gate_tss_realization(&mut realization);

    let mut contexts = Vec::with_capacity(realization.arrivals.len());
    for arrival in &realization.arrivals {
        let stack_selection = if let Some(slot) = realization.gate.interrupt_stack_table_slot {
            let Some(stack) = realization
                .tss
                .interrupt_stacks
                .iter()
                .find(|stack| stack.slot == slot)
            else {
                return Err(ExternalRootDiagnostic(format!(
                    "installed x86-64 gate selects absent TSS interrupt-stack slot {slot}"
                )));
            };
            X86_64HardwareStackSelection::InterruptStackTable {
                slot,
                dedicated_class: stack.dedicated_class,
            }
        } else if arrival.interrupted_privilege == realization.gate.entry_privilege {
            X86_64HardwareStackSelection::Current
        } else {
            let Some(stack) = realization
                .tss
                .privilege_stacks
                .iter()
                .find(|stack| stack.entry_privilege == realization.gate.entry_privilege)
            else {
                return Err(ExternalRootDiagnostic(format!(
                    "installed x86-64 gate requires an absent TSS privilege-{} stack",
                    realization.gate.entry_privilege
                )));
            };
            X86_64HardwareStackSelection::PrivilegeTransition {
                dedicated_class: stack.dedicated_class,
            }
        };
        contexts.push(X86_64InstalledArrivalContext {
            context: arrival.context,
            mechanism: arrival.mechanism,
            interrupted_privilege: arrival.interrupted_privilege,
            entry_privilege: realization.gate.entry_privilege,
            stack_selection,
            nesting: boundary.plan().state.preemption,
        });
    }
    if realization != complete_roster.realization {
        return Err(ExternalRootDiagnostic(
            "x86-64 installed gate/TSS realization does not equal the complete validated gate/profile realization"
                .into(),
        ));
    }

    let facts = validate_x86_64_installed_hardware_entry_facts(X86_64InstalledHardwareEntryFacts {
        identity: InstalledEntryFactIdentity {
            target_profile: X86_64TargetProfileIdentity::LONG_MODE_INTERRUPT_GATES,
            artifact: installed_code.artifact().normalized_identity(),
            installed_code: installed_code.identity().normalized_identity(),
            entry: entry.normalized_identity(),
            entry_offset,
            boundary_plan_report_fingerprint: boundary.contract_report_fingerprint(),
            boundary_plan_commitment: boundary.contract_commitment_digest(),
        },
        vector: realization.gate.vector,
        gate: realization.gate.gate,
        boundary_stack: boundary.plan().state.stack,
        contexts,
    })
    .map_err(|diagnostic| ExternalRootDiagnostic(diagnostic.0))?;
    let target_arrival = derive_x86_64_hardware_arrival(&facts)
        .map_err(|diagnostic| ExternalRootDiagnostic(diagnostic.0))?;
    Ok(InstalledX86_64TargetDerivedHardwareArrival {
        installed_code_context: installed_code.receipt_context(),
        validation_receipt: complete_roster.validation_receipt,
        facts,
        target_arrival,
    })
}

/// Bind sealed x86-64 hardware-arrival epochs directly to an emitted Terminal
/// body when no software adapter changes stacks between arrival and body.
///
/// The target rule owns frame geometry and the complete context set. This
/// binder replays every nominal installation coordinate before that derived
/// realization can enter external-root admission.
pub fn bind_x86_64_target_direct_entry_stack_realization(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    installed_target_arrival: &InstalledX86_64TargetDerivedHardwareArrival,
) -> Result<BoundEpochStackCompositionInput, ExternalRootDiagnostic> {
    let StackLocalEvidence::TerminalEntry(binding) = &summary.local_evidence else {
        return Err(ExternalRootDiagnostic(
            "target-derived direct entry stack realization requires emitter-derived Terminal body evidence"
                .into(),
        ));
    };
    if installed_code.architecture() != omega_target::Architecture::X86_64 {
        return Err(ExternalRootDiagnostic(
            "x86-64 target arrival evidence cannot bind a non-x86 installed artifact".into(),
        ));
    }
    if !binding.matches_installed_entry(installed_code, entry) {
        return Err(ExternalRootDiagnostic(
            "target-derived direct entry body evidence names a different installed entry".into(),
        ));
    }
    if installed_target_arrival.installed_code_context != installed_code.receipt_context() {
        return Err(ExternalRootDiagnostic(
            "x86-64 target arrival evidence names a different exact installed occurrence".into(),
        ));
    }
    let target_arrival = installed_target_arrival.target_arrival();
    let identity = target_arrival.installed_identity();
    if identity.artifact != installed_code.artifact().normalized_identity()
        || identity.installed_code != installed_code.identity().normalized_identity()
        || identity.entry != entry.normalized_identity()
        || identity.boundary_plan_report_fingerprint != boundary.contract_report_fingerprint()
        || identity.boundary_plan_commitment == [0; 32]
        || identity.boundary_plan_commitment != boundary.contract_commitment_digest()
        || !installed_code.binds_entry_offset(entry, identity.entry_offset)
    {
        return Err(ExternalRootDiagnostic(
            "x86-64 target arrival facts name a different installed artifact, entry, or boundary plan"
                .into(),
        ));
    }
    let body_domains = target_arrival.body_domains().clone();
    let realization = target_arrival.realization().clone();
    validate_bound_entry_stack_realization(
        summary,
        boundary,
        installed_code,
        entry,
        &body_domains,
        &realization,
    )?;
    Ok(BoundEpochStackCompositionInput {
        pure: EpochStackCompositionInput {
            root: summary.root,
            provider: summary.provider,
            realization: realization.clone(),
            body_wcsu_bytes: summary.local_wcsu_bytes(),
            body_wcsu_alignment: summary.wcsu_alignment(),
        },
        body_evidence: summary.local_evidence.clone(),
        realization_evidence: EntryStackRealizationEvidence {
            root: summary.root,
            provider: summary.provider,
            architecture: installed_code.architecture(),
            installed_code: installed_code.identity(),
            installed_code_context: installed_code.receipt_context(),
            artifact: installed_code.artifact(),
            entry,
            boundary_contract_report_fingerprint: boundary.contract_report_fingerprint(),
            boundary_contract_commitment: boundary.contract_commitment_digest(),
            body_domains,
            realization,
            arrival_origin: ArrivalStackRealizationOrigin::X86_64TargetRule,
            adapter_origin: AdapterStackRealizationOrigin::None,
            target_rule_report_fingerprint: Some(target_arrival.report_fingerprint()),
            target_installation_validation_receipt: Some(
                installed_target_arrival.validation_receipt(),
            ),
            generated_adapter: None,
            validation_receipt: None,
        },
    })
}

/// Bind the exact emitted receiver-free x86 ProgramStorage wrapper to one
/// installed Terminal entry. This is generated adapter evidence only: it
/// proves the wrapper's stack geometry and resolved continuation call, not
/// firmware invocation or an actual stack mutation.
pub fn bind_x86_64_generated_program_storage_adapter_stack_realization(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    body_domains: ValidatedEntryStackDomainClosure,
    emission: X86_64GeneratedProgramStorageAdapterEmission<'_>,
) -> Result<BoundEpochStackCompositionInput, ExternalRootDiagnostic> {
    let StackLocalEvidence::TerminalEntry(binding) = &summary.local_evidence else {
        return Err(ExternalRootDiagnostic(
            "generated ProgramStorage adapter requires emitter-derived Terminal body evidence"
                .into(),
        ));
    };
    if installed_code.architecture() != omega_target::Architecture::X86_64 {
        return Err(ExternalRootDiagnostic(
            "x86-64 generated ProgramStorage adapter cannot bind a non-x86 installed artifact"
                .into(),
        ));
    }
    if !binding.matches_installed_entry(installed_code, entry)
        || !installed_code.binds_entry_offset(entry, emission.wrapper_section_offset)
    {
        return Err(ExternalRootDiagnostic(
            "generated ProgramStorage adapter body evidence names a different installed entry"
                .into(),
        ));
    }
    let expected_boundary = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(16, 8), ValueShape::integer(16, 8)],
            result: None,
        },
    )
    .expect("the closed receiver-free ProgramStorage wrapper ABI validates");
    if boundary.plan().call != expected_boundary.plan().call {
        return Err(ExternalRootDiagnostic(
            "generated ProgramStorage adapter requires the exact receiver-free Microsoft-x64 semantic continuation ABI"
                .into(),
        ));
    }
    let template =
        validate_x86_64_semantic_unit_wrapper_template(emission.request, emission.template_bytes)
            .map_err(|_| {
            ExternalRootDiagnostic(
                "generated ProgramStorage adapter template failed exact emitted-operation replay"
                    .into(),
            )
        })?;
    let resolved = validate_x86_64_resolved_semantic_unit_wrapper(
        &template,
        template.relocation(),
        emission.wrapper_section_offset,
        emission.continuation_section_offset,
        emission.resolved_bytes,
    )
    .map_err(|_| {
        ExternalRootDiagnostic(
            "generated ProgramStorage adapter continuation call failed exact emitted-operation replay"
                .into(),
            )
        })?;
    if !installed_code.binds_exact_materialized_entry_bytes(entry, resolved.bytes()) {
        return Err(ExternalRootDiagnostic(
            "generated ProgramStorage adapter bytes do not equal the exact installed entry interval"
                .into(),
        ));
    }
    if body_domains.boundary_stack() != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "generated ProgramStorage adapter stack-domain closure drifted from the boundary stack disposition"
                .into(),
        ));
    }

    let frame_bytes = u64::from(emission.request.outgoing_frame_byte_count);
    let frame_alignment = u64::from(emission.request.pre_call_stack_alignment);
    let realization = validate_entry_stack_realization(EntryStackRealization {
        contexts: body_domains
            .contexts()
            .iter()
            .map(|context| ArrivalContextRealization {
                context: context.context,
                epochs: [
                    EntryStackStage::Enter,
                    EntryStackStage::Body,
                    EntryStackStage::Exit,
                ]
                .map(|stage| EntryStackEpoch {
                    stage,
                    active_domain: context.domain,
                    occupancy_by_domain: vec![StackOccupancy {
                        domain: context.domain,
                        bytes: frame_bytes,
                        alignment: frame_alignment,
                    }],
                    nesting: boundary.plan().state.preemption,
                })
                .into(),
            })
            .collect(),
    })
    .map_err(|error| {
        ExternalRootDiagnostic(format!(
            "generated ProgramStorage adapter stack realization is invalid: {}",
            error.0
        ))
    })?;
    validate_bound_entry_stack_realization(
        summary,
        boundary,
        installed_code,
        entry,
        &body_domains,
        &realization,
    )?;

    let resolution = resolved.resolution();
    let mut adapter_fingerprint = Fnv1a::new();
    adapter_fingerprint.u64(0x6765_6e5f_7073_7772); // "gen_pswr"
    adapter_fingerprint.bytes(resolved.bytes());
    adapter_fingerprint.u64(resolution.wrapper_section_offset);
    adapter_fingerprint.u64(resolution.continuation_section_offset);
    adapter_fingerprint.u64(resolution.next_instruction_section_offset);
    adapter_fingerprint.u64(resolution.displacement as u64);
    let generated_adapter = GeneratedProgramStorageAdapterStackEvidence {
        request: emission.request,
        resolved_bytes: resolved.bytes().to_vec(),
        resolution,
        non_authoritative_report_fingerprint: adapter_fingerprint.finish(),
    };
    Ok(BoundEpochStackCompositionInput {
        pure: EpochStackCompositionInput {
            root: summary.root,
            provider: summary.provider,
            realization: realization.clone(),
            body_wcsu_bytes: summary.local_wcsu_bytes(),
            body_wcsu_alignment: summary.wcsu_alignment(),
        },
        body_evidence: summary.local_evidence.clone(),
        realization_evidence: EntryStackRealizationEvidence {
            root: summary.root,
            provider: summary.provider,
            architecture: installed_code.architecture(),
            installed_code: installed_code.identity(),
            installed_code_context: installed_code.receipt_context(),
            artifact: installed_code.artifact(),
            entry,
            boundary_contract_report_fingerprint: boundary.contract_report_fingerprint(),
            boundary_contract_commitment: boundary.contract_commitment_digest(),
            body_domains,
            realization,
            arrival_origin: ArrivalStackRealizationOrigin::NoHardwareArrival,
            adapter_origin: AdapterStackRealizationOrigin::GeneratedProgramStorageSemanticWrapper,
            target_rule_report_fingerprint: None,
            target_installation_validation_receipt: None,
            generated_adapter: Some(generated_adapter),
            validation_receipt: None,
        },
    })
}

fn validate_bound_entry_stack_realization(
    summary: &ProviderStackSummary,
    boundary: &ValidatedBoundaryEntryPlan,
    installed_code: &InstalledCode,
    entry: EntryStubId,
    body_domains: &ValidatedEntryStackDomainClosure,
    realization: &ValidatedEntryStackRealization,
) -> Result<(), ExternalRootDiagnostic> {
    installed_code.selected_entry_target(entry).map_err(|_| {
        ExternalRootDiagnostic("entry stack realization names no exact installed entry".into())
    })?;
    if boundary.plan().state.initial_regime.architecture() != installed_code.architecture() {
        return Err(ExternalRootDiagnostic(
            "entry stack realization target differs from the installed artifact architecture"
                .into(),
        ));
    }
    if summary.stack != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "entry stack summary drifted from the boundary plan's stack disposition".into(),
        ));
    }
    if body_domains.boundary_stack() != boundary.plan().state.stack {
        return Err(ExternalRootDiagnostic(
            "entry stack domain closure drifted from the boundary stack disposition".into(),
        ));
    }
    if body_domains.contexts().len() != realization.realization().contexts.len() {
        return Err(ExternalRootDiagnostic(
            "entry stack domain closure and realization contain different arrival-context sets"
                .into(),
        ));
    }
    for context in &realization.realization().contexts {
        let body = context
            .epochs
            .iter()
            .find(|epoch| epoch.stage == EntryStackStage::Body)
            .expect("validated realization has exactly one body epoch");
        let Some(closed) = body_domains
            .contexts()
            .iter()
            .find(|closed| closed.context == context.context)
        else {
            return Err(ExternalRootDiagnostic(format!(
                "entry stack arrival context 0x{:016x} is absent from the domain closure",
                context.context.get()
            )));
        };
        if body.active_domain != closed.domain {
            return Err(ExternalRootDiagnostic(format!(
                "entry stack arrival context 0x{:016x} executes its body on a domain other than its exact context closure",
                context.context.get()
            )));
        }
        for epoch in &context.epochs {
            if epoch.nesting == Preemption::ProviderDefined {
                return Err(ExternalRootDiagnostic(format!(
                    "entry stack arrival context 0x{:016x} retains unresolved provider-defined nesting",
                    context.context.get()
                )));
            }
            if !preemption_refines(epoch.nesting, boundary.plan().state.preemption) {
                return Err(ExternalRootDiagnostic(format!(
                    "entry stack arrival context 0x{:016x} widens the boundary plan's nesting ceiling",
                    context.context.get()
                )));
            }
        }
    }
    if let StackLocalEvidence::TerminalEntry(binding) = &summary.local_evidence
        && !binding.matches_installed_entry(installed_code, entry)
    {
        return Err(ExternalRootDiagnostic(
            "terminal body WCSU and entry realization name different installed entries".into(),
        ));
    }
    Ok(())
}

fn body_domain_closure(
    boundary_stack: EntryStack,
    realization: &ValidatedEntryStackRealization,
) -> Result<ValidatedEntryStackDomainClosure, ExternalRootDiagnostic> {
    validate_entry_stack_domain_closure(
        boundary_stack,
        realization
            .realization()
            .contexts
            .iter()
            .map(|context| {
                let body = context
                    .epochs
                    .iter()
                    .find(|epoch| epoch.stage == EntryStackStage::Body)
                    .expect("validated realization has exactly one body epoch");
                ArrivalContextStackDomain {
                    context: context.context,
                    domain: body.active_domain,
                }
            })
            .collect(),
    )
    .map_err(|error| {
        ExternalRootDiagnostic(format!(
            "entry stack body-domain closure is invalid: {}",
            error.0
        ))
    })
}

fn preemption_refines(actual: Preemption, ceiling: Preemption) -> bool {
    match (actual, ceiling) {
        (_, Preemption::ProviderDefined) => true,
        (Preemption::NotApplicable | Preemption::Masked, Preemption::Nestable { .. }) => true,
        (
            Preemption::Nestable {
                maximum_depth: actual,
            },
            Preemption::Nestable {
                maximum_depth: ceiling,
            },
        ) => actual <= ceiling,
        (actual, ceiling) => actual == ceiling,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DomainStackDemand {
    pub bytes: u64,
    pub alignment: u64,
}

/// Context-maximized result for one root occurrence at artifact entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComposedEpochStackDemand {
    root: ExternalRootId,
    provider: RootProviderId,
    by_domain: BTreeMap<StackDomain, DomainStackDemand>,
    contributing_roots: BTreeSet<ExternalRootId>,
}

impl ComposedEpochStackDemand {
    pub const fn root(&self) -> ExternalRootId {
        self.root
    }

    pub const fn provider(&self) -> RootProviderId {
        self.provider
    }

    pub fn domain(&self, domain: StackDomain) -> Option<DomainStackDemand> {
        self.by_domain.get(&domain).copied()
    }

    pub fn domains(&self) -> impl Iterator<Item = (StackDomain, DomainStackDemand)> + '_ {
        self.by_domain
            .iter()
            .map(|(domain, demand)| (*domain, *demand))
    }

    pub const fn contributing_roots(&self) -> &BTreeSet<ExternalRootId> {
        &self.contributing_roots
    }
}

/// Exact pure-composition result. The retained inputs prevent a compact
/// fingerprint collision from becoming authority or equality evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpochStackComposition {
    relation: StackNestingRelation,
    inputs: BTreeMap<ExternalRootId, EpochStackCompositionInput>,
    demands: BTreeMap<ExternalRootId, ComposedEpochStackDemand>,
    domain_wcsu: BTreeMap<StackDomain, DomainStackDemand>,
    non_authoritative_report_fingerprint: u64,
}

impl EpochStackComposition {
    pub const fn relation(&self) -> &StackNestingRelation {
        &self.relation
    }

    pub fn demand(&self, root: ExternalRootId) -> Option<&ComposedEpochStackDemand> {
        self.demands.get(&root)
    }

    pub fn domain(&self, domain: StackDomain) -> Option<DomainStackDemand> {
        self.domain_wcsu.get(&domain).copied()
    }

    /// Compatibility accessor for the non-authoritative report/cache
    /// fingerprint. Exact relation, inputs, and demands remain retained.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    pub const fn non_authoritative_report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }
}

/// Admission-capable epoch composition retaining every exact body and adapter
/// evidence row behind the pure arithmetic result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEpochStackComposition {
    composition: EpochStackComposition,
    inputs: BTreeMap<ExternalRootId, BoundEpochStackCompositionInput>,
    non_authoritative_report_fingerprint: u64,
}

impl BoundEpochStackComposition {
    pub const fn composition(&self) -> &EpochStackComposition {
        &self.composition
    }

    pub fn input(&self, root: ExternalRootId) -> Option<&BoundEpochStackCompositionInput> {
        self.inputs.get(&root)
    }

    pub fn inputs(
        &self,
    ) -> impl Iterator<Item = (&ExternalRootId, &BoundEpochStackCompositionInput)> {
        self.inputs.iter()
    }

    pub fn demand(&self, root: ExternalRootId) -> Option<&ComposedEpochStackDemand> {
        self.composition.demand(root)
    }

    pub fn domain(&self, domain: StackDomain) -> Option<DomainStackDemand> {
        self.composition.domain(domain)
    }

    pub const fn relation(&self) -> &StackNestingRelation {
        self.composition.relation()
    }

    /// Compatibility accessor for the non-authoritative report/cache
    /// fingerprint. Every exact bound body and realization row is retained.
    pub const fn report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }

    pub const fn non_authoritative_report_fingerprint(&self) -> u64 {
        self.non_authoritative_report_fingerprint
    }
}

pub fn compose_bound_entry_stack_epochs<'a>(
    relation: &StackNestingRelation,
    inputs: impl IntoIterator<Item = &'a BoundEpochStackCompositionInput>,
) -> Result<BoundEpochStackComposition, ExternalRootDiagnostic> {
    let mut bound = BTreeMap::new();
    for input in inputs {
        if bound.insert(input.root(), input.clone()).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "bound epoch stack input for root 0x{:016x} is duplicated",
                input.root().normalized_identity()
            )));
        }
    }
    let composition = compose_entry_stack_epochs(
        relation,
        bound.values().map(BoundEpochStackCompositionInput::pure),
    )?;
    let mut report_fingerprint = Fnv1a::new();
    report_fingerprint.u64(composition.report_fingerprint());
    report_fingerprint.u64(bound.len() as u64);
    for input in bound.values() {
        fingerprint_stack_local_evidence(&mut report_fingerprint, input.body_evidence());
        let evidence = input.realization_evidence();
        report_fingerprint.u64(match evidence.arrival_origin() {
            ArrivalStackRealizationOrigin::NoHardwareArrival => 0,
            ArrivalStackRealizationOrigin::X86_64TargetRule => 1,
            ArrivalStackRealizationOrigin::OpaqueProvider => 2,
        });
        report_fingerprint.u64(match evidence.adapter_origin() {
            AdapterStackRealizationOrigin::None => 0,
            AdapterStackRealizationOrigin::GeneratedProgramStorageSemanticWrapper => 1,
            AdapterStackRealizationOrigin::OpaqueProvider => 2,
        });
        report_fingerprint.u64(evidence.root().normalized_identity());
        report_fingerprint.u64(evidence.provider().normalized_identity());
        report_fingerprint.u64(match evidence.architecture() {
            omega_target::Architecture::X86_64 => 1,
            omega_target::Architecture::Aarch64 => 2,
        });
        report_fingerprint.u64(evidence.installed_code().normalized_identity());
        report_fingerprint.u64(evidence.artifact().normalized_identity());
        report_fingerprint.u64(evidence.entry().normalized_identity());
        report_fingerprint.u64(evidence.boundary_contract_report_fingerprint());
        report_fingerprint.bytes(&evidence.boundary_contract_commitment());
        report_fingerprint.u64(evidence.body_domains.report_fingerprint());
        report_fingerprint.u64(evidence.realization().report_fingerprint());
        report_fingerprint.u64(
            evidence
                .target_rule_report_fingerprint()
                .unwrap_or_default(),
        );
        report_fingerprint.u64(
            evidence
                .target_installation_validation_receipt()
                .map(X86_64GateProfileValidationReceiptId::normalized_identity)
                .unwrap_or_default(),
        );
        report_fingerprint.u64(
            evidence
                .generated_adapter()
                .map(GeneratedProgramStorageAdapterStackEvidence::report_fingerprint)
                .unwrap_or_default(),
        );
        report_fingerprint.u64(
            evidence
                .validation_receipt()
                .map(StackValidationReceiptId::normalized_identity)
                .unwrap_or_default(),
        );
    }
    Ok(BoundEpochStackComposition {
        composition,
        inputs: bound,
        non_authoritative_report_fingerprint: report_fingerprint.finish(),
    })
}

/// Compose structurally validated epoch plans.
///
/// Epochs within one context are sequential alternatives and take their
/// per-domain maximum. Arrival contexts are alternatives and also take their
/// maximum. A permitted nested occurrence is concurrent: its per-domain demand
/// is appended with alignment to the parent epoch's live occupancy. Relative
/// `Interrupted` domains resolve to the parent epoch's active domain.
pub fn compose_entry_stack_epochs<'a>(
    relation: &StackNestingRelation,
    inputs: impl IntoIterator<Item = &'a EpochStackCompositionInput>,
) -> Result<EpochStackComposition, ExternalRootDiagnostic> {
    let mut by_root = BTreeMap::new();
    for input in inputs {
        validate_input(input)?;
        if by_root.insert(input.root, input.clone()).is_some() {
            return Err(ExternalRootDiagnostic(format!(
                "epoch stack input for root 0x{:016x} is duplicated",
                input.root.normalized_identity()
            )));
        }
    }
    if by_root.is_empty() {
        return Err(ExternalRootDiagnostic(
            "epoch stack composition requires at least one root input".into(),
        ));
    }

    let mut outgoing: BTreeMap<ExternalRootId, Vec<ExternalRootId>> = BTreeMap::new();
    for edge in &relation.edges {
        if !by_root.contains_key(&edge.interrupted) {
            return Err(ExternalRootDiagnostic(format!(
                "stack nesting relation references missing interrupted root 0x{:016x}",
                edge.interrupted.normalized_identity()
            )));
        }
        if !by_root.contains_key(&edge.preemptor) {
            return Err(ExternalRootDiagnostic(format!(
                "stack nesting relation references missing preemptor root 0x{:016x}",
                edge.preemptor.normalized_identity()
            )));
        }
        outgoing
            .entry(edge.interrupted)
            .or_default()
            .push(edge.preemptor);
    }

    let domains = stack_domains(&by_root);
    let maximum_depth = maximum_nesting_depth(&by_root);
    let mut memo = BTreeMap::new();
    for live_depth in (1..=maximum_depth).rev() {
        for interrupted_domain in &domains {
            for input in by_root.values() {
                let composed = compose_root_at_depth(
                    input.root,
                    *interrupted_domain,
                    live_depth,
                    &outgoing,
                    &by_root,
                    &memo,
                )?;
                memo.insert((input.root, *interrupted_domain, live_depth), composed);
            }
        }
    }

    let mut demands = BTreeMap::new();
    let mut domain_wcsu = BTreeMap::new();
    for input in by_root.values() {
        let composed = memo
            .get(&(input.root, StackDomain::Interrupted, 1))
            .expect("depth-one root state was composed")
            .clone();
        merge_alternative_map(&mut domain_wcsu, &composed.by_domain);
        demands.insert(input.root, composed);
    }

    let non_authoritative_report_fingerprint =
        non_authoritative_epoch_stack_inputs_report_fingerprint(relation, &by_root);
    Ok(EpochStackComposition {
        relation: relation.clone(),
        inputs: by_root,
        demands,
        domain_wcsu,
        non_authoritative_report_fingerprint,
    })
}

fn validate_input(input: &EpochStackCompositionInput) -> Result<(), ExternalRootDiagnostic> {
    if input.body_wcsu_bytes == 0 {
        return Err(ExternalRootDiagnostic(format!(
            "epoch stack input for root 0x{:016x} has zero body WCSU",
            input.root.normalized_identity()
        )));
    }
    if input.body_wcsu_alignment == 0 || !input.body_wcsu_alignment.is_power_of_two() {
        return Err(ExternalRootDiagnostic(format!(
            "epoch stack input for root 0x{:016x} has body alignment {} instead of a nonzero power of two",
            input.root.normalized_identity(),
            input.body_wcsu_alignment
        )));
    }
    Ok(())
}

fn maximum_nesting_depth(inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>) -> u16 {
    inputs
        .values()
        .flat_map(|input| &input.realization.realization().contexts)
        .flat_map(|context| &context.epochs)
        .filter_map(|epoch| match epoch.nesting {
            Preemption::Nestable { maximum_depth } => Some(maximum_depth),
            Preemption::NotApplicable | Preemption::Masked | Preemption::ProviderDefined => None,
        })
        .max()
        .unwrap_or(1)
}

fn stack_domains(
    inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>,
) -> BTreeSet<StackDomain> {
    let mut domains = BTreeSet::from([StackDomain::Interrupted]);
    for epoch in inputs
        .values()
        .flat_map(|input| &input.realization.realization().contexts)
        .flat_map(|context| &context.epochs)
    {
        if let StackDomainRef::Dedicated { class } = epoch.active_domain {
            domains.insert(StackDomain::Dedicated { class });
        }
        for occupancy in &epoch.occupancy_by_domain {
            if let StackDomainRef::Dedicated { class } = occupancy.domain {
                domains.insert(StackDomain::Dedicated { class });
            }
        }
    }
    domains
}

fn compose_root_at_depth(
    root: ExternalRootId,
    interrupted_domain: StackDomain,
    live_depth: u16,
    outgoing: &BTreeMap<ExternalRootId, Vec<ExternalRootId>>,
    inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>,
    memo: &BTreeMap<(ExternalRootId, StackDomain, u16), ComposedEpochStackDemand>,
) -> Result<ComposedEpochStackDemand, ExternalRootDiagnostic> {
    let input = inputs.get(&root).expect("nesting endpoint was validated");
    let mut root_peak = BTreeMap::new();
    let mut contributing_roots = BTreeSet::from([root]);

    for context in &input.realization.realization().contexts {
        let mut context_peak = BTreeMap::new();
        for epoch in &context.epochs {
            let active_domain = resolve_domain(epoch.active_domain, interrupted_domain)?;
            let mut base = BTreeMap::new();
            for occupancy in &epoch.occupancy_by_domain {
                let domain = resolve_domain(occupancy.domain, interrupted_domain)?;
                append_demand(
                    &mut base,
                    domain,
                    DomainStackDemand {
                        bytes: occupancy.bytes,
                        alignment: occupancy.alignment,
                    },
                )?;
            }
            if epoch.stage == EntryStackStage::Body {
                append_demand(
                    &mut base,
                    active_domain,
                    DomainStackDemand {
                        bytes: input.body_wcsu_bytes,
                        alignment: input.body_wcsu_alignment,
                    },
                )?;
            }

            let mut epoch_peak = base.clone();
            if let Preemption::Nestable { maximum_depth } = epoch.nesting
                && live_depth < maximum_depth
                && let Some(preemptors) = outgoing.get(&root)
            {
                for preemptor in preemptors {
                    let nested = memo
                        .get(&(*preemptor, active_domain, live_depth + 1))
                        .expect("deeper nesting state was composed first");
                    contributing_roots.extend(nested.contributing_roots.iter().copied());
                    let concurrent = append_maps(&base, &nested.by_domain)?;
                    merge_alternative_map(&mut epoch_peak, &concurrent);
                }
            }
            merge_alternative_map(&mut context_peak, &epoch_peak);
        }
        merge_alternative_map(&mut root_peak, &context_peak);
    }

    Ok(ComposedEpochStackDemand {
        root,
        provider: input.provider,
        by_domain: root_peak,
        contributing_roots,
    })
}

fn resolve_domain(
    domain: StackDomainRef,
    interrupted_domain: StackDomain,
) -> Result<StackDomain, ExternalRootDiagnostic> {
    match domain {
        StackDomainRef::Interrupted => Ok(interrupted_domain),
        StackDomainRef::Dedicated { class } => Ok(StackDomain::Dedicated { class }),
        StackDomainRef::ProviderSelected => Err(ExternalRootDiagnostic(
            "validated epoch stack realization retained a provider-selected domain".into(),
        )),
    }
}

fn append_maps(
    parent: &BTreeMap<StackDomain, DomainStackDemand>,
    nested: &BTreeMap<StackDomain, DomainStackDemand>,
) -> Result<BTreeMap<StackDomain, DomainStackDemand>, ExternalRootDiagnostic> {
    let mut combined = parent.clone();
    for (domain, demand) in nested {
        append_demand(&mut combined, *domain, *demand)?;
    }
    Ok(combined)
}

fn append_demand(
    demands: &mut BTreeMap<StackDomain, DomainStackDemand>,
    domain: StackDomain,
    appended: DomainStackDemand,
) -> Result<(), ExternalRootDiagnostic> {
    match demands.get_mut(&domain) {
        Some(existing) => {
            existing.bytes =
                super::stack_demand::align_up_checked(existing.bytes, appended.alignment)?
                    .checked_add(appended.bytes)
                    .ok_or_else(|| {
                        ExternalRootDiagnostic("stack epoch demand addition overflowed".into())
                    })?;
            existing.alignment = existing.alignment.max(appended.alignment);
        }
        None => {
            demands.insert(domain, appended);
        }
    }
    Ok(())
}

fn merge_alternative_map(
    target: &mut BTreeMap<StackDomain, DomainStackDemand>,
    alternative: &BTreeMap<StackDomain, DomainStackDemand>,
) {
    for (domain, demand) in alternative {
        target
            .entry(*domain)
            .and_modify(|current| {
                current.bytes = current.bytes.max(demand.bytes);
                current.alignment = current.alignment.max(demand.alignment);
            })
            .or_insert(*demand);
    }
}

fn non_authoritative_epoch_stack_inputs_report_fingerprint(
    relation: &StackNestingRelation,
    inputs: &BTreeMap<ExternalRootId, EpochStackCompositionInput>,
) -> u64 {
    let mut hash = Fnv1a::new();
    hash.u64(relation.identity.normalized_identity());
    hash.u64(inputs.len() as u64);
    for input in inputs.values() {
        hash.u64(input.root.normalized_identity());
        hash.u64(input.provider.normalized_identity());
        hash.u64(input.realization.report_fingerprint());
        hash.u64(input.body_wcsu_bytes);
        hash.u64(input.body_wcsu_alignment);
    }
    hash.u64(relation.edges.len() as u64);
    for edge in &relation.edges {
        hash.u64(edge.interrupted.normalized_identity());
        hash.u64(edge.preemptor.normalized_identity());
    }
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::super::{NestingRelationId, StackNestingEdge};
    use super::*;
    use omega_calling_conventions::{
        ArrivalContextId, ArrivalContextRealization, EntryStackEpoch, EntryStackRealization,
        StackOccupancy, validate_entry_stack_realization,
    };

    fn id<T>(value: u64, make: impl FnOnce(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        make(value).expect("nonzero normalized identity")
    }

    fn realization(contexts: Vec<ArrivalContextRealization>) -> ValidatedEntryStackRealization {
        validate_entry_stack_realization(EntryStackRealization { contexts })
            .expect("valid stack realization")
    }

    fn epoch(
        stage: EntryStackStage,
        active_domain: StackDomainRef,
        occupancy_by_domain: Vec<StackOccupancy>,
        nesting: Preemption,
    ) -> EntryStackEpoch {
        EntryStackEpoch {
            stage,
            active_domain,
            occupancy_by_domain,
            nesting,
        }
    }

    fn context(value: u64, epochs: Vec<EntryStackEpoch>) -> ArrivalContextRealization {
        ArrivalContextRealization {
            context: ArrivalContextId::new(value).expect("nonzero context"),
            epochs,
        }
    }

    #[test]
    fn epochs_and_contexts_take_maxima_while_body_wcsu_joins_only_the_body_domain() {
        let root = id(1, ExternalRootId::from_normalized_identity);
        let input = EpochStackCompositionInput {
            root,
            provider: id(2, RootProviderId::from_normalized_identity),
            realization: realization(vec![
                context(
                    1,
                    vec![
                        epoch(
                            EntryStackStage::Enter,
                            StackDomainRef::Interrupted,
                            vec![StackOccupancy {
                                domain: StackDomainRef::Interrupted,
                                bytes: 120,
                                alignment: 8,
                            }],
                            Preemption::Masked,
                        ),
                        epoch(
                            EntryStackStage::Body,
                            StackDomainRef::Dedicated { class: 4 },
                            vec![StackOccupancy {
                                domain: StackDomainRef::Dedicated { class: 4 },
                                bytes: 8,
                                alignment: 8,
                            }],
                            Preemption::Masked,
                        ),
                    ],
                ),
                context(
                    2,
                    vec![epoch(
                        EntryStackStage::Body,
                        StackDomainRef::Interrupted,
                        vec![StackOccupancy {
                            domain: StackDomainRef::Interrupted,
                            bytes: 24,
                            alignment: 8,
                        }],
                        Preemption::Masked,
                    )],
                ),
            ]),
            body_wcsu_bytes: 64,
            body_wcsu_alignment: 16,
        };
        let composed = compose_entry_stack_epochs(
            &StackNestingRelation {
                identity: id(3, NestingRelationId::from_normalized_identity),
                edges: BTreeSet::new(),
            },
            [&input],
        )
        .expect("context-aware composition");

        assert_eq!(
            composed.domain(StackDomain::Interrupted),
            Some(DomainStackDemand {
                bytes: 120,
                alignment: 16,
            })
        );
        assert_eq!(
            composed.domain(StackDomain::Dedicated { class: 4 }),
            Some(DomainStackDemand {
                bytes: 80,
                alignment: 16,
            })
        );
    }

    #[test]
    fn compact_equal_epoch_compositions_remain_structurally_distinct() {
        let root = id(4, ExternalRootId::from_normalized_identity);
        let provider = id(5, RootProviderId::from_normalized_identity);
        let relation = StackNestingRelation {
            identity: id(6, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        };
        let input = |body_wcsu_bytes| EpochStackCompositionInput {
            root,
            provider,
            realization: realization(vec![context(
                1,
                vec![epoch(
                    EntryStackStage::Body,
                    StackDomainRef::Interrupted,
                    Vec::new(),
                    Preemption::Masked,
                )],
            )]),
            body_wcsu_bytes,
            body_wcsu_alignment: 8,
        };
        let first_input = input(64);
        let second_input = input(72);
        let exact = compose_entry_stack_epochs(&relation, [&first_input])
            .expect("first exact epoch composition");
        let mut substituted = compose_entry_stack_epochs(&relation, [&second_input])
            .expect("second exact epoch composition");
        substituted.non_authoritative_report_fingerprint =
            exact.non_authoritative_report_fingerprint;

        assert_eq!(
            exact.non_authoritative_report_fingerprint(),
            substituted.non_authoritative_report_fingerprint()
        );
        assert_ne!(exact, substituted);
    }

    #[test]
    fn nested_interrupted_is_path_relative_and_finite_depth_closes_cycles() {
        let parent = id(10, ExternalRootId::from_normalized_identity);
        let child = id(11, ExternalRootId::from_normalized_identity);
        let provider = id(12, RootProviderId::from_normalized_identity);
        let parent_input = EpochStackCompositionInput {
            root: parent,
            provider,
            realization: realization(vec![context(
                1,
                vec![epoch(
                    EntryStackStage::Body,
                    StackDomainRef::Dedicated { class: 4 },
                    vec![StackOccupancy {
                        domain: StackDomainRef::Dedicated { class: 4 },
                        bytes: 24,
                        alignment: 8,
                    }],
                    Preemption::Nestable { maximum_depth: 2 },
                )],
            )]),
            body_wcsu_bytes: 40,
            body_wcsu_alignment: 16,
        };
        let child_input = EpochStackCompositionInput {
            root: child,
            provider,
            realization: realization(vec![context(
                1,
                vec![epoch(
                    EntryStackStage::Body,
                    StackDomainRef::Interrupted,
                    vec![StackOccupancy {
                        domain: StackDomainRef::Interrupted,
                        bytes: 8,
                        alignment: 8,
                    }],
                    Preemption::Nestable { maximum_depth: 2 },
                )],
            )]),
            body_wcsu_bytes: 16,
            body_wcsu_alignment: 16,
        };
        let relation = StackNestingRelation {
            identity: id(13, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::from([
                StackNestingEdge {
                    interrupted: parent,
                    preemptor: child,
                },
                StackNestingEdge {
                    interrupted: child,
                    preemptor: parent,
                },
            ]),
        };
        let composed = compose_entry_stack_epochs(&relation, [&parent_input, &child_input])
            .expect("finite epoch nesting");

        assert_eq!(
            composed
                .demand(parent)
                .expect("parent demand")
                .domain(StackDomain::Dedicated { class: 4 }),
            Some(DomainStackDemand {
                bytes: 112,
                alignment: 16,
            })
        );
        assert_eq!(
            composed.domain(StackDomain::Interrupted),
            Some(DomainStackDemand {
                bytes: 32,
                alignment: 16,
            })
        );
        assert_eq!(
            composed.domain(StackDomain::Dedicated { class: 4 }),
            Some(DomainStackDemand {
                bytes: 112,
                alignment: 16,
            })
        );
    }

    #[test]
    fn input_validation_and_missing_nesting_endpoints_fail_closed() {
        let root = id(20, ExternalRootId::from_normalized_identity);
        let mut input = EpochStackCompositionInput {
            root,
            provider: id(21, RootProviderId::from_normalized_identity),
            realization: realization(vec![context(
                1,
                vec![epoch(
                    EntryStackStage::Body,
                    StackDomainRef::Interrupted,
                    Vec::new(),
                    Preemption::Masked,
                )],
            )]),
            body_wcsu_bytes: 8,
            body_wcsu_alignment: 8,
        };
        let relation = StackNestingRelation {
            identity: id(22, NestingRelationId::from_normalized_identity),
            edges: BTreeSet::new(),
        };
        input.body_wcsu_alignment = 3;
        let error = compose_entry_stack_epochs(&relation, [&input])
            .expect_err("malformed body alignment must reject");
        assert!(error.0.contains("nonzero power of two"));

        input.body_wcsu_alignment = 8;
        let missing = id(23, ExternalRootId::from_normalized_identity);
        let error = compose_entry_stack_epochs(
            &StackNestingRelation {
                identity: relation.identity,
                edges: BTreeSet::from([StackNestingEdge {
                    interrupted: root,
                    preemptor: missing,
                }]),
            },
            [&input],
        )
        .expect_err("missing nested root must reject");
        assert!(error.0.contains("missing preemptor"));
    }
}
