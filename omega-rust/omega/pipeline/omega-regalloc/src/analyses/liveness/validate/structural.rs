//! Structural-Unit roster validation and independent liveness replay.

use super::function_contract::validate_function;
use super::shared::*;

pub(super) fn validate_structural_unit_roster(
    scalar_functions: &[SelectedFunction],
    selected_functions: &[SelectedStructuralUnitFunction],
    actual_functions: &[FunctionLiveness],
) -> Result<(), LivenessError> {
    let selected_machines = selected_functions
        .iter()
        .map(|function| function.machine)
        .collect::<Vec<_>>();
    validate_structural_machine_roster(
        scalar_functions.iter().map(|function| function.machine),
        &selected_machines,
        actual_functions,
    )?;
    let mut selected_by_machine = BTreeMap::new();
    for (ordinal, function) in selected_functions.iter().enumerate() {
        selected_by_machine.insert(function.machine, (ordinal, function));
    }
    for (ordinal, actual) in actual_functions.iter().enumerate() {
        let (selected_ordinal, selected) = selected_by_machine[&actual.machine];
        debug_assert_eq!(selected_ordinal, ordinal);
        let expected = replay_structural_unit_function(ordinal, selected)?;
        validate_function(ordinal, actual, &expected)?;
    }
    Ok(())
}

pub(super) fn validate_structural_machine_roster(
    scalar_machines: impl IntoIterator<Item = psi_core::MachineId>,
    selected_machines: &[psi_core::MachineId],
    actual_functions: &[FunctionLiveness],
) -> Result<(), LivenessError> {
    if selected_machines.len() != actual_functions.len() {
        return Err(LivenessError::RootMismatch);
    }
    let mut all_selected = BTreeSet::new();
    for machine in scalar_machines
        .into_iter()
        .chain(selected_machines.iter().copied())
    {
        if !all_selected.insert(machine) {
            return Err(LivenessError::DuplicateMachine {
                machine: machine.get(),
            });
        }
    }
    let mut actual_machines = BTreeSet::new();
    for (ordinal, (selected, actual)) in selected_machines.iter().zip(actual_functions).enumerate()
    {
        if !actual_machines.insert(actual.machine) {
            return Err(LivenessError::DuplicateMachine {
                machine: actual.machine.get(),
            });
        }
        if *selected != actual.machine {
            return Err(LivenessError::StructuralFunctionMismatch { function: ordinal });
        }
    }
    Ok(())
}

fn replay_structural_unit_function(
    function_index: usize,
    function: &SelectedStructuralUnitFunction,
) -> Result<FunctionLiveness, LivenessError> {
    let mut selected_rows = Vec::with_capacity(usize::from(function.call.is_some()) + 1);
    if let Some(call) = &function.call {
        selected_rows.push((
            call.id,
            call.implicit_uses.as_slice(),
            call.implicit_defs.as_slice(),
            call.clobbers.as_slice(),
        ));
    }
    let return_instruction = &function.terminator.instruction;
    selected_rows.push((
        return_instruction.id,
        return_instruction.implicit_uses.as_slice(),
        return_instruction.implicit_defs.as_slice(),
        return_instruction.clobbers.as_slice(),
    ));

    let mut live_units = BTreeSet::new();
    let mut instructions = Vec::with_capacity(selected_rows.len());
    for (ordinal, (instruction, uses, defs, clobbers)) in selected_rows.iter().enumerate().rev() {
        let position = LivenessPosition(u32::try_from(ordinal).map_err(|_| {
            LivenessError::NonDensePositions {
                function: function_index,
            }
        })?);
        let live_out = collect(&live_units);
        for unit in defs.iter().chain(clobbers.iter()) {
            live_units.remove(unit);
        }
        live_units.extend(uses.iter().copied());
        instructions.push(InstructionLiveness {
            position,
            instruction: *instruction,
            virtual_uses: Vec::new(),
            virtual_defs: Vec::new(),
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_uses: uses.to_vec(),
            unit_defs: defs.to_vec(),
            unit_clobbers: clobbers.to_vec(),
            unit_live_in: collect(&live_units),
            unit_live_out: live_out,
        });
    }
    instructions.reverse();
    Ok(FunctionLiveness {
        machine: function.machine,
        entry_definitions: Vec::new(),
        operand_positions: Vec::new(),
        blocks: vec![BlockLiveness {
            block: function.entry_block,
            source_block: function.source_entry_block,
            virtual_live_in: Vec::new(),
            virtual_live_out: Vec::new(),
            unit_live_in: collect(&live_units),
            unit_live_out: Vec::new(),
            instructions,
            successors: Vec::new(),
        }],
    })
}
