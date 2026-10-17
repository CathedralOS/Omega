//! Rejoining admitted provider executions and checking the custody of
//! their port effects, reads, writes and settlement receipts.

use crate::OptimizedBoundaryOccurrence;
use crate::physical::derivation::evidence::{ranges_overlap, span};
pub(crate) use crate::physical::native_byte_span;
use crate::{NativeProviderExecution, NativeSelectedProviderPlan};
use installation_evidence::ProviderExecutionEvidence;
use machine_code::PortEffectRecord;
use object_file::SectionKind;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use std::collections::BTreeSet;
use target::{Architecture, NativeTarget};
use target_operations::{ProviderExecutionBinding, ProviderPlanReportIdentity};
use terminal_psi::OperationKind;

/// Rejoin a selected family alias to this artifact before using its provider
/// requirement for a physical ledger lookup. The boundary occurrence and its
/// head contract remain unchanged; only the provider-row address is transported.
pub(crate) fn validate_family_provider_custody(
    terminal: terminal_psi::TerminalPsiIdentity,
    module: &terminal_psi::TerminalModule,
    target: NativeTarget,
    boundary: semantic_vocabulary::BoundaryMachineId,
    identity: &str,
    plan: &NativeSelectedProviderPlan,
) -> Result<(), &'static str> {
    plan.validate_external_requirement_applications()?;
    let Some(row) = plan.family_realization_rows().iter().find(|row| {
        plan.family_realization_identities().text(row.head_identity) == Some(identity)
            || plan
                .family_realization_identities()
                .text(row.member_identity)
                == Some(identity)
    }) else {
        return Ok(());
    };
    if row.terminal != terminal
        || row.provider_plan_report_identity != plan.report_identity()
        || row.provider_plan_digest.as_bytes() != plan.plan_digest().as_bytes()
        || !target::TargetProfile::from_canonical_target_name(&row.target)
            .is_ok_and(|profile| profile.native_realization() == Some(target))
        || !((row.head == boundary
            && plan.family_realization_identities().text(row.head_identity) == Some(identity))
            || (row.member == boundary
                && plan
                    .family_realization_identities()
                    .text(row.member_identity)
                    == Some(identity)))
    {
        return Err("physical family provider changed its artifact, target, or selected plan");
    }
    for (boundary, identity) in [
        (
            row.head,
            plan.family_realization_identities().text(row.head_identity),
        ),
        (
            row.member,
            plan.family_realization_identities()
                .text(row.member_identity),
        ),
    ] {
        if !terminal_psi::find_boundary_machine(&module.boundary_machines, boundary)
            .is_some_and(|declaration| module.identities.text(declaration.identity) == identity)
        {
            return Err("physical family provider changed its exact callable declaration");
        }
    }
    let family = module
        .target_machine_families
        .iter()
        .find(|family| family.canonical == terminal_psi::TargetMachineCallable::Boundary(row.head))
        .ok_or("physical family provider has no canonical family")?;
    if family.head_contract == [0; 32]
        || !family.members.iter().any(|member| {
            member.target == row.target
                && member.callable == terminal_psi::TargetMachineCallable::Boundary(row.member)
                && member.supply != terminal_psi::TargetMachineSupply::CheckedBody
        })
    {
        return Err("physical family provider changed its selected member or head contract");
    }
    Ok(())
}

/// Rejoin the one retained provider execution an installed settlement names.
/// The record must identify the selected plan and match exactly one retained
/// execution on every compact coordinate.
pub(crate) fn rejoin_admitted_provider_execution(
    requirement_identity: &str,
    selected_plan: &NativeSelectedProviderPlan,
    provider_executions: &[NativeProviderExecution],
    execution_record: machine_code::ProviderExecutionRecord,
) -> Result<ProviderExecutionBinding, &'static str> {
    if execution_record.provider_plan_report_identity != selected_plan.report_identity() {
        return Err("installed D41 settlement names the wrong selected provider plan");
    }
    let selected_requirement = selected_plan.selected_requirement_identity(requirement_identity);
    let matching_executions = provider_executions
        .iter()
        .filter(|execution| {
            execution.requirement_identity() == selected_requirement
                && execution.provider_plan_report_identity()
                    == execution_record.provider_plan_report_identity
                && execution.provider_execution_report_identity()
                    == execution_record.provider_execution_report_identity
                && execution.provider_execution_report_fingerprint()
                    == execution_record.provider_execution_report_fingerprint
                && execution.normalized_root_report_identity()
                    == execution_record.normalized_root_report_identity
                && execution.boundary_contract_report_fingerprint()
                    == execution_record.boundary_contract_report_fingerprint
        })
        .count();
    if matching_executions != 1 {
        return Err("installed D41 settlement cannot rejoin one retained provider execution");
    }
    let plan_report_identity =
        ProviderPlanReportIdentity::new(execution_record.provider_plan_report_identity)
            .ok_or("installed D41 settlement has a zero provider-plan report identity")?;
    ProviderExecutionBinding::from_execution_record(
        plan_report_identity,
        execution_record.provider_execution_report_identity,
        execution_record.provider_execution_report_fingerprint,
        execution_record.normalized_root_report_identity,
        execution_record.boundary_contract_report_fingerprint,
    )
    .ok_or("installed D41 settlement has an invalid provider execution")
}

/// Join one `MetadataOnlyPort` settlement to its exact privileged port
/// effect. The effect operation must be the immediately preceding emitted
/// operation whose interval ends where the settlement begins, must still
/// name one exact Terminal port write, and must reproduce its target bytes
/// across machine, object, and final-image custody.
#[allow(clippy::too_many_arguments)]
pub(crate) fn metadata_port_effect_custody(
    module: &terminal_psi::TerminalModule,
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    occurrence: &OptimizedBoundaryOccurrence,
    settlement: &machine_code::BoundarySettlementRecord,
    realization: &target_operations::MetadataOnlyPortRealization,
    target: NativeTarget,
    function: &image_emission::ObjectFunction,
    operation: &terminal_psi::Operation,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    consumed_port_effects: &mut BTreeSet<usize>,
) -> Result<PortEffectRecord, &'static str> {
    if target.architecture != Architecture::X86_64
        || !settlement.scalar_arguments.is_empty()
        || !settlement.runtime_scalar_arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || !settlement.native_result.is_unit()
        || settlement.byte_count != 0
        || !matches!(operation.result, terminal_psi::OperationResult::Unit)
        || !declaration.result.is_unit()
    {
        return Err("metadata port D41 settlement custody is incomplete or substituted");
    }
    let matching_effects = object
        .port_effects()
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            let effect = &candidate.effect;
            candidate.machine == occurrence.machine()
                && effect.psi_operation == realization.effect_operation
                && effect.service == realization.service
                && effect.port == realization.port
                && effect.value == realization.value
                && effect.operation_ordinal.checked_add(1) == Some(settlement.operation_ordinal)
                && effect.code_offset.checked_add(effect.byte_count) == Some(settlement.code_offset)
        })
        .collect::<Vec<_>>();
    let [(effect_index, object_effect)] = matching_effects.as_slice() else {
        return Err("metadata port D41 settlement does not rejoin one privileged port effect");
    };
    let effect = &object_effect.effect;
    let matching_effect_operations = module
        .machines
        .iter()
        .filter(|machine| machine.id == occurrence.machine())
        .flat_map(|machine| machine.operations())
        .filter(|operation| operation.id == effect.psi_operation)
        .collect::<Vec<_>>();
    let [effect_operation] = matching_effect_operations.as_slice() else {
        return Err("metadata port D41 effect does not rejoin one Terminal operation");
    };
    if !matches!(
        effect_operation.kind,
        OperationKind::PortWrite { service, port, value }
            if service == effect.service && port == effect.port && value == effect.value
    ) {
        return Err("metadata port D41 effect changed its semantic port write");
    }
    if !function
        .provenance
        .operations
        .contains(&effect.psi_operation)
        || effect.byte_count != x86_encoding::IMMEDIATE_PORT_WRITE_WIDTH
    {
        return Err("metadata port D41 effect left its function provenance or width");
    }
    let expected_object_offset = function
        .text_offset
        .checked_add(effect.code_offset)
        .ok_or("metadata port D41 effect object span overflow")?;
    if object_effect.text_offset != expected_object_offset {
        return Err("metadata port D41 effect object span is detached");
    }
    let expected = x86_encoding::encode_immediate_port_write(effect.port, effect.value);
    let machine_span = native_byte_span(effect.code_offset, effect.byte_count);
    let object_span = native_byte_span(object_effect.text_offset, effect.byte_count);
    let machine_bytes = span(function.bytes(object), machine_span)?;
    let object_bytes = span(object.text_bytes(), object_span)?;
    let final_image_bytes = span(&image.final_text_bytes, object_span)?;
    if machine_bytes != expected.as_slice()
        || machine_bytes != object_bytes
        || object_bytes != final_image_bytes
    {
        return Err("metadata port D41 effect bytes changed across physical custody");
    }
    let object_end = object_effect
        .text_offset
        .checked_add(effect.byte_count)
        .ok_or("metadata port D41 effect relocation span overflow")?;
    if object.relocations().records().any(|(_, relocation)| {
        relocation.section == SectionKind::Text
            && ranges_overlap(
                object_effect.text_offset,
                object_end,
                relocation.offset,
                relocation.offset.saturating_add(relocation.byte_width),
            )
    }) {
        return Err("metadata port D41 effect unexpectedly contains a relocation");
    }
    consumed_port_effects.insert(*effect_index);
    Ok(effect.clone())
}

/// Replay the exact direct port-read custody: one 16-byte x86-64 `in al, dx`
/// sequence, one `u8` result in `RAX`, and the exact `0xc3` return edge that
/// returns the read value.
#[allow(clippy::too_many_arguments)]
pub(crate) fn direct_port_read_custody(
    object: &image_emission::ObjectArtifact,
    image: &image::EmittedImageOutput,
    occurrence: &OptimizedBoundaryOccurrence,
    settlement: &machine_code::BoundarySettlementRecord,
    realization: &target_operations::DirectPortReadU8Realization,
    target: NativeTarget,
    function: &image_emission::ObjectFunction,
    operation: &terminal_psi::Operation,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<(), &'static str> {
    let u8_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
    let Some(result) = settlement.native_result.scalar() else {
        return Err("direct port-read D41 settlement requires one scalar result");
    };
    if target.architecture != Architecture::X86_64
        || !settlement.scalar_arguments.is_empty()
        || !settlement.runtime_scalar_arguments.is_empty()
        || !settlement.byte_sequence_arguments.is_empty()
        || settlement.byte_count != x86_encoding::IMMEDIATE_PORT_READ_U8_WIDTH
        || result.scalar_type != u8_type
        || result.placement.shape != calling_conventions::ValueShape::integer(1, 1)
        || result.placement.locations.as_slice()
            != [calling_conventions::ValueLocation::Register {
                register: calling_conventions::MachineRegister::X86Rax,
                value_byte_offset: 0,
                byte_size: 1,
            }]
        || function.unit_stack.is_some()
        || function.scalar_stack.is_none()
    {
        return Err("direct port-read D41 settlement custody is incomplete or substituted");
    }
    if !settlement.arguments.iter().all(|argument| {
        argument.path.is_empty()
            && function
                .scalar_structural_parameter(argument.place)
                .is_some()
    }) {
        return Err("direct port-read D41 settlement changed a structural argument place");
    }
    let result_matches = matches!(
        &operation.result,
        terminal_psi::OperationResult::Scalar(value)
            if value.id == result.value && value.scalar_type == u8_type
    ) && matches!(
        declaration.result,
        terminal_psi::BoundaryMachineResult::Scalar(scalar) if scalar == u8_type
    );
    if !result_matches {
        return Err("direct port-read D41 settlement changed its semantic scalar result");
    }
    let Some(return_ordinal) = settlement.operation_ordinal.checked_add(1) else {
        return Err("direct port-read D41 return ordinal overflow");
    };
    let Some(return_offset) = settlement.code_offset.checked_add(settlement.byte_count) else {
        return Err("direct port-read D41 return offset overflow");
    };
    let matching_returns = object
        .semantic_code_attribution()
        .iter()
        .filter(|attribution| {
            attribution.machine == occurrence.machine()
                && attribution.attribution.site
                    == machine_code::SemanticCodeSite::Edge(result.return_edge)
                && attribution.attribution.operation_ordinal == return_ordinal
                && attribution.attribution.code_offset == return_offset
                && attribution.attribution.byte_count == 1
        })
        .collect::<Vec<_>>();
    let [return_attribution] = matching_returns.as_slice() else {
        return Err("direct port-read D41 settlement does not rejoin one return edge");
    };
    let expected_return_object_offset = function
        .text_offset
        .checked_add(return_offset)
        .ok_or("direct port-read D41 return object span overflow")?;
    let expected = x86_encoding::encode_immediate_port_read_u8(realization.port);
    let machine_span = native_byte_span(settlement.code_offset, settlement.byte_count);
    if span(function.bytes(object), machine_span)? != expected.as_slice()
        || return_attribution.text_offset != expected_return_object_offset
        || function.bytes(object).get(return_offset) != Some(&0xc3)
        || object.text_bytes().get(return_attribution.text_offset) != Some(&0xc3)
        || image.final_text_bytes.get(return_attribution.text_offset) != Some(&0xc3)
    {
        return Err("direct port-read D41 custody changed across physical custody");
    }
    Ok(())
}

/// Replay the exact Linux `write_line` byte custody: one borrowed-view
/// byte-sequence structural argument, the exact target encoder output, and
/// the code/data intervals the settlement retains.
pub(crate) fn linux_write_line_custody_is_exact(
    target: NativeTarget,
    settlement: &machine_code::BoundarySettlementRecord,
    function_bytes: &[u8],
) -> bool {
    image_emission::linux_write_line_custody_is_exact(target, settlement, Some(function_bytes))
}

/// Replay the object validator's completion-custody responsibility after the
/// verified module has been discarded. The caller already constrained the
/// execution/realization pair, so this mirrors the argument-path,
/// receipt-index, receipt-custody, and provider-custody reconstruction
/// checks exactly.
pub(crate) fn settlement_completion_custody_is_exact(
    settlement: &machine_code::BoundarySettlementRecord,
    identities: &terminal_psi::TerminalIdentityTable,
) -> bool {
    if settlement.arguments.iter().any(|argument| {
        argument.path.iter().any(
            |segment| matches!(segment, terminal_psi::StructuralPathSegment::Field(identity) if *identity == terminal_psi::TerminalIdentityIndex::ABSENT),
        )
    }) || settlement.completion_receipts.iter().any(|receipt| {
        usize::try_from(receipt.argument_index)
            .map_or(true, |index| index >= settlement.arguments.len())
    }) {
        return false;
    }
    if !image_emission::completion_receipts_have_exact_custody(
        identities,
        &settlement.arguments,
        &settlement.completion_claim_sources,
        &settlement.completion_receipts,
    ) {
        return false;
    }
    let Some(expected) = machine_code::derive_completion_provider_custody(
        settlement.execution,
        &settlement.completion_claim_sources,
        &settlement.completion_receipts,
    ) else {
        return false;
    };
    expected == settlement.completion_provider_custody
}
