use omega_isa_x86_64::{
    ValidatedX86_64SelectedStructuralUnitCallTemplate,
    encode_x86_64_selected_structural_unit_call_template,
};
use omega_physical_instructions::PostAllocationStructuralUnitFunction;
use omega_register_model::{ValidatedPhysicalRegisterModel, ValidatedRegisterConstraintCatalog};
use omega_selected_instructions::{
    SelectedStructuralUnitCallInstruction, SelectedStructuralUnitFunction,
};
use omega_selected_instructions::{
    StructuralUnitCallMachineEffects, StructuralUnitFunctionMachineEffects,
};
use omega_target::NativeTarget;

use super::{
    OptimizedSelectedFormEncodingError, SelectedFormEncodingState, SelectedFormMachineDisposition,
    SelectedStructuralUnitCallEncodingRow, SelectedStructuralUnitFunctionEncoding,
    row_encoding::encode_row,
};

pub(super) fn encode_structural_function(
    target: NativeTarget,
    selected_plan: &omega_selected_instructions::SelectedInstructionPlan,
    selected: &SelectedStructuralUnitFunction,
    machine: &PostAllocationStructuralUnitFunction,
    effects: &StructuralUnitFunctionMachineEffects,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralUnitFunctionEncoding, OptimizedSelectedFormEncodingError> {
    if selected.machine != machine.machine
        || selected.machine != effects.machine
        || selected.entry_block != machine.block
        || selected.entry_block != effects.block
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
    }
    if machine.call != effects.call
        || machine.return_effect != effects.return_effect
        || machine.return_ownership != effects.return_ownership
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralFunctionRosterMismatch);
    }
    let call = match (&selected.call, &machine.call) {
        (None, None) => None,
        (Some(selected_call), Some(machine_call)) => Some(encode_structural_call(
            target,
            selected_plan,
            selected_call,
            machine_call,
            physical,
            constraints,
        )?),
        (Some(selected_call), None) => {
            return Err(
                OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(selected_call.id),
            );
        }
        (None, Some(machine_call)) => {
            return Err(
                OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(
                    machine_call.instruction,
                ),
            );
        }
    };
    let selected_return = &selected.terminator.instruction;
    if selected_return.id != machine.return_instruction.instruction
        || selected_return.id != effects.return_instruction.instruction
        || selected_return.kind != effects.return_instruction.kind
        || selected_return.provenance != machine.return_provenance
        || selected_return.provenance != effects.return_instruction.provenance
        || selected.terminator.effect != machine.return_effect
        || selected.terminator.ownership != machine.return_ownership
        || selected.terminator.effect != effects.return_effect
        || selected.terminator.ownership != effects.return_ownership
        || !effects
            .return_instruction
            .alternatives
            .contains(&machine.return_instruction.alternative)
    {
        return Err(
            OptimizedSelectedFormEncodingError::StructuralReturnRosterMismatch(selected_return.id),
        );
    }
    let return_instruction = encode_row(
        target,
        selected_return,
        &machine.return_instruction,
        physical,
        SelectedFormMachineDisposition::RetainedV1,
        None,
    )?;
    if !matches!(
        return_instruction.state,
        SelectedFormEncodingState::Encoded { ref bytes, .. } if bytes.as_slice() == [0xc3]
    ) {
        return Err(
            OptimizedSelectedFormEncodingError::StructuralReturnRosterMismatch(selected_return.id),
        );
    }
    Ok(SelectedStructuralUnitFunctionEncoding {
        machine: selected.machine,
        block: selected.entry_block,
        call,
        return_instruction,
    })
}

fn encode_structural_call(
    target: NativeTarget,
    selected_plan: &omega_selected_instructions::SelectedInstructionPlan,
    selected: &SelectedStructuralUnitCallInstruction,
    machine: &StructuralUnitCallMachineEffects,
    physical: &ValidatedPhysicalRegisterModel,
    constraints: &ValidatedRegisterConstraintCatalog,
) -> Result<SelectedStructuralUnitCallEncodingRow, OptimizedSelectedFormEncodingError> {
    if machine.instruction != selected.id
        || machine.operation != selected.operation
        || machine.callee != selected.callee
        || machine.constraint != selected.constraint
        || machine.unit_uses != selected.implicit_uses
        || machine.unit_defs != selected.implicit_defs
        || machine.unit_clobbers != selected.clobbers
        || machine.layout != selected.layout
        || machine.effect != selected.effect
        || machine.ownership != selected.ownership
        || machine.claim_transfers != selected.claim_transfers
        || machine.provenance != selected.provenance
        || selected_plan
            .structural_unit_functions
            .iter()
            .filter(|function| function.machine == selected.callee)
            .count()
            != 1
    {
        return Err(OptimizedSelectedFormEncodingError::StructuralCallRosterMismatch(selected.id));
    }
    let encoded = encode_x86_64_selected_structural_unit_call_template(
        target,
        physical,
        constraints,
        selected,
        machine.declaration,
    )
    .map_err(OptimizedSelectedFormEncodingError::X86_64Structural)?;
    Ok(structural_call_encoding_row(selected, encoded))
}

fn structural_call_encoding_row(
    selected: &SelectedStructuralUnitCallInstruction,
    encoded: ValidatedX86_64SelectedStructuralUnitCallTemplate,
) -> SelectedStructuralUnitCallEncodingRow {
    SelectedStructuralUnitCallEncodingRow {
        instruction: selected.id,
        operation: selected.operation,
        callee: selected.callee,
        bytes: encoded.bytes().to_vec(),
        footprint: Box::new(encoded.footprint().clone()),
        fixup: encoded.fixup(),
    }
}
