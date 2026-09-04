use std::collections::BTreeMap;

use omega_isa_x86_64::{
    resolve_x86_64_structural_unit_internal_call,
    validate_x86_64_selected_structural_unit_call_template,
};
use omega_machine_code::StructuralUnitCallFragmentSpan;
use omega_machine_optimizer::StructuralUnitCallMachineEffects;
use omega_object_file::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
    PlacedInternalMachineCallResolution,
};
use omega_register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog};
use omega_selected_instructions::{SelectedBlockId, SelectedStructuralUnitCallInstruction};
use omega_target::NativeTarget;
use psi_core::MachineId;

use crate::{
    ResolvedStructuralUnitCallLayout, SelectedStructuralUnitCallEncodingRow,
    WholeFunctionStructuralUnitCallEvidence,
};

use super::super::super::RelocationFreeTextSectionPlacementError;
use super::super::conversion::u64_to_usize;
use super::fixup;

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve(
    caller: MachineId,
    block: SelectedBlockId,
    function_section_offset: u64,
    function_offsets: &BTreeMap<MachineId, u64>,
    function_bytes: &mut [u8],
    fragment_call: &StructuralUnitCallFragmentSpan,
    selected_call: &SelectedStructuralUnitCallInstruction,
    machine_call: &StructuralUnitCallMachineEffects,
    effect_call: &StructuralUnitCallMachineEffects,
    encoded_call: &SelectedStructuralUnitCallEncodingRow,
    layout_call: &ResolvedStructuralUnitCallLayout,
    exit_call: &WholeFunctionStructuralUnitCallEvidence,
    target: NativeTarget,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<PlacedInternalMachineCallResolution, RelocationFreeTextSectionPlacementError> {
    if fragment_call.instruction != selected_call.id
        || fragment_call.instruction != machine_call.instruction
        || fragment_call.instruction != effect_call.instruction
        || fragment_call.instruction != encoded_call.instruction
        || fragment_call.instruction != layout_call.instruction
        || fragment_call.instruction != exit_call.instruction
        || fragment_call.operation != selected_call.operation
        || fragment_call.operation != machine_call.operation
        || fragment_call.operation != effect_call.operation
        || fragment_call.operation != encoded_call.operation
        || fragment_call.operation != layout_call.operation
        || fragment_call.operation != exit_call.operation
        || fragment_call.callee != selected_call.callee
        || fragment_call.callee != machine_call.callee
        || fragment_call.callee != effect_call.callee
        || fragment_call.callee != encoded_call.callee
        || fragment_call.callee != layout_call.callee
        || fragment_call.callee != exit_call.callee
        || fragment_call.provenance != selected_call.provenance
        || fragment_call.provenance != effect_call.provenance
        || fragment_call.offset != layout_call.offset
        || fragment_call.offset != exit_call.offset
        || encoded_call.footprint.as_ref() != layout_call.footprint.as_ref()
        || encoded_call.fixup != layout_call.fixup
        || encoded_call.fixup != exit_call.fixup
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    let template = validate_x86_64_selected_structural_unit_call_template(
        target,
        physical,
        constraints,
        selected_call,
        effect_call.declaration,
        &fragment_call.bytes,
    )
    .map_err(|error| {
        RelocationFreeTextSectionPlacementError::StructuralUnitCallTemplate(caller, error)
    })?;
    if template.bytes() != fragment_call.bytes
        || template.footprint() != encoded_call.footprint.as_ref()
        || !fixup::matches_target(fragment_call, template.fixup())?
    {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    let call_section_offset = function_section_offset
        .checked_add(fragment_call.offset)
        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    let callee_section_offset = *function_offsets.get(&fragment_call.callee).ok_or(
        RelocationFreeTextSectionPlacementError::MissingInternalMachineTarget(fragment_call.callee),
    )?;
    let resolved = resolve_x86_64_structural_unit_internal_call(
        &template,
        template.fixup(),
        call_section_offset,
        callee_section_offset,
    )
    .map_err(|error| {
        RelocationFreeTextSectionPlacementError::StructuralUnitCallResolution(caller, error)
    })?;
    let call_start = u64_to_usize(fragment_call.offset)?;
    let call_end = call_start
        .checked_add(resolved.bytes().len())
        .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?;
    if function_bytes.get(call_start..call_end) != Some(fragment_call.bytes.as_slice()) {
        return Err(RelocationFreeTextSectionPlacementError::SourceShapeMismatch);
    }
    function_bytes
        .get_mut(call_start..call_end)
        .ok_or(RelocationFreeTextSectionPlacementError::SourceShapeMismatch)?
        .copy_from_slice(resolved.bytes());

    let resolution = resolved.resolution();
    let neutral = fragment_call.fixup;
    Ok(PlacedInternalMachineCallResolution {
        kind:
            InternalMachineCallResolutionKind::X86Relative32FromNextInstructionToInternalMachineV1,
        state: InternalMachineCallResolutionState::ResolvedInSectionV1,
        caller,
        block,
        instruction: fragment_call.instruction,
        operation: fragment_call.operation,
        callee: fragment_call.callee,
        call_function_offset: fragment_call.offset,
        call_section_offset,
        call_byte_count: u64::try_from(resolved.bytes().len())
            .map_err(|_| RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
        opcode_function_offset: neutral.opcode_function_offset,
        opcode_section_offset: function_section_offset
            .checked_add(neutral.opcode_function_offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
        field_function_offset: neutral.patch_function_offset,
        field_section_offset: function_section_offset
            .checked_add(neutral.patch_function_offset)
            .ok_or(RelocationFreeTextSectionPlacementError::OffsetOverflow)?,
        next_instruction_function_offset: neutral.reference_function_offset,
        next_instruction_section_offset: resolution.next_instruction_section_offset,
        callee_section_offset: resolution.callee_section_offset,
        field_byte_width: neutral.patch_byte_width,
        addend: neutral.addend,
        displacement: resolution.displacement,
    })
}
