use crate::machine_code::model::{MachineInstruction, MachineInstructionKind};
use crate::plan::NativePlan;
use omega_core::arena::Handle;
use omega_core::diagnostics::Diagnostic;

pub(in crate::machine_code) fn byte_distances_to_next_runtime_machine_write_end(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    literal: &str,
) -> Result<Vec<isize>, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(Vec::new());
    };
    let Some(machine_write) =
        next_runtime_write_group_end(native_plan, machine_instructions, machine_instruction_index)
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode runtime text guard at byte {}: missing guarded runtime write",
            current.offset
        )));
    };

    let target = machine_write.offset + machine_write.byte_width;
    Ok(literal
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(byte_index, _)| {
            let branch_program_counter = current.offset + 8 + byte_index * 12 + 8;
            target as isize - branch_program_counter as isize
        })
        .collect())
}

pub(in crate::machine_code) fn byte_distance_to_next_runtime_write_end(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    byte_distance_to_next_runtime_write_end_from_branch_offset(
        native_plan,
        machine_instructions,
        machine_instruction_index,
        current.byte_width.saturating_sub(4),
    )
}

pub(in crate::machine_code) fn byte_distance_to_next_runtime_write_end_from_branch_offset(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    branch_offset: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(machine_write) =
        next_runtime_write_group_end(native_plan, machine_instructions, machine_instruction_index)
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode runtime storage guard at byte {}: missing guarded runtime write",
            current.offset
        )));
    };

    let branch_program_counter = current.offset + branch_offset;
    let target = machine_write.offset + machine_write.byte_width;
    Ok(target as isize - branch_program_counter as isize)
}

fn next_runtime_write_group_end<'instructions>(
    native_plan: &NativePlan,
    machine_instructions: &'instructions [MachineInstruction],
    machine_instruction_index: usize,
) -> Option<&'instructions MachineInstruction> {
    let first_write_index = machine_instructions
        .iter()
        .enumerate()
        .skip(machine_instruction_index + 1)
        .find_map(|(index, instruction)| is_runtime_write(instruction).then_some(index))?;

    let first_source =
        selected_instruction_source(native_plan, &machine_instructions[first_write_index]);
    let mut last_write_index = first_write_index;
    for (index, instruction) in machine_instructions
        .iter()
        .enumerate()
        .skip(first_write_index + 1)
    {
        if !is_runtime_write(instruction) {
            break;
        }
        if selected_instruction_source(native_plan, instruction) != first_source {
            break;
        }
        last_write_index = index;
    }

    machine_instructions.get(last_write_index)
}

fn selected_instruction_source<'plan>(
    native_plan: &'plan NativePlan,
    instruction: &MachineInstruction,
) -> Option<(&'plan str, &'plan str)> {
    let handle = Handle::from_arena_index(instruction.selected_instruction_index);
    if !native_plan.instructions.instructions.is_valid(handle) {
        return None;
    }
    let selected = native_plan.instructions.instructions.get(handle);
    Some((
        selected.source_machine.as_str(),
        selected.source_state.as_str(),
    ))
}

fn is_runtime_write(instruction: &MachineInstruction) -> bool {
    matches!(
        instruction.kind,
        MachineInstructionKind::RuntimeMachineIntegerWrite { .. }
            | MachineInstructionKind::RuntimeStorageCopy { .. }
            | MachineInstructionKind::RuntimeTextBufferMaterialize { .. }
            | MachineInstructionKind::RuntimeTextStoredPlaceAppend { .. }
            | MachineInstructionKind::RuntimeTextLiteralAppend { .. }
    )
}
