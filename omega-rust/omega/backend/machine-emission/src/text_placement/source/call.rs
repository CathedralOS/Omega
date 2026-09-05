use super::super::TextPlacementError;
use super::fixup;
use isa_x86_64::{
    ValidatedX86_64SelectedStructuralUnitCallTemplate,
    validate_x86_64_selected_structural_unit_call_template,
};
use machine_code::SelectedStructuralUnitCallEncodingRow;
use machine_code::{
    ResolvedStructuralUnitCallLayout, StructuralUnitCallFragmentSpan,
    WholeFunctionStructuralUnitCallEvidence,
};
use register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog};
use selected_instructions::SelectedStructuralUnitCallInstruction;
use selected_instructions::StructuralUnitCallMachineEffects;
use semantic_vocabulary::MachineId;
use target::NativeTarget;
#[allow(clippy::too_many_arguments)]
pub(in crate::text_placement) fn validate(
    caller: MachineId,
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
) -> Result<ValidatedX86_64SelectedStructuralUnitCallTemplate, TextPlacementError> {
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
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    let template = validate_x86_64_selected_structural_unit_call_template(
        target,
        physical,
        constraints,
        selected_call,
        effect_call.declaration,
        &fragment_call.bytes,
    )
    .map_err(|error| TextPlacementError::StructuralUnitCallTemplate(caller, error))?;
    if template.bytes() != fragment_call.bytes
        || template.footprint() != encoded_call.footprint.as_ref()
        || !fixup::matches_target(fragment_call, template.fixup())?
    {
        return Err(TextPlacementError::SourceShapeMismatch);
    }
    Ok(template)
}
