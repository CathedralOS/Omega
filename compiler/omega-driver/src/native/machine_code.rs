use crate::native::architecture;
use crate::native::instructions::SelectedInstructionKind;
use crate::native::plan::NativePlan;
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
    HostCallSequence {
        capability: String,
        operation: String,
    },
    Return,
}

pub fn build_machine_code_plan(native_plan: &NativePlan) -> MachineCodePlan {
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
        );
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

    machine_code_plan
}

fn select_machine_instructions(
    native_plan: &NativePlan,
    function_offset: usize,
    function: &crate::native::instructions::FunctionInstructionPlan,
    bytes: &mut Arena<u8>,
) -> Vec<MachineInstruction> {
    let Some(selected_instructions) = native_plan
        .instructions
        .instructions
        .span(function.instructions)
    else {
        return Vec::new();
    };

    let mut offset = function_offset;
    let mut machine_instructions = Vec::new();

    for (selected_offset, selected_instruction) in selected_instructions.iter().enumerate() {
        let selected_instruction_index = function
            .instructions
            .start()
            .arena_index()
            .checked_add(u32::try_from(selected_offset).expect("selected instruction overflow"))
            .expect("selected instruction overflow");
        let (kind, byte_width) = machine_instruction_shape(native_plan, &selected_instruction.kind);
        let byte_span = bytes.insert_many(encode_machine_instruction(
            native_plan,
            &selected_instruction.kind,
        ));

        machine_instructions.push(MachineInstruction {
            selected_instruction_index,
            offset,
            byte_width,
            bytes: byte_span,
            kind,
        });
        offset += byte_width;
    }

    machine_instructions
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
        SelectedInstructionKind::LeaveFunction => (
            MachineInstructionKind::Return,
            return_width(native_plan.target.architecture),
        ),
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::BeginPlatformCall { .. } => (MachineInstructionKind::NoBytes, 0),
    }
}

fn encode_machine_instruction(native_plan: &NativePlan, kind: &SelectedInstructionKind) -> Vec<u8> {
    match kind {
        SelectedInstructionKind::HostOperation { operands, .. } => {
            let Some(operands) = native_plan.instructions.operands.span(*operands) else {
                return Vec::new();
            };

            architecture::encode_host_call_sequence(native_plan.target.architecture, operands)
        }
        SelectedInstructionKind::LeaveFunction => {
            architecture::encode_return(native_plan.target.architecture)
        }
        SelectedInstructionKind::EnterFunction
        | SelectedInstructionKind::BeginPlatformCall { .. } => Vec::new(),
    }
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
