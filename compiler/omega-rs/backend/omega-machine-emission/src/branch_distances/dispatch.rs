use psi_diagnostics::Diagnostic;

use crate::MachineEmissionContext;
use crate::layout::LaidOutMachineInstruction;
use omega_assigned_target_operations::{SelectedInstructionKind, StateGuardLowering};
use omega_machine_instructions::MachineInstructionKind;

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

pub(crate) fn byte_distance_to_next_dispatch_action_end(
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    // An ARM guard -- an inlined multi-arm transition's compare, sitting among a
    // dispatch case's STATEMENT instructions -- fails to its SIBLING arm, not to
    // the state's failure dispatch write. The arm boundary is the
    // `ForwardBranchSkip` jump emitted after the matched arm's body: failure
    // lands immediately AFTER that jump (the trailing NoOp marker = the next
    // arm's first byte). `ForwardBranchSkip` never appears as a dispatch-edge
    // guard (leaf-arm only), so meeting one before any dispatch action proves
    // this guard is an arm guard; a state-level guard reaches its
    // DispatchStateWrite/DispatchTerminate first and keeps the failure-action
    // target. Without the early stop, a failed arm guard sails past its own
    // no-arm into the state trailer: the no-arm body is emitted but orphaned,
    // and `is_zeroish(inf)`'s NaN compare routed straight to the caller's
    // failure exit.
    let branch_program_counter = current.offset + current.byte_width.saturating_sub(4);
    let branch_scope_id =
        enclosing_branch_scope_id(machine_instructions, machine_instruction_index);
    for instruction in machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
    {
        let is_dispatch_action = matches!(
            instruction.kind,
            MachineInstructionKind::DispatchStateWrite | MachineInstructionKind::DispatchTerminate
        );
        let arm_skip_scope_id = match instruction.source_kind {
            SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::ForwardBranchSkip,
                byte_offset,
                ..
            } => Some(byte_offset),
            _ => None,
        };
        let is_arm_skip = arm_skip_scope_id.is_some()
            && branch_scope_id.is_none_or(|scope_id| arm_skip_scope_id == Some(scope_id));
        if is_dispatch_action || is_arm_skip {
            let target = instruction.offset + instruction.byte_width;
            return Ok(target as isize - branch_program_counter as isize);
        }
    }
    Err(Diagnostic::error(format!(
        "cannot encode dispatch guard at byte {}: missing guarded dispatch action",
        current.offset
    )))
}

fn enclosing_branch_scope_id(
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Option<usize> {
    // See the matching marker emitted by runtime-dispatch branch selection.
    for instruction in machine_instructions
        .iter()
        .take(machine_instruction_index)
        .rev()
    {
        match instruction.source_kind {
            SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::BranchArmsEnd,
                ..
            } => return None,
            SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::NoOp,
                byte_offset,
                byte_size: 0,
                expected_value: i64::MIN,
                has_storage: false,
                ..
            } if byte_offset != 0 => return Some(byte_offset),
            _ => {}
        }
    }
    None
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
        .find(|instruction| {
            matches!(
                instruction.source_kind,
                SelectedInstructionKind::LeaveDispatchLoop
            )
        })
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
