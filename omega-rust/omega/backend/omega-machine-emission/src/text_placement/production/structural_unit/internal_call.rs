use std::collections::BTreeMap;

use omega_isa_x86_64::resolve_x86_64_structural_unit_internal_call;
use omega_machine_code::StructuralUnitCallFragmentSpan;
use omega_machine_code::{
    InternalMachineCallResolutionKind, InternalMachineCallResolutionState,
    PlacedInternalMachineCallResolution,
};
use omega_machine_optimizer::StructuralUnitCallMachineEffects;
use omega_register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog};
use omega_selected_instructions::{SelectedBlockId, SelectedStructuralUnitCallInstruction};
use omega_target::NativeTarget;
use psi_core::MachineId;

use omega_machine_code::{
    ResolvedStructuralUnitCallLayout, WholeFunctionStructuralUnitCallEvidence,
};
use omega_post_allocation_machine_to_selected_form_encoding::SelectedStructuralUnitCallEncodingRow;

use super::super::super::TextPlacementError;
use super::super::conversion::u64_to_usize;

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
) -> Result<PlacedInternalMachineCallResolution, TextPlacementError> {
    let template = super::super::super::source::call::validate(
        caller,
        fragment_call,
        selected_call,
        machine_call,
        effect_call,
        encoded_call,
        layout_call,
        exit_call,
        target,
        physical,
        constraints,
    )?;
    let call_section_offset = function_section_offset
        .checked_add(fragment_call.offset)
        .ok_or(TextPlacementError::OffsetOverflow)?;
    let callee_section_offset = *function_offsets.get(&fragment_call.callee).ok_or(
        TextPlacementError::MissingInternalMachineTarget(fragment_call.callee),
    )?;
    let resolved = resolve_x86_64_structural_unit_internal_call(
        &template,
        template.fixup(),
        call_section_offset,
        callee_section_offset,
    )
    .map_err(|error| TextPlacementError::StructuralUnitCallResolution(caller, error))?;
    let call_start = u64_to_usize(fragment_call.offset)?;
    let call_end = call_start
        .checked_add(resolved.bytes().len())
        .ok_or(TextPlacementError::OffsetOverflow)?;
    if function_bytes.get(call_start..call_end) != Some(fragment_call.bytes.as_slice()) {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    function_bytes
        .get_mut(call_start..call_end)
        .ok_or(TextPlacementError::SourceShapeMismatch)?
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
            .map_err(|_| TextPlacementError::OffsetOverflow)?,
        opcode_function_offset: neutral.opcode_function_offset,
        opcode_section_offset: function_section_offset
            .checked_add(neutral.opcode_function_offset)
            .ok_or(TextPlacementError::OffsetOverflow)?,
        field_function_offset: neutral.patch_function_offset,
        field_section_offset: function_section_offset
            .checked_add(neutral.patch_function_offset)
            .ok_or(TextPlacementError::OffsetOverflow)?,
        next_instruction_function_offset: neutral.reference_function_offset,
        next_instruction_section_offset: resolution.next_instruction_section_offset,
        callee_section_offset: resolution.callee_section_offset,
        field_byte_width: neutral.patch_byte_width,
        addend: neutral.addend,
        displacement: resolution.displacement,
    })
}
