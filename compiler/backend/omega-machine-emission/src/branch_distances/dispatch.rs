use omega_core::diagnostics::Diagnostic;

use crate::MachineEmissionContext;
use crate::layout::LaidOutMachineInstruction;
use omega_machine_instructions::MachineInstructionKind;
use omega_target_operations::SelectedInstructionKind;

pub(crate) fn byte_distance_to_case_end(
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(case_leave) = machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
        .find(|instruction| instruction.kind == MachineInstructionKind::DispatchCaseLeave)
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode dispatch case at byte {}: missing matching leave case",
            current.offset
        )));
    };

    let branch_program_counter = current.offset + 4;
    let target = case_leave.offset + case_leave.byte_width;
    Ok(target as isize - branch_program_counter as isize)
}

pub(crate) fn byte_distance_to_next_state_write_end(
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(state_write) = machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
        .find(|instruction| matches!(instruction.kind, MachineInstructionKind::DispatchStateWrite))
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode dispatch guard at byte {}: missing guarded state write",
            current.offset
        )));
    };

    let branch_program_counter = current.offset + 16;
    let target = state_write.offset + state_write.byte_width;
    Ok(target as isize - branch_program_counter as isize)
}

pub(crate) fn byte_distance_to_case_leave(
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(case_leave) = machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
        .find(|instruction| instruction.kind == MachineInstructionKind::DispatchCaseLeave)
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode dispatch state write at byte {}: missing matching leave case",
            current.offset
        )));
    };

    let branch_program_counter = current.offset + 4;
    Ok(case_leave.offset as isize - branch_program_counter as isize)
}

pub(crate) fn byte_distance_to_dispatch_loop_start(
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(loop_enter) = machine_instructions
        .iter()
        .find(|instruction| matches!(instruction.kind, MachineInstructionKind::DispatchLoopEnter))
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode dispatch case leave at byte {}: missing dispatch loop entry",
            current.offset
        )));
    };

    let branch_program_counter = current.offset;
    let target = loop_enter.offset + loop_enter.byte_width;
    Ok(target as isize - branch_program_counter as isize)
}

pub(crate) fn byte_distance_to_dispatch_loop_leave(
    _emission_context: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(loop_leave) = machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
        .find(|instruction| matches!(instruction.source_kind, SelectedInstructionKind::LeaveDispatchLoop))
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode dispatch termination at byte {}: missing dispatch loop leave",
            current.offset
        )));
    };

    let branch_program_counter = current.offset + 4;
    let target = loop_leave.offset + loop_leave.byte_width;
    Ok(target as isize - branch_program_counter as isize)
}
