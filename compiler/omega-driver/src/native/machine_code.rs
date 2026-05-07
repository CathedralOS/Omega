use crate::diagnostics::Diagnostic;
use crate::native::architecture;
use crate::native::instructions::SelectedInstructionKind;
use crate::native::plan::NativePlan;
use crate::native::state_guards::{StateGuardLowering, StateGuardOperator};
use crate::native::target::NativeTarget;
use omega_core::arena::{Arena, HandleSpan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCodePlan {
    pub target: NativeTarget,
    pub functions: Arena<MachineFunctionCode>,
    pub instructions: Arena<MachineInstruction>,
    pub bytes: Arena<u8>,
    pub byte_count: usize,
}

impl Default for MachineCodePlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
            bytes: Arena::new(),
            byte_count: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineFunctionCode {
    pub symbol: String,
    pub offset: usize,
    pub byte_count: usize,
    pub instructions: HandleSpan<MachineInstruction>,
}

impl Default for MachineFunctionCode {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            offset: 0,
            byte_count: 0,
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInstruction {
    pub selected_instruction_index: u32,
    pub offset: usize,
    pub byte_width: usize,
    pub bytes: HandleSpan<u8>,
    pub kind: MachineInstructionKind,
}

impl Default for MachineInstruction {
    fn default() -> Self {
        Self {
            selected_instruction_index: 0,
            offset: 0,
            byte_width: 0,
            bytes: HandleSpan::empty(),
            kind: MachineInstructionKind::NoBytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MachineInstructionKind {
    NoBytes,
    DispatchLoopEnter {
        entry_dispatch_index: u32,
    },
    DispatchCaseEnter {
        dispatch_index: u32,
    },
    DispatchGuardCompareStatic {
        operator: StateGuardOperator,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
    },
    RuntimeTextLiteralCompare {
        literal: String,
    },
    RuntimeTextLiteralWrite {
        literal: String,
    },
    RuntimeMachineIntegerWrite {
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    DispatchStateWrite {
        dispatch_index: u32,
    },
    DispatchTerminate {
        terminal_dispatch_index: u32,
    },
    DispatchCaseLeave,
    HostCallSequence {
        capability: String,
        operation: String,
    },
    Return,
}

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
    function: &crate::native::instructions::FunctionInstructionPlan,
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
        } => (
            MachineInstructionKind::HostCallSequence {
                capability: capability.clone(),
                operation: operation.clone(),
            },
            host_call_sequence_width(
                native_plan.target.architecture,
                native_plan
                    .instructions
                    .operands
                    .span(*operands)
                    .unwrap_or(&[]),
            ),
        ),
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
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => (
            MachineInstructionKind::RuntimeTextLiteralWrite {
                literal: literal.clone(),
            },
            runtime_text_literal_write_width(native_plan.target.architecture, literal),
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
        SelectedInstructionKind::HostOperation { operands, .. } => {
            let Some(operands) = native_plan.instructions.operands.span(*operands) else {
                return Ok(Vec::new());
            };

            architecture::encode_host_call_sequence(native_plan.target.architecture, operands)
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
                    machine_instructions,
                    machine_instruction_index,
                    literal,
                )?,
            )
        }
        SelectedInstructionKind::WriteRuntimeTextLiteral { literal, .. } => {
            architecture::encode_runtime_text_literal_write(
                native_plan.target.architecture,
                literal,
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
    machine_instructions: &[MachineInstruction],
    machine_instruction_index: usize,
    literal: &str,
) -> Result<Vec<isize>, Diagnostic> {
    let Some(current) = machine_instructions.get(machine_instruction_index) else {
        return Ok(Vec::new());
    };
    let Some(machine_write) = machine_instructions
        .iter()
        .skip(machine_instruction_index + 1)
        .find(|instruction| {
            matches!(
                instruction.kind,
                MachineInstructionKind::RuntimeMachineIntegerWrite { .. }
            )
        })
    else {
        return Err(Diagnostic::error(format!(
            "cannot encode runtime text guard at byte {}: missing guarded machine write",
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
    architecture: crate::native::target::Architecture,
    operands: &[crate::native::instructions::InstructionOperand],
) -> usize {
    architecture::host_call_sequence_width(architecture, operands)
}

fn return_width(architecture: crate::native::target::Architecture) -> usize {
    architecture::return_width(architecture)
}

fn dispatch_loop_enter_width(architecture: crate::native::target::Architecture) -> usize {
    architecture::dispatch_loop_enter_width(architecture)
}

fn dispatch_case_enter_width(architecture: crate::native::target::Architecture) -> usize {
    architecture::dispatch_case_enter_width(architecture)
}

fn dispatch_state_write_width(architecture: crate::native::target::Architecture) -> usize {
    architecture::dispatch_state_write_width(architecture)
}

fn dispatch_case_leave_width(architecture: crate::native::target::Architecture) -> usize {
    architecture::dispatch_case_leave_width(architecture)
}

fn dispatch_guard_compare_static_width(architecture: crate::native::target::Architecture) -> usize {
    architecture::dispatch_guard_compare_static_width(architecture)
}

fn runtime_text_literal_compare_width(
    architecture: crate::native::target::Architecture,
    literal: &str,
) -> usize {
    architecture::runtime_text_literal_compare_width(architecture, literal)
}

fn runtime_text_literal_write_width(
    architecture: crate::native::target::Architecture,
    literal: &str,
) -> usize {
    architecture::runtime_text_literal_write_width(architecture, literal)
}

fn runtime_machine_integer_write_width(architecture: crate::native::target::Architecture) -> usize {
    architecture::runtime_machine_integer_write_width(architecture)
}
