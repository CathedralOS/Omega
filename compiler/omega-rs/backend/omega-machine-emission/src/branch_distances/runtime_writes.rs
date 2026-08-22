use crate::MachineEmissionContext;
use crate::layout::LaidOutMachineInstruction;
use omega_assigned_target_operations::{SelectedInstructionKind, StateGuardLowering};
use omega_control_flow::StateKey;
use omega_machine_instructions::MachineInstructionKind;
use omega_target::Architecture;
use psi_arena::Handle;
use psi_diagnostics::Diagnostic;

/// Distance (in bytes, from the start of the current instruction) to the next
/// same-scoped `BranchArmsEnd` marker -- the target of a `ForwardBranchSkip`
/// jump emitted after a matched arm's body in a nested multi-arm transition.
pub(crate) fn byte_distance_to_branch_arms_end(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let branch_scope_id = match current.source_kind {
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::ForwardBranchSkip,
            byte_offset,
            ..
        } => byte_offset,
        _ => 0,
    };
    let target = machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
        .find(|instruction| branch_arms_end_scope_id(input, instruction) == Some(branch_scope_id))
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "cannot encode forward branch skip at byte {}: missing BranchArmsEnd marker",
                current.offset
            ))
        })?;
    Ok(target.offset as isize - current.offset as isize)
}

fn branch_arms_end_scope_id(
    input: MachineEmissionContext<'_>,
    instruction: &LaidOutMachineInstruction,
) -> Option<usize> {
    let handle = Handle::from_arena_index(instruction.selected_instruction_index);
    if !input
        .assigned_target_operations
        .code
        .instructions
        .is_valid(handle)
    {
        return None;
    }
    match input
        .assigned_target_operations
        .code
        .instructions
        .get(handle)
        .kind
    {
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::BranchArmsEnd,
            byte_offset,
            ..
        } => Some(byte_offset),
        _ => None,
    }
}

pub(crate) fn byte_distances_to_next_runtime_machine_write_end(
    architecture: Architecture,
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    literal: &[u8],
) -> Result<RuntimeMachineWriteEndBranchDistances, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Err(Diagnostic::error(format!(
            "cannot encode runtime text guard: missing machine instruction `{machine_instruction_index}`"
        )));
    };
    let Some(target_offset) = next_guarded_runtime_write_target_offset(
        input,
        machine_instructions,
        machine_instruction_index,
    ) else {
        return Err(Diagnostic::error(format!(
            "cannot encode runtime text guard at byte {}: missing guarded runtime write",
            current.offset
        )));
    };

    Ok(RuntimeMachineWriteEndBranchDistances {
        current_offset: current.offset,
        target_offset,
        byte_index: 0,
        byte_count: literal.len(),
        architecture,
    })
}

pub(crate) struct RuntimeMachineWriteEndBranchDistances {
    current_offset: usize,
    target_offset: usize,
    byte_index: usize,
    byte_count: usize,
    architecture: Architecture,
}

impl Iterator for RuntimeMachineWriteEndBranchDistances {
    type Item = isize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.byte_index >= self.byte_count {
            return None;
        }

        let branch_program_counter = match self.architecture {
            Architecture::Aarch64 => self.current_offset + 8 + self.byte_index * 12 + 8,
            Architecture::X86_64 => {
                self.current_offset
                    + omega_isa_x86_64::runtime_text_literal_compare_branch_next_offset(
                        self.byte_index,
                    )
            }
        };
        self.byte_index += 1;
        Some(self.target_offset as isize - branch_program_counter as isize)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.byte_count.saturating_sub(self.byte_index);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for RuntimeMachineWriteEndBranchDistances {}

pub(crate) fn byte_distance_to_next_runtime_write_end(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    byte_distance_to_next_runtime_write_end_from_branch_offset(
        input,
        machine_instructions,
        machine_instruction_index,
        current.byte_width.saturating_sub(4),
    )
}

pub(crate) fn byte_distance_to_next_guarded_effect_end(
    _input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    branch_offset: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(target) =
        next_immediate_guarded_effect_end_offset(machine_instructions, machine_instruction_index)
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode runtime guarded write at byte {}: missing immediate guarded runtime write",
            current.offset
        )));
    };

    let branch_program_counter = current.offset + branch_offset;
    Ok(target as isize - branch_program_counter as isize)
}

pub(crate) fn byte_distance_to_next_runtime_write_end_from_branch_offset(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    branch_offset: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(target) = next_guarded_runtime_write_target_offset(
        input,
        machine_instructions,
        machine_instruction_index,
    ) else {
        return Err(Diagnostic::error(format!(
            "cannot encode runtime storage guard at byte {}: missing guarded runtime write",
            current.offset
        )));
    };

    let branch_program_counter = current.offset + branch_offset;
    Ok(target as isize - branch_program_counter as isize)
}

fn next_immediate_guarded_effect_end_offset(
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Option<usize> {
    let instruction = machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
        .find(|instruction| is_guarded_effect(instruction))?;
    Some(instruction.offset + instruction.byte_width)
}

fn next_guarded_runtime_write_target_offset(
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Option<usize> {
    let current_site =
        selected_instruction_site(input, machine_instructions.get(machine_instruction_index)?);

    if let Some(scope_id) =
        enclosing_branch_scope_id(machine_instructions, machine_instruction_index)
    {
        for instruction in machine_instructions
            .iter()
            .skip(machine_instruction_index + 1)
        {
            match instruction.source_kind {
                SelectedInstructionKind::EvaluateDispatchGuard {
                    guard_lowering: StateGuardLowering::ForwardBranchSkip,
                    byte_offset,
                    ..
                } if byte_offset == scope_id => {
                    return Some(instruction.offset + instruction.byte_width);
                }
                SelectedInstructionKind::EvaluateDispatchGuard {
                    guard_lowering: StateGuardLowering::BranchArmsEnd,
                    byte_offset,
                    ..
                } if byte_offset == scope_id => return Some(instruction.offset),
                _ => {}
            }
        }
    }

    let first_write_index = machine_instructions
        .iter()
        .enumerate()
        .skip(machine_instruction_index + 1)
        .find_map(|(index, instruction)| is_guarded_effect(instruction).then_some(index))?;

    let first_site = selected_instruction_site(input, &machine_instructions[first_write_index]);

    if current_site.is_some() && current_site != first_site {
        // Land on the arm's failure boundary that carries the guard's source_key.
        // Skip same-site guarded EFFECTS: a matched arm's own body (e.g. its
        // assignment-value target copy) shares the guard's source_key and sits
        // before the arm's `ForwardBranchSkip` + trailing NoOp landing. Landing on
        // that copy makes a FAILED guard fall into the matched-arm skip and jump
        // past the next arm — with 3+ arms the middle arm is skipped and the default
        // runs. The real boundary is the first same-site NON-effect (the NoOp marker
        // leaf.rs emits after the ForwardBranchSkip).
        if let Some(boundary) = machine_instructions
            .iter()
            .skip(first_write_index + 1)
            .find(|instruction| {
                selected_instruction_site(input, instruction) == current_site
                    && !is_guarded_effect(instruction)
            })
        {
            return Some(boundary.offset);
        }
    }

    next_runtime_write_group_end(input, machine_instructions, first_write_index)
        .map(|machine_write| machine_write.offset + machine_write.byte_width)
}

fn enclosing_branch_scope_id(
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
) -> Option<usize> {
    // Selection places this zero-width, sentinel-valued NoOp immediately
    // before an arm's guard conjuncts. It keeps scope metadata out of real
    // comparisons while letting their failure branches ignore inner scopes.
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

fn next_runtime_write_group_end<'instructions>(
    input: MachineEmissionContext<'_>,
    machine_instructions: &'instructions [LaidOutMachineInstruction],
    first_write_index: usize,
) -> Option<&'instructions LaidOutMachineInstruction> {
    let first_site = selected_instruction_site(input, &machine_instructions[first_write_index]);
    let mut last_write_index = first_write_index;
    for (index, instruction) in machine_instructions
        .iter()
        .enumerate()
        .skip(first_write_index + 1)
    {
        if !is_guarded_effect(instruction) {
            break;
        }
        if selected_instruction_site(input, instruction) != first_site {
            break;
        }
        last_write_index = index;
    }

    machine_instructions.get(last_write_index)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SelectedInstructionSite {
    source_key: StateKey,
}

fn selected_instruction_site<'plan>(
    input: MachineEmissionContext<'plan>,
    instruction: &LaidOutMachineInstruction,
) -> Option<SelectedInstructionSite> {
    let handle = Handle::from_arena_index(instruction.selected_instruction_index);
    if !input
        .assigned_target_operations
        .code
        .instructions
        .is_valid(handle)
    {
        return None;
    }
    let selected = input
        .assigned_target_operations
        .code
        .instructions
        .get(handle);
    Some(SelectedInstructionSite {
        source_key: selected.source_key,
    })
}

fn is_guarded_effect(instruction: &LaidOutMachineInstruction) -> bool {
    matches!(
        instruction.kind,
        MachineInstructionKind::RuntimeMachineIntegerWrite
            | MachineInstructionKind::RuntimeStorageBitFieldWrite
            | MachineInstructionKind::RuntimeStorageBinaryWrite
            | MachineInstructionKind::RuntimeStorageConvert
            | MachineInstructionKind::RuntimeFrameIndexedIntegerWrite
            | MachineInstructionKind::RuntimeFrameBaseIndexedIntegerWrite
            | MachineInstructionKind::RuntimeFrameIndexedBinaryWrite
            | MachineInstructionKind::RuntimeFrameBaseIndexedBinaryWrite
            | MachineInstructionKind::RuntimeMachineIndexedBinaryWrite
            | MachineInstructionKind::RuntimePointeeIntegerWrite
            | MachineInstructionKind::RuntimePointeeBinaryWrite
            | MachineInstructionKind::RuntimeMachineStringWrite
            | MachineInstructionKind::RuntimeFrameStringWrite
            | MachineInstructionKind::RuntimePointeeStringWrite
            | MachineInstructionKind::RuntimeFrameIndexedStringWrite
            | MachineInstructionKind::RuntimeMachineIndexedStringWrite
            | MachineInstructionKind::RuntimeStorageAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimePointeeAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimeFrameIndexedAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimeFrameFixedIndexedAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimeFrameBaseIndexedAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimeStorageCopy
            | MachineInstructionKind::RuntimeStorageCopyToRuntimePointee
            | MachineInstructionKind::RuntimeStorageCopyToRuntimeFrameIndexed
            | MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameIndexed
            | MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameFixedIndexed
            | MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameFixedIndexedToRuntimePointee
            | MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameIndexedToRuntimePointee
            | MachineInstructionKind::RuntimeStorageCopyFromRuntimePointeeToRuntimeFrame
            | MachineInstructionKind::RuntimeTextBufferMaterialize
            | MachineInstructionKind::RuntimeTextBufferMaterializeToRuntimePointee
            | MachineInstructionKind::RuntimeTextBufferMaterializeToRuntimeFrameIndexed
            | MachineInstructionKind::RuntimeTextStoredPlaceAppend
            | MachineInstructionKind::RuntimeTextStoredPlaceAppendToRuntimePointee
            | MachineInstructionKind::RuntimeTextStoredPlaceAppendToRuntimeFrameIndexed
            | MachineInstructionKind::RuntimeTextLiteralAppend
            | MachineInstructionKind::RuntimeTextLiteralAppendToRuntimePointee
            | MachineInstructionKind::RuntimeTextLiteralAppendToRuntimeFrameIndexed
            | MachineInstructionKind::RuntimeTextLineRead
            | MachineInstructionKind::HostCallSequence
            | MachineInstructionKind::ReturnRegisterIntegerWrite
            | MachineInstructionKind::RuntimeStorageCopyToReturnRegister
            | MachineInstructionKind::DispatchStateWrite
            | MachineInstructionKind::DispatchTerminate
    )
}
