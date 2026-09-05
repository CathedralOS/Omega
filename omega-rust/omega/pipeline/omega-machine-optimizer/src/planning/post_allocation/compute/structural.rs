//! Structural-Unit function custody and construction.

use omega_register_model::ValidatedPhysicalRegisterModel;
use omega_selected_instructions::{SelectedInstructionPlan, SelectedStructuralUnitFunction};
use omega_selected_instructions_to_register_homes::ValidatedRegisterHomes;

use crate::{
    PostAllocationMachineError, PostAllocationStructuralUnitFunction,
    StructuralUnitFunctionMachineEffects, ValidatedPreAllocationMachineEffects,
};

use super::instruction;

pub(super) fn build_functions(
    selected: &SelectedInstructionPlan,
    effects: &ValidatedPreAllocationMachineEffects,
    homes: &ValidatedRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<Vec<PostAllocationStructuralUnitFunction>, PostAllocationMachineError> {
    selected
        .structural_unit_functions
        .iter()
        .enumerate()
        .map(|(structural_index, function)| {
            build_function(
                selected.functions.len() + structural_index,
                function,
                unique_effect(effects, function.machine)?,
                unique_home(homes, function.machine)?,
                physical,
            )
        })
        .collect()
}

fn build_function(
    function_index: usize,
    selected: &SelectedStructuralUnitFunction,
    effects: &StructuralUnitFunctionMachineEffects,
    homes: &omega_selected_instructions_to_register_homes::FunctionRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PostAllocationStructuralUnitFunction, PostAllocationMachineError> {
    if effects.machine != selected.machine
        || effects.block != selected.entry_block
        || effects.return_effect != selected.terminator.effect
        || effects.return_ownership != selected.terminator.ownership
        || !call_matches(selected, effects)
    {
        return Err(PostAllocationMachineError::StructuralFunctionMismatch {
            machine: selected.machine,
        });
    }
    if homes.machine != selected.machine || !homes.assignments.is_empty() {
        return Err(PostAllocationMachineError::StructuralAllocationMismatch {
            machine: selected.machine,
        });
    }
    let return_instruction = instruction::build(
        function_index,
        &selected.terminator.instruction,
        &effects.return_instruction,
        homes,
        physical,
    )?;
    Ok(PostAllocationStructuralUnitFunction {
        machine: selected.machine,
        block: selected.entry_block,
        call: effects.call.clone(),
        return_instruction,
        return_provenance: selected.terminator.instruction.provenance.clone(),
        return_effect: selected.terminator.effect,
        return_ownership: selected.terminator.ownership.clone(),
    })
}

fn call_matches(
    selected: &SelectedStructuralUnitFunction,
    effects: &StructuralUnitFunctionMachineEffects,
) -> bool {
    match (&selected.call, &effects.call) {
        (None, None) => true,
        (Some(selected), Some(effects)) => {
            effects.instruction == selected.id
                && effects.operation == selected.operation
                && effects.callee == selected.callee
                && effects.constraint == selected.constraint
                && effects.unit_uses == selected.implicit_uses
                && effects.unit_defs == selected.implicit_defs
                && effects.unit_clobbers == selected.clobbers
                && effects.layout == selected.layout
                && effects.effect == selected.effect
                && effects.ownership == selected.ownership
                && effects.claim_transfers == selected.claim_transfers
                && effects.provenance == selected.provenance
                && effects.declaration.constraint == selected.constraint
        }
        _ => false,
    }
}

pub(super) fn unique_effect(
    effects: &ValidatedPreAllocationMachineEffects,
    machine: psi_core::MachineId,
) -> Result<&StructuralUnitFunctionMachineEffects, PostAllocationMachineError> {
    let matches = effects
        .plan()
        .structural_unit_functions
        .iter()
        .filter(|function| function.machine == machine)
        .collect::<Vec<_>>();
    let [function] = matches.as_slice() else {
        return Err(PostAllocationMachineError::StructuralFunctionMismatch { machine });
    };
    Ok(*function)
}

pub(super) fn unique_home(
    homes: &ValidatedRegisterHomes,
    machine: psi_core::MachineId,
) -> Result<
    &omega_selected_instructions_to_register_homes::FunctionRegisterHomes,
    PostAllocationMachineError,
> {
    let matches = homes
        .plan()
        .structural_unit_functions
        .iter()
        .filter(|function| function.machine == machine)
        .collect::<Vec<_>>();
    let [function] = matches.as_slice() else {
        return Err(PostAllocationMachineError::StructuralAllocationMismatch { machine });
    };
    Ok(*function)
}
