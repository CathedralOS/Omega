use std::collections::BTreeSet;

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::SelectedStructuralUnitFunction;
use selected_instructions_to_register_homes::{
    FunctionRegisterHomes, ValidatedAllocationLegality, ValidatedLiveRanges,
    ValidatedRegisterHomes, ValidatedSelectedAnalysis,
};
use semantic_vocabulary::MachineId;

use crate::PostAllocationMachineError;
use physical_instructions::{PostAllocationMachinePlan, PostAllocationStructuralUnitFunction};
use selected_instructions::StructuralUnitFunctionMachineEffects;
use selected_instructions_to_register_homes::ValidatedPreAllocationMachineEffects;

use super::instruction::reconstruct_instruction;

pub(super) fn validate_structural_functions<S: ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    homes: &ValidatedRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
    plan: &PostAllocationMachinePlan,
) -> Result<(), PostAllocationMachineError> {
    if plan.structural_unit_functions.len()
        != selected.selected_plan().structural_unit_functions.len()
        || plan.structural_unit_functions.len() != effects.plan().structural_unit_functions.len()
    {
        return Err(PostAllocationMachineError::StructuralAllocationMismatch {
            machine: selected.selected_plan().entry,
        });
    }
    let actual_machines = plan
        .structural_unit_functions
        .iter()
        .map(|function| function.machine)
        .collect::<BTreeSet<_>>();
    if actual_machines.len() != plan.structural_unit_functions.len() {
        return Err(PostAllocationMachineError::StructuralAllocationMismatch {
            machine: selected.selected_plan().entry,
        });
    }
    for (structural_index, selected_function) in selected
        .selected_plan()
        .structural_unit_functions
        .iter()
        .enumerate()
    {
        let effect_function = unique_structural_effect(effects, selected_function.machine)?;
        let home_function = unique_structural_home(homes, selected_function.machine)?;
        let expected = reconstruct_structural_function(
            selected.selected_plan().functions.len() + structural_index,
            selected_function,
            effect_function,
            home_function,
            physical,
        )?;
        let Some(actual) = plan
            .structural_unit_functions
            .get(structural_index)
            .filter(|function| function.machine == selected_function.machine)
        else {
            return Err(PostAllocationMachineError::StructuralFunctionMismatch {
                machine: selected_function.machine,
            });
        };
        if *actual != expected {
            return Err(PostAllocationMachineError::StructuralFunctionMismatch {
                machine: selected_function.machine,
            });
        }
    }
    Ok(())
}

fn reconstruct_structural_function(
    function_index: usize,
    selected: &SelectedStructuralUnitFunction,
    effects: &StructuralUnitFunctionMachineEffects,
    homes: &FunctionRegisterHomes,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<PostAllocationStructuralUnitFunction, PostAllocationMachineError> {
    if effects.machine != selected.machine
        || effects.block != selected.entry_block
        || effects.return_effect != selected.terminator.effect
        || effects.return_ownership != selected.terminator.ownership
        || !structural_call_matches(selected, effects)
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
    Ok(PostAllocationStructuralUnitFunction {
        machine: selected.machine,
        block: selected.entry_block,
        call: effects.call.clone(),
        return_instruction: reconstruct_instruction(
            function_index,
            &selected.terminator.instruction,
            &effects.return_instruction,
            homes,
            physical,
        )?,
        return_provenance: selected.terminator.instruction.provenance.clone(),
        return_effect: selected.terminator.effect,
        return_ownership: selected.terminator.ownership.clone(),
    })
}

fn structural_call_matches(
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

pub(super) fn unique_structural_effect(
    effects: &ValidatedPreAllocationMachineEffects,
    machine: MachineId,
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

pub(super) fn unique_structural_home(
    homes: &ValidatedRegisterHomes,
    machine: MachineId,
) -> Result<&FunctionRegisterHomes, PostAllocationMachineError> {
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

pub(super) fn validate_structural_allocation<S: ValidatedSelectedAnalysis>(
    selected: &S,
    effects: &ValidatedPreAllocationMachineEffects,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    homes: &ValidatedRegisterHomes,
) -> Result<(), PostAllocationMachineError> {
    let source = &selected.selected_plan().structural_unit_functions;
    if effects.plan().structural_unit_functions.len() != source.len()
        || ranges.plan().structural_unit_functions.len() != source.len()
        || legality.plan().structural_unit_functions.len() != source.len()
        || homes.plan().structural_unit_functions.len() != source.len()
    {
        let machine = source
            .first()
            .map(|function| function.machine)
            .unwrap_or(selected.selected_plan().entry);
        return Err(PostAllocationMachineError::StructuralAllocationMismatch { machine });
    }
    for function in source {
        unique_structural_effect(effects, function.machine)?;
        let range_matches = ranges
            .plan()
            .structural_unit_functions
            .iter()
            .filter(|candidate| candidate.machine == function.machine)
            .collect::<Vec<_>>();
        let legality_matches = legality
            .plan()
            .structural_unit_functions
            .iter()
            .filter(|candidate| candidate.machine == function.machine)
            .collect::<Vec<_>>();
        let home = unique_structural_home(homes, function.machine)?;
        let ([range], [legality]) = (range_matches.as_slice(), legality_matches.as_slice()) else {
            return Err(PostAllocationMachineError::StructuralAllocationMismatch {
                machine: function.machine,
            });
        };
        if range.block_domains.len() != 1
            || range.block_domains[0].block != function.entry_block
            || !range.virtual_registers.is_empty()
            || !range.tied_pairs.is_empty()
            || !range.early_clobbers.is_empty()
            || !range.interference.is_empty()
            || !legality.virtual_registers.is_empty()
            || !home.assignments.is_empty()
        {
            return Err(PostAllocationMachineError::StructuralAllocationMismatch {
                machine: function.machine,
            });
        }
    }
    Ok(())
}
