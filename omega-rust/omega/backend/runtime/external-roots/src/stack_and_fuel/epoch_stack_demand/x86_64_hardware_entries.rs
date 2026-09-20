//! Installed x86_64 hardware entry facts and bound entry realizations.

use crate::identities::Fnv1a;
use crate::stack_and_fuel::epoch_stack_demand::arrival_contexts::canonicalize_x86_64_installed_gate_tss_realization;
use crate::stack_and_fuel::epoch_stack_demand::{
    AdapterStackRealizationOrigin, ArrivalStackRealizationOrigin, BoundEpochStackCompositionInput,
    EntryStackRealizationEvidence, EpochStackCompositionInput,
    GeneratedProgramStorageAdapterStackEvidence, ValidatedX86_64InstalledGateProfileRoster,
    X86_64GeneratedProgramStorageAdapterEmission,
};
use crate::{
    ExternalRootDiagnostic, ProviderStackSummary, StackLocalEvidence,
    X86_64GateProfileValidationReceiptId,
};
use calling_conventions::{
    ArrivalContextRealization, ArrivalContextStackDomain, CallSignature, CallingPolicy,
    EntryControl, EntryStack, EntryStackEpoch, EntryStackRealization, EntryStackStage,
    InstalledEntryFactIdentity, MachineRegime, Preemption, StackOccupancy,
    ValidatedBoundaryEntryPlan, ValidatedEntryStackDomainClosure, ValidatedEntryStackRealization,
    ValidatedX86_64InstalledHardwareEntryFacts, ValueShape, X86_64HardwareStackSelection,
    X86_64InstalledArrivalContext, X86_64InstalledGateTssRealization,
    X86_64InstalledHardwareEntryFacts, X86_64TargetDerivedHardwareArrival,
    X86_64TargetProfileIdentity, derive_x86_64_hardware_arrival,
    evaluate_ordinary_boundary_entry_plan, validate_entry_stack_domain_closure,
    validate_entry_stack_realization, validate_x86_64_installed_hardware_entry_facts,
};
use executable_installation::{InstalledCode, InstalledCodeContext};
use isa_x86_64::{
    validate_x86_64_resolved_semantic_unit_wrapper, validate_x86_64_semantic_unit_wrapper_template,
};
use layout_plans::EntryStubId;
use std::collections::BTreeSet;

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
    if installed_code.architecture() != target::Architecture::X86_64
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
    let Some(body_matches_entry) = summary
        .local_evidence
        .terminal_body_matches_installed_entry(installed_code, entry)
    else {
        return Err(ExternalRootDiagnostic(
            "target-derived direct entry stack realization requires emitter-derived Terminal body evidence"
                .into(),
        ));
    };
    if installed_code.architecture() != target::Architecture::X86_64 {
        return Err(ExternalRootDiagnostic(
            "x86-64 target arrival evidence cannot bind a non-x86 installed artifact".into(),
        ));
    }
    if !body_matches_entry {
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
    if installed_code.architecture() != target::Architecture::X86_64 {
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

pub(crate) fn validate_bound_entry_stack_realization(
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
    if let Some(matches) = summary
        .local_evidence
        .terminal_body_matches_installed_entry(installed_code, entry)
        && !matches
    {
        return Err(ExternalRootDiagnostic(
            "terminal body WCSU and entry realization name different installed entries".into(),
        ));
    }
    Ok(())
}

pub(crate) fn body_domain_closure(
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
