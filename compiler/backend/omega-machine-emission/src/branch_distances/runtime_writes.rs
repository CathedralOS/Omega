use crate::MachineEmissionContext;
use crate::layout::LaidOutMachineInstruction;
use omega_control_flow::StateKey;
use omega_core::arena::Handle;
use omega_core::diagnostics::Diagnostic;
use omega_machine_instructions::MachineInstructionKind;
use omega_target::Architecture;

pub(crate) fn byte_distances_to_next_runtime_machine_write_end(
    architecture: Architecture,
    input: MachineEmissionContext<'_>,
    machine_instructions: &[LaidOutMachineInstruction],
    machine_instruction_index: usize,
    literal: &str,
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

    let first_write_index = machine_instructions
        .iter()
        .enumerate()
        .skip(machine_instruction_index + 1)
        .find_map(|(index, instruction)| is_guarded_effect(instruction).then_some(index))?;

    let first_site = selected_instruction_site(input, &machine_instructions[first_write_index]);

    if current_site.is_some() && current_site != first_site {
        if let Some(boundary) = machine_instructions
            .iter()
            .skip(first_write_index + 1)
            .find(|instruction| selected_instruction_site(input, instruction) == current_site)
        {
            return Some(boundary.offset);
        }
    }

    next_runtime_write_group_end(input, machine_instructions, first_write_index)
        .map(|machine_write| machine_write.offset + machine_write.byte_width)
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
            | MachineInstructionKind::RuntimeStorageBinaryWrite
            | MachineInstructionKind::RuntimeFrameIndexedIntegerWrite
            | MachineInstructionKind::RuntimeFrameBaseIndexedIntegerWrite
            | MachineInstructionKind::RuntimeFrameIndexedBinaryWrite
            | MachineInstructionKind::RuntimeFrameBaseIndexedBinaryWrite
            | MachineInstructionKind::RuntimeMachineStringWrite
            | MachineInstructionKind::RuntimePointeeStringWrite
            | MachineInstructionKind::RuntimeFrameIndexedStringWrite
            | MachineInstructionKind::RuntimeMachineIndexedStringWrite
            | MachineInstructionKind::RuntimeStorageAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimePointeeAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimeFrameIndexedAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimeFrameBaseIndexedAddressToRuntimeFrameWrite
            | MachineInstructionKind::RuntimeStorageCopy
            | MachineInstructionKind::RuntimeStorageCopyToRuntimeFrameIndexed
            | MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameIndexed
            | MachineInstructionKind::RuntimeStorageCopyFromRuntimeFrameFixedIndexed
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
            | MachineInstructionKind::DispatchStateWrite
            | MachineInstructionKind::DispatchTerminate
    )
}
