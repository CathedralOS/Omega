//! Entry and adapter stack realization evidence and its binding.

use crate::executable_installation::{
    ArtifactId, InstalledCode, InstalledCodeContext, InstalledCodeId,
};
use crate::external_roots::stack_and_fuel::epoch_stack_demand::x86_64_hardware_entries::{
    body_domain_closure, validate_bound_entry_stack_realization,
};
use crate::external_roots::stack_and_fuel::epoch_stack_demand::{
    AdmittedOpaqueArrivalContextSet, EpochStackCompositionInput,
};
use crate::external_roots::{
    ExternalRootDiagnostic, ExternalRootId, ProviderStackSummary, RootProviderId,
    StackLocalEvidence, StackValidationReceiptId, X86_64GateProfileValidationReceiptId,
};
use abstract_operations_to_target_operations::calling_conventions::{
    ArrivalContextId, ArrivalContextRealization, EntryStackEpoch, EntryStackRealization,
    EntryStackStage, StackDomainRef, StackOccupancy, ValidatedBoundaryEntryPlan,
    ValidatedEntryStackDomainClosure, ValidatedEntryStackRealization,
    validate_entry_stack_realization,
};
use target_operations_to_selected_instructions::isa_x86_64::{
    X86_64SemanticUnitWrapperEncodingRequest, X86_64SemanticUnitWrapperResolution,
    canonical_x86_64_semantic_unit_wrapper_encoding_request,
    encode_x86_64_semantic_unit_wrapper_template, validate_x86_64_resolved_semantic_unit_wrapper,
};
use terminal_psi::layout_plans::EntryStubId;

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
    pub(crate) request: X86_64SemanticUnitWrapperEncodingRequest,
    pub(crate) resolved_bytes: Vec<u8>,
    pub(crate) resolution: X86_64SemanticUnitWrapperResolution,
    pub(crate) non_authoritative_report_fingerprint: u64,
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
    pub(crate) root: ExternalRootId,
    pub(crate) provider: RootProviderId,
    pub(crate) architecture: target::Architecture,
    pub(crate) installed_code: InstalledCodeId,
    pub(crate) installed_code_context: InstalledCodeContext,
    pub(crate) artifact: ArtifactId,
    pub(crate) entry: EntryStubId,
    pub(crate) boundary_contract_report_fingerprint: u64,
    pub(crate) boundary_contract_commitment: [u8; 32],
    pub(crate) body_domains: ValidatedEntryStackDomainClosure,
    pub(crate) realization: ValidatedEntryStackRealization,
    pub(crate) arrival_origin: ArrivalStackRealizationOrigin,
    pub(crate) adapter_origin: AdapterStackRealizationOrigin,
    pub(crate) target_rule_report_fingerprint: Option<u64>,
    pub(crate) target_installation_validation_receipt: Option<X86_64GateProfileValidationReceiptId>,
    pub(crate) generated_adapter: Option<GeneratedProgramStorageAdapterStackEvidence>,
    pub(crate) validation_receipt: Option<StackValidationReceiptId>,
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

    pub const fn architecture(&self) -> target::Architecture {
        self.architecture
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.installed_code
    }

    /// The exact installed-code occurrence context retained at binding.
    ///
    /// Consumers rejoining this evidence against retained runtime provision
    /// compare the complete context: the compact installed-code identity is
    /// provider-minted and cannot distinguish two placements issued the same
    /// identity.
    pub const fn installed_code_context(&self) -> &InstalledCodeContext {
        &self.installed_code_context
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

    pub(crate) fn matches_installed_code_entry(
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
    pub(crate) pure: EpochStackCompositionInput,
    pub(crate) body_evidence: StackLocalEvidence,
    pub(crate) realization_evidence: EntryStackRealizationEvidence,
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

/// Exact generated-wrapper contribution to the live-adapter term of the UEFI
/// same-stack inequality. The byte count is derived from the independently
/// replayed wrapper request and all retained entry epochs; callers cannot
/// supply it as a numeric assertion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedProgramStorageAdapterLiveFrameDemand {
    evidence: EntryStackRealizationEvidence,
    bytes: u64,
    alignment: u64,
}

impl GeneratedProgramStorageAdapterLiveFrameDemand {
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn alignment(&self) -> u64 {
        self.alignment
    }

    pub const fn semantic_boundary_commitment(&self) -> [u8; 32] {
        self.evidence.boundary_contract_commitment
    }

    pub const fn installed_code(&self) -> InstalledCodeId {
        self.evidence.installed_code
    }

    pub const fn entry(&self) -> EntryStubId {
        self.evidence.entry
    }

    #[cfg(test)]
    pub(crate) fn with_semantic_boundary_commitment_for_test(
        mut self,
        commitment: [u8; 32],
    ) -> Self {
        self.evidence.boundary_contract_commitment = commitment;
        self
    }
}

#[cfg(test)]
impl BoundEpochStackCompositionInput {
    pub(crate) fn without_generated_adapter_origin_for_test(mut self) -> Self {
        self.realization_evidence.adapter_origin = AdapterStackRealizationOrigin::None;
        self
    }

    pub(crate) fn with_first_adapter_epoch_bytes_for_test(mut self, bytes: u64) -> Self {
        let mut realization = self.pure.realization.realization().clone();
        realization.contexts[0].epochs[0].occupancy_by_domain[0].bytes = bytes;
        let realization = validate_entry_stack_realization(realization).unwrap();
        self.pure.realization = realization.clone();
        self.realization_evidence.realization = realization;
        self
    }
}

/// Reconstruct the canonical generated wrapper and derive its live frame peak
/// from every retained epoch. This is deliberately narrower than general epoch
/// composition: only the receiver-free x86-64 ProgramStorage adapter shape is
/// admitted.
pub fn derive_generated_program_storage_adapter_live_frame_demand(
    input: &BoundEpochStackCompositionInput,
) -> Result<GeneratedProgramStorageAdapterLiveFrameDemand, ExternalRootDiagnostic> {
    let evidence = input.realization_evidence();
    let Some(generated) = evidence.generated_adapter() else {
        return Err(ExternalRootDiagnostic(
            "UEFI live adapter-frame demand lacks generated ProgramStorage wrapper evidence".into(),
        ));
    };
    if evidence.arrival_origin() != ArrivalStackRealizationOrigin::NoHardwareArrival
        || evidence.adapter_origin()
            != AdapterStackRealizationOrigin::GeneratedProgramStorageSemanticWrapper
        || evidence.architecture() != target::Architecture::X86_64
        || evidence.validation_receipt().is_some()
        || evidence.target_rule_report_fingerprint().is_some()
        || evidence.target_installation_validation_receipt().is_some()
        || input.pure().realization != *evidence.realization()
    {
        return Err(ExternalRootDiagnostic(
            "UEFI live adapter-frame demand has the wrong realization origin or custody".into(),
        ));
    }

    let request = generated.request();
    if request
        != canonical_x86_64_semantic_unit_wrapper_encoding_request(target::NativeTarget::uefi_x64())
    {
        return Err(ExternalRootDiagnostic(
            "UEFI live adapter-frame demand is not the canonical x86-64 wrapper request".into(),
        ));
    }
    let template = encode_x86_64_semantic_unit_wrapper_template(request).map_err(|_| {
        ExternalRootDiagnostic(
            "UEFI live adapter-frame demand could not reconstruct its wrapper template".into(),
        )
    })?;
    let resolution = generated.resolution();
    let replayed = validate_x86_64_resolved_semantic_unit_wrapper(
        &template,
        resolution.source,
        resolution.wrapper_section_offset,
        resolution.continuation_section_offset,
        generated.resolved_bytes(),
    )
    .map_err(|_| {
        ExternalRootDiagnostic(
            "UEFI live adapter-frame demand failed exact wrapper-byte replay".into(),
        )
    })?;
    if replayed.resolution() != resolution || replayed.bytes() != generated.resolved_bytes() {
        return Err(ExternalRootDiagnostic(
            "UEFI live adapter-frame demand changed wrapper call custody during replay".into(),
        ));
    }

    let bytes = u64::from(request.outgoing_frame_byte_count);
    let alignment = u64::from(request.pre_call_stack_alignment);
    let realization = evidence.realization().realization();
    if realization.contexts.is_empty() {
        return Err(ExternalRootDiagnostic(
            "UEFI live adapter-frame demand has no retained arrival context".into(),
        ));
    }
    for context in &realization.contexts {
        let expected_domain = evidence
            .body_domains
            .contexts()
            .iter()
            .find(|candidate| candidate.context == context.context)
            .map(|candidate| candidate.domain);
        if context.epochs.len() != 3
            || context.epochs.iter().map(|epoch| epoch.stage).ne([
                EntryStackStage::Enter,
                EntryStackStage::Body,
                EntryStackStage::Exit,
            ])
            || context.epochs.iter().any(|epoch| {
                expected_domain != Some(epoch.active_domain)
                    || epoch.occupancy_by_domain.as_slice()
                        != [StackOccupancy {
                            domain: epoch.active_domain,
                            bytes,
                            alignment,
                        }]
            })
        {
            return Err(ExternalRootDiagnostic(
                "UEFI live adapter-frame demand drifted from its exact Enter/Body/Exit occupancy"
                    .into(),
            ));
        }
    }
    Ok(GeneratedProgramStorageAdapterLiveFrameDemand {
        evidence: evidence.clone(),
        bytes,
        alignment,
    })
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
