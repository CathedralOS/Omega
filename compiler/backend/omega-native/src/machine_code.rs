use crate::abi::HostBindingMechanism;
use crate::architecture;
use crate::instructions::SelectedInstructionKind;
use crate::plan::NativePlan;
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;

mod model;

pub use model::{MachineCodePlan, MachineFunctionCode, MachineInstruction, MachineInstructionKind};

pub fn build_machine_code_plan(native_plan: &NativePlan) -> Result<MachineCodePlan, Diagnostic> {
    let mut machine_code_plan = MachineCodePlan {
        target: native_plan.target,
        functions: Arena::new(),
        instructions: Arena::new(),
        bytes: Arena::new(),
        byte_count: 0,
    };

    for (_, function) in native_plan.instructions.functions.iter() {
        let function_offset = machine_code_plan.byte_count;
        let machine_instructions = select_machine_instructions(
            native_plan,
            function_offset,
            function,
            &mut machine_code_plan.bytes,
        )?;
        let function_byte_count = machine_instructions
            .iter()
            .map(|instruction| instruction.byte_width)
            .sum();
        let instructions = machine_code_plan
            .instructions
            .insert_many(machine_instructions);

        machine_code_plan.functions.insert(MachineFunctionCode {
            symbol: function.symbol.clone(),
            offset: function_offset,
            byte_count: function_byte_count,
            instructions,
        });
        machine_code_plan.byte_count += function_byte_count;
    }

    Ok(machine_code_plan)
}

fn select_machine_instructions(
    native_plan: &NativePlan,
    function_offset: usize,
    function: &crate::instructions::FunctionInstructionPlan,
    bytes: &mut Arena<u8>,
) -> Result<Vec<MachineInstruction>, Diagnostic> {
    let Some(selected_instructions) = native_plan
        .instructions
        .instructions
        .span(function.instructions)
    else {
        return Ok(Vec::new());
    };

    let mut offset = function_offset;
    let mut machine_instructions = selected_instructions
        .iter()
        .enumerate()
        .map(|(selected_offset, selected_instruction)| {
            let selected_instruction_index = function
                .instructions
                .start()
                .arena_index()
                .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
                .expect("selected instruction overflow");
            let (kind, byte_width) =
                machine_instruction_shape(native_plan, &selected_instruction.kind);
            let instruction = MachineInstruction {
                selected_instruction_index,
                offset,
                byte_width,
                bytes: HandleSpan::empty(),
                kind,
            };
            offset += byte_width;
            instruction
        })
        .collect::<Vec<_>>();

    for (selected_offset, selected_instruction) in selected_instructions.iter().enumerate() {
        let selected_instruction_index =
            machine_instructions[selected_offset].selected_instruction_index;
        let byte_span = bytes.insert_many(encode_machine_instruction(
            native_plan,
            &machine_instructions,
            selected_offset,
            &selected_instruction.kind,
        )?);

        debug_assert_eq!(
            selected_instruction_index,
            machine_instructions[selected_offset].selected_instruction_index
        );
        machine_instructions[selected_offset].bytes = byte_span;
    }

    Ok(machine_instructions)
}

fn machine_instruction_shape(
    native_plan: &NativePlan,
    kind: &SelectedInstructionKind,
) -> (MachineInstructionKind, usize) {
    match kind {
        SelectedInstructionKind::HostOperation {
            capability,
            operation,
            operands,
        } => {
            let operands = native_plan
                .instructions
                .operands
                .span(*operands)
                .unwrap_or(&[]);
            let byte_width = match host_binding_mechanism(native_plan, capability, operation) {
                Some(HostBindingMechanism::Syscall { number, .. }) => {
                    architecture::syscall_sequence_width(
                        native_plan.target.architecture,
                        operands,
                        *number,
                    )
                }
                _ => host_call_sequence_width(native_plan.target.architecture, operands),
            };

            (
                MachineInstructionKind::HostCallSequence {
                    capability: capability.clone(),
                    operation: operation.clone(),
                },
                byte_width,
            )
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => (
            MachineInstructionKind::DispatchLoopEnter {
                entry_dispatch_index: *entry_dispatch_index,
            },
            dispatch_loop_enter_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => (
            MachineInstructionKind::DispatchCaseEnter {
                dispatch_index: *dispatch_index,
            },
            dispatch_case_enter_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator: operator @ (StateGuardOperator::Equal | StateGuardOperator::NotEqual),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            ..
        } => (
            MachineInstructionKind::DispatchGuardCompareStatic {
                operator: *operator,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
                expected_value: *expected_value,
            },
            dispatch_guard_compare_static_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => (
            MachineInstructionKind::RuntimeTextLiteralCompare {
                literal: literal.clone(),
            },
            runtime_text_literal_compare_width(native_plan.target.architecture, literal),
        ),
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => (
            MachineInstructionKind::RuntimeTextStorageCompare {
                source_offset: *source_offset,
                operator: *operator,
            },
            runtime_text_storage_compare_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            ..
        } => (
            MachineInstructionKind::RuntimeStorageCompare {
                left_offset: *left_offset,
                right_offset: *right_offset,
                byte_size: *byte_size,
                operator: *operator,
            },
            runtime_storage_compare_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::CompareRuntimeStorageValue {
            byte_offset,
            byte_size,
            expected_value,
            operator,
            ..
        } => (
            MachineInstructionKind::RuntimeStorageValueCompare {
                byte_offset: *byte_offset,
                byte_size: *byte_size,
                expected_value: *expected_value,
                operator: *operator,
            },
            runtime_storage_value_compare_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => (
            MachineInstructionKind::RuntimeTextLiteralWrite {
                literal: literal.clone(),
            },
            runtime_text_literal_write_width(native_plan.target.architecture, literal),
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            byte_offset,
            literal,
            ..
        } => (
            MachineInstructionKind::RuntimeTextLiteralSegmentWrite {
                byte_offset: *byte_offset,
                literal: literal.clone(),
            },
            runtime_text_literal_segment_write_width(native_plan.target.architecture, literal),
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => (
            MachineInstructionKind::RuntimeTextStoredSuffixAppend {
                buffer_offset: *buffer_offset,
                source_offset: *source_offset,
                target_offset: *target_offset,
                length_delta: *length_delta,
            },
            runtime_text_stored_suffix_append_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => (
            MachineInstructionKind::RuntimeTextBufferMaterialize {
                target_offset: *target_offset,
            },
            runtime_text_buffer_materialize_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => (
            MachineInstructionKind::RuntimeTextStoredPlaceAppend {
                source_offset: *source_offset,
                target_offset: *target_offset,
            },
            runtime_text_stored_place_append_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            target_offset,
            literal,
            ..
        } => (
            MachineInstructionKind::RuntimeTextLiteralAppend {
                target_offset: *target_offset,
                literal: literal.clone(),
            },
            runtime_text_literal_append_width(native_plan.target.architecture, literal),
        ),
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => (
            MachineInstructionKind::RuntimeMachineIntegerWrite {
                byte_offset: *byte_offset,
                byte_size: *byte_size,
                value: *value,
            },
            runtime_machine_integer_write_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => (
            MachineInstructionKind::RuntimeMachineStringWrite {
                byte_offset: *byte_offset,
                byte_length: *byte_length,
            },
            runtime_machine_string_write_width(native_plan.target.architecture, *byte_length),
        ),
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            syscall_number,
            syscall_number_register,
            supervisor_call,
            ..
        } => (
            MachineInstructionKind::RuntimeTextLineRead {
                target_offset: *target_offset,
                byte_capacity: *byte_capacity,
                syscall_number: *syscall_number,
                syscall_number_register: *syscall_number_register,
                supervisor_call: *supervisor_call,
            },
            runtime_text_line_read_width(
                native_plan.target.architecture,
                *byte_capacity,
                *syscall_number,
            ),
        ),
        SelectedInstructionKind::CopyRuntimeStorage {
            source_offset,
            target_offset,
            byte_count,
            ..
        } => (
            MachineInstructionKind::RuntimeStorageCopy {
                source_offset: *source_offset,
                target_offset: *target_offset,
                byte_count: *byte_count,
            },
            runtime_storage_copy_width(native_plan.target.architecture, *byte_count),
        ),
        SelectedInstructionKind::SetDispatchState { dispatch_index } => (
            MachineInstructionKind::DispatchStateWrite {
                dispatch_index: *dispatch_index,
            },
            dispatch_state_write_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::TerminateDispatch => (
            MachineInstructionKind::DispatchTerminate {
                terminal_dispatch_index: native_plan.runtime_dispatch_loop.terminal_dispatch_index,
            },
            dispatch_state_write_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::LeaveDispatchCase => (
            MachineInstructionKind::DispatchCaseLeave,
            dispatch_case_leave_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::LeaveFunction => (
            MachineInstructionKind::Return,
            return_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall { .. } => (MachineInstructionKind::NoBytes, 0),
    }
}

fn encode_machine_instruction(
    native_plan: &NativePlan,
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    kind: &SelectedInstructionKind,
) -> Result<Vec<u8>, Diagnostic> {
    match kind {
        SelectedInstructionKind::HostOperation {
            capability,
            operation,
            operands,
        } => {
            let Some(operands) = native_plan.instructions.operands.span(*operands) else {
                return Ok(Vec::new());
            };

            match host_binding_mechanism(native_plan, capability, operation) {
                Some(HostBindingMechanism::Syscall {
                    number,
                    number_register,
                    supervisor_call,
                    ..
                }) => architecture::encode_syscall_sequence(
                    native_plan.target.architecture,
                    operands,
                    *number,
                    *number_register,
                    *supervisor_call,
                ),
                _ => architecture::encode_host_call_sequence(
                    native_plan.target.architecture,
                    operands,
                ),
            }
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => architecture::encode_dispatch_loop_enter(
            native_plan.target.architecture,
            *entry_dispatch_index,
        ),
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            architecture::encode_dispatch_case_enter(
                native_plan.target.architecture,
                *dispatch_index,
                byte_distance_to_case_end(machine_instructions, machine_instruction_index)?,
            )
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator: operator @ (StateGuardOperator::Equal | StateGuardOperator::NotEqual),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            ..
        } => architecture::encode_dispatch_guard_compare_static(
            native_plan.target.architecture,
            *byte_offset,
            *byte_size,
            *expected_value,
            byte_distance_to_next_state_write_end(machine_instructions, machine_instruction_index)?,
            *operator == StateGuardOperator::NotEqual,
        ),
        SelectedInstructionKind::CompareRuntimeTextLiteral { literal, .. } => {
            architecture::encode_runtime_text_literal_compare(
                native_plan.target.architecture,
                literal,
                byte_distances_to_next_runtime_machine_write_end(
                    native_plan,
                    machine_instructions,
                    machine_instruction_index,
                    literal,
                )?,
                byte_distance_to_next_runtime_write_end(
                    native_plan,
                    machine_instructions,
                    machine_instruction_index,
                )?,
            )
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => architecture::encode_runtime_text_storage_compare(
            native_plan.target.architecture,
            *source_offset,
            byte_distance_to_next_runtime_write_end_from_branch_offset(
                native_plan,
                machine_instructions,
                machine_instruction_index,
                40,
            )?,
            byte_distance_to_next_runtime_write_end_from_branch_offset(
                native_plan,
                machine_instructions,
                machine_instruction_index,
                runtime_text_storage_compare_width(native_plan.target.architecture)
                    .saturating_sub(4),
            )?,
            *operator == StateGuardOperator::NotEqual,
        ),
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            ..
        } => architecture::encode_runtime_storage_compare(
            native_plan.target.architecture,
            *left_offset,
            *right_offset,
            *byte_size,
            byte_distance_to_next_runtime_write_end(
                native_plan,
                machine_instructions,
                machine_instruction_index,
            )?,
            *operator == StateGuardOperator::NotEqual,
        ),
        SelectedInstructionKind::CompareRuntimeStorageValue {
            byte_offset,
            byte_size,
            expected_value,
            operator,
            ..
        } => architecture::encode_runtime_storage_value_compare(
            native_plan.target.architecture,
            *byte_offset,
            *byte_size,
            *expected_value,
            byte_distance_to_next_runtime_write_end(
                native_plan,
                machine_instructions,
                machine_instruction_index,
            )?,
            *operator == StateGuardOperator::NotEqual,
        ),
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            architecture::encode_runtime_text_literal_write(
                native_plan.target.architecture,
                literal,
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
            byte_offset,
            literal,
            ..
        } => architecture::encode_runtime_text_literal_segment_write(
            native_plan.target.architecture,
            *byte_offset,
            literal,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
            ..
        } => architecture::encode_runtime_text_stored_suffix_append(
            native_plan.target.architecture,
            *buffer_offset,
            *source_offset,
            *target_offset,
            *length_delta,
        ),
        SelectedInstructionKind::AppendRuntimeTextStoredPlace {
            source_offset,
            target_offset,
            ..
        } => architecture::encode_runtime_text_stored_place_append(
            native_plan.target.architecture,
            0,
            *source_offset,
            *target_offset,
        ),
        SelectedInstructionKind::AppendRuntimeTextLiteral {
            target_offset,
            literal,
            ..
        } => architecture::encode_runtime_text_literal_append(
            native_plan.target.architecture,
            0,
            *target_offset,
            literal,
        ),
        SelectedInstructionKind::MaterializeRuntimeTextBuffer { target_offset, .. } => {
            architecture::encode_runtime_text_buffer_materialize(
                native_plan.target.architecture,
                *target_offset,
            )
        }
        SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        } => architecture::encode_runtime_machine_integer_write(
            native_plan.target.architecture,
            *byte_offset,
            *byte_size,
            *value,
        ),
        SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            byte_length,
            ..
        } => architecture::encode_runtime_machine_string_write(
            native_plan.target.architecture,
            *byte_offset,
            *byte_length,
        ),
        SelectedInstructionKind::ReadRuntimeTextLine {
            target_offset,
            byte_capacity,
            syscall_number,
            syscall_number_register,
            supervisor_call,
            ..
        } => architecture::encode_runtime_text_line_read(
            native_plan.target.architecture,
            *target_offset,
            *byte_capacity,
            *syscall_number,
            *syscall_number_register,
            *supervisor_call,
        ),
        SelectedInstructionKind::CopyRuntimeStorage {
            source_offset,
            target_offset,
            byte_count,
            ..
        } => architecture::encode_runtime_storage_copy(
            native_plan.target.architecture,
            *source_offset,
            *target_offset,
            *byte_count,
        ),
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            architecture::encode_dispatch_state_write(
                native_plan.target.architecture,
                *dispatch_index,
                byte_distance_to_case_leave(machine_instructions, machine_instruction_index)?,
            )
        }
        SelectedInstructionKind::TerminateDispatch => architecture::encode_dispatch_state_write(
            native_plan.target.architecture,
            native_plan.runtime_dispatch_loop.terminal_dispatch_index,
            byte_distance_to_case_leave(machine_instructions, machine_instruction_index)?,
        ),
        SelectedInstructionKind::LeaveDispatchCase => architecture::encode_dispatch_case_leave(
            native_plan.target.architecture,
            byte_distance_to_dispatch_loop_start(machine_instructions, machine_instruction_index)?,
        ),
        SelectedInstructionKind::LeaveFunction => {
            architecture::encode_return(native_plan.target.architecture)
        }
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::EvaluateDispatchGuard { .. }
        | SelectedInstructionKind::LeaveDispatchLoop
        | SelectedInstructionKind::BeginPlatformCall { .. } => Ok(Vec::new()),
    }
}

fn host_binding_mechanism<'plan>(
    native_plan: &'plan NativePlan,
    capability: &str,
    operation: &str,
) -> Option<&'plan HostBindingMechanism> {
    native_plan
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.capability == capability && binding.operation == operation)
        .map(|(_, binding)| &binding.mechanism)
}

fn byte_distance_to_case_end(
    machine_instructions: &[MachineInstruction],
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

fn byte_distance_to_next_state_write_end(
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(state_write) = machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
        .find(|instruction| {
            matches!(
                instruction.kind,
                MachineInstructionKind::DispatchStateWrite { .. }
            )
        })
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

fn byte_distance_to_case_leave(
    machine_instructions: &[MachineInstruction],
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

fn byte_distances_to_next_runtime_machine_write_end(
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

fn byte_distance_to_next_runtime_write_end(
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

fn byte_distance_to_next_runtime_write_end_from_branch_offset(
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

fn byte_distance_to_dispatch_loop_start(
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
) -> Result<isize, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(0);
    };
    let Some(loop_enter) = machine_instructions.iter().find(|instruction| {
        matches!(
            instruction.kind,
            MachineInstructionKind::DispatchLoopEnter { .. }
        )
    }) else {
        return Err(Diagnostic::error(format!(
            "cannot encode dispatch case leave at byte {}: missing dispatch loop entry",
            current.offset
        )));
    };

    let branch_program_counter = current.offset;
    let target = loop_enter.offset + loop_enter.byte_width;
    Ok(target as isize - branch_program_counter as isize)
}

fn host_call_sequence_width(
    architecture: crate::target::Architecture,
    operands: &[crate::instructions::InstructionOperand],
) -> usize {
    architecture::host_call_sequence_width(architecture, operands)
}

fn return_width(architecture: crate::target::Architecture) -> usize {
    architecture::return_width(architecture)
}

fn dispatch_loop_enter_width(architecture: crate::target::Architecture) -> usize {
    architecture::dispatch_loop_enter_width(architecture)
}

fn dispatch_case_enter_width(architecture: crate::target::Architecture) -> usize {
    architecture::dispatch_case_enter_width(architecture)
}

fn dispatch_state_write_width(architecture: crate::target::Architecture) -> usize {
    architecture::dispatch_state_write_width(architecture)
}

fn dispatch_case_leave_width(architecture: crate::target::Architecture) -> usize {
    architecture::dispatch_case_leave_width(architecture)
}

fn dispatch_guard_compare_static_width(architecture: crate::target::Architecture) -> usize {
    architecture::dispatch_guard_compare_static_width(architecture)
}

fn runtime_text_literal_compare_width(
    architecture: crate::target::Architecture,
    literal: &str,
) -> usize {
    architecture::runtime_text_literal_compare_width(architecture, literal)
}

fn runtime_text_storage_compare_width(architecture: crate::target::Architecture) -> usize {
    architecture::runtime_text_storage_compare_width(architecture)
}

fn runtime_storage_compare_width(architecture: crate::target::Architecture) -> usize {
    architecture::runtime_storage_compare_width(architecture)
}

fn runtime_storage_value_compare_width(architecture: crate::target::Architecture) -> usize {
    architecture::runtime_storage_value_compare_width(architecture)
}

fn runtime_text_literal_write_width(
    architecture: crate::target::Architecture,
    literal: &str,
) -> usize {
    architecture::runtime_text_literal_write_width(architecture, literal)
}

fn runtime_text_literal_segment_write_width(
    architecture: crate::target::Architecture,
    literal: &str,
) -> usize {
    architecture::runtime_text_literal_segment_write_width(architecture, literal)
}

fn runtime_text_stored_suffix_append_width(architecture: crate::target::Architecture) -> usize {
    architecture::runtime_text_stored_suffix_append_width(architecture)
}

fn runtime_text_buffer_materialize_width(architecture: crate::target::Architecture) -> usize {
    architecture::runtime_text_buffer_materialize_width(architecture)
}

fn runtime_text_stored_place_append_width(architecture: crate::target::Architecture) -> usize {
    architecture::runtime_text_stored_place_append_width(architecture)
}

fn runtime_text_literal_append_width(
    architecture: crate::target::Architecture,
    literal: &str,
) -> usize {
    architecture::runtime_text_literal_append_width(architecture, literal)
}

fn runtime_machine_integer_write_width(architecture: crate::target::Architecture) -> usize {
    architecture::runtime_machine_integer_write_width(architecture)
}

fn runtime_machine_string_write_width(
    architecture: crate::target::Architecture,
    byte_length: usize,
) -> usize {
    architecture::runtime_machine_string_write_width(architecture, byte_length)
}

fn runtime_text_line_read_width(
    architecture: crate::target::Architecture,
    byte_capacity: usize,
    syscall_number: u32,
) -> usize {
    architecture::runtime_text_line_read_width(architecture, byte_capacity, syscall_number)
}

fn runtime_storage_copy_width(
    architecture: crate::target::Architecture,
    byte_count: usize,
) -> usize {
    architecture::runtime_storage_copy_width(architecture, byte_count)
}
