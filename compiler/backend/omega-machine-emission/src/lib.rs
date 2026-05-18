use omega_calling_conventions::HostAbiPlan;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_bytes::{EncodedMachineFunction, EncodedMachineInstruction, EncodedMachinePlan};
use omega_machine_program::{MachineInstruction, MachineProgram};
use omega_target::NativeTarget;
use omega_target_operations::{
    InstructionPlan, SelectedInstructionKind, StateGuardLowering, StateGuardOperator,
};

mod branch_distances;
mod encoding;
mod host_bindings;
mod layout;

use encoding::encode_machine_instruction_bytes;
use layout::layout_machine_instructions;

#[derive(Debug)]
pub struct MachineEmissionInput<'plan, 'machine> {
    pub target: NativeTarget,
    pub instructions: &'plan InstructionPlan,
    pub machine_program: &'machine MachineProgram,
    pub host_abi: &'plan HostAbiPlan,
    pub terminal_dispatch_index: u32,
}

pub fn emit_machine_bytes(
    input: MachineEmissionInput<'_, '_>,
) -> Result<EncodedMachinePlan, Diagnostic> {
    let mut encoded_bytes = EncodedMachinePlan::with_capacity(
        input.target,
        input.machine_program.functions.len(),
        input.machine_program.instructions.len(),
        0,
    );

    for (_, function) in input.machine_program.functions.iter() {
        let byte_offset = encoded_bytes.bytes.len();
        emit_function_bytes(
            MachineEmissionContext {
                target: input.target,
                instructions: input.instructions,
                host_abi: input.host_abi,
                terminal_dispatch_index: input.terminal_dispatch_index,
            },
            input.machine_program,
            &mut encoded_bytes,
            function.instructions,
        )?;
        let byte_count = encoded_bytes.bytes.len() - byte_offset;
        encoded_bytes.functions.insert(EncodedMachineFunction {
            source_key: function.source_key,
            byte_offset,
            byte_count,
        });
    }

    encoded_bytes.byte_count = encoded_bytes.bytes.len();

    Ok(encoded_bytes)
}

fn emit_function_bytes(
    emission_context: MachineEmissionContext<'_>,
    machine_program: &MachineProgram,
    encoded_plan: &mut EncodedMachinePlan,
    machine_instructions_span: HandleSpan<MachineInstruction>,
) -> Result<(), Diagnostic> {
    let Some(machine_instructions) = machine_program.instructions.span(machine_instructions_span)
    else {
        return Ok(());
    };
    let laid_out_instructions =
        layout_machine_instructions(emission_context, machine_instructions)?;
    encoded_plan.bytes.reserve(
        laid_out_instructions
            .iter()
            .map(|instruction| instruction.byte_width)
            .sum(),
    );

    for (machine_instruction_index, machine_instruction) in machine_instructions.iter().enumerate()
    {
        let laid_out_instruction = &laid_out_instructions[machine_instruction_index];
        if laid_out_instruction.byte_width == 0 {
            encoded_plan.instructions.insert(EncodedMachineInstruction {
                selected_instruction_index: machine_instruction.selected_instruction_index,
                bytes: HandleSpan::empty(),
            });
            continue;
        }

        let selected_handle =
            Handle::from_arena_index(machine_instruction.selected_instruction_index);
        let selected_instruction = emission_context
            .instructions
            .instructions
            .get(selected_handle);
        let byte_span = insert_encoded_machine_instruction(
            &mut encoded_plan.bytes,
            emission_context,
            &laid_out_instructions,
            machine_instruction_index,
            &selected_instruction.kind,
        )?;
        encoded_plan.instructions.insert(EncodedMachineInstruction {
            selected_instruction_index: machine_instruction.selected_instruction_index,
            bytes: byte_span,
        });
    }

    Ok(())
}

fn insert_encoded_machine_instruction(
    encoded_bytes: &mut Arena<u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &SelectedInstructionKind,
) -> Result<HandleSpan<u8>, Diagnostic> {
    encoded_bytes.try_insert_many_with(|inserter| {
        if insert_fixed_machine_instruction_bytes(
            inserter,
            emission_context,
            laid_out_instructions,
            machine_instruction_index,
            kind,
        )? {
            return Ok(());
        }

        for byte in encode_machine_instruction_bytes(
            emission_context,
            laid_out_instructions,
            machine_instruction_index,
            kind,
        )? {
            inserter.insert(byte);
        }

        Ok(())
    })
}

fn insert_fixed_machine_instruction_bytes(
    inserter: &mut omega_core::arena::ArenaSpanInserter<'_, u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &SelectedInstructionKind,
) -> Result<bool, Diagnostic> {
    match kind {
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_loop_enter_bytes(
                emission_context.target.architecture,
                *entry_dispatch_index,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            let bytes = omega_instruction_selection::encode_dispatch_case_enter_bytes(
                emission_context.target.architecture,
                *dispatch_index,
                branch_distances::byte_distance_to_case_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                operator @ (StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_guard_compare_static_bytes(
                emission_context.target.architecture,
                *byte_offset,
                *byte_size,
                *expected_value,
                branch_distances::byte_distance_to_next_state_write_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            insert_dispatch_state_write_bytes(
                inserter,
                emission_context,
                laid_out_instructions,
                machine_instruction_index,
                *dispatch_index,
            )?;
            Ok(true)
        }
        SelectedInstructionKind::TerminateDispatch => {
            insert_dispatch_state_write_bytes(
                inserter,
                emission_context,
                laid_out_instructions,
                machine_instruction_index,
                emission_context.terminal_dispatch_index,
            )?;
            Ok(true)
        }
        SelectedInstructionKind::LeaveDispatchCase => {
            let bytes = omega_instruction_selection::encode_dispatch_case_leave_bytes(
                emission_context.target.architecture,
                branch_distances::byte_distance_to_dispatch_loop_start(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::LeaveFunction => {
            let (bytes, byte_count) = omega_instruction_selection::encode_return_bytes(
                emission_context.target.architecture,
            )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn insert_dispatch_state_write_bytes(
    inserter: &mut omega_core::arena::ArenaSpanInserter<'_, u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    dispatch_index: u32,
) -> Result<(), Diagnostic> {
    let bytes = omega_instruction_selection::encode_dispatch_state_write_bytes(
        emission_context.target.architecture,
        dispatch_index,
        branch_distances::byte_distance_to_case_leave(
            laid_out_instructions,
            machine_instruction_index,
        )?,
    )?;
    for byte in bytes {
        inserter.insert(byte);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct MachineEmissionContext<'plan> {
    pub target: NativeTarget,
    pub instructions: &'plan InstructionPlan,
    pub host_abi: &'plan HostAbiPlan,
    pub terminal_dispatch_index: u32,
}
