use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use omega_core::arena::{Arena, HandleSpan};
use omega_target::NativeTarget;
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionPlan {
    pub target: NativeTarget,
    pub functions: Arena<FunctionInstructionPlan>,
    pub instructions: Arena<SelectedInstruction>,
    pub operands: Arena<InstructionOperand>,
}

impl Default for InstructionPlan {
    fn default() -> Self {
        Self {
            target: NativeTarget::host(),
            functions: Arena::new(),
            instructions: Arena::new(),
            operands: Arena::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionInstructionPlan {
    pub symbol: String,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub instructions: HandleSpan<SelectedInstruction>,
}

impl Default for FunctionInstructionPlan {
    fn default() -> Self {
        Self {
            symbol: String::new(),
            machine: ProgramName::default(),
            state: ProgramName::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstruction {
    pub kind: SelectedInstructionKind,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub source_statement: usize,
}

impl Default for SelectedInstruction {
    fn default() -> Self {
        Self {
            kind: SelectedInstructionKind::EnterFunction,
            source_machine: ProgramName::default(),
            source_state: ProgramName::default(),
            source_statement: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectedInstructionKind {
    EnterFunction,
    EnterDispatchLoop {
        entry_dispatch_index: u32,
        terminal_dispatch_index: u32,
        current_state_slot: String,
        next_state_slot: String,
    },
    EnterDispatchCase {
        dispatch_index: u32,
        label: String,
    },
    EvaluateDispatchGuard {
        guard_lowering: StateGuardLowering,
        operator: StateGuardOperator,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        has_storage: bool,
    },
    CompareRuntimeTextLiteral {
        buffer_symbol: String,
        literal: String,
    },
    CompareRuntimeTextStorage {
        buffer_symbol: String,
        source_symbol: String,
        source_offset: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeStorage {
        left_symbol: String,
        left_offset: usize,
        right_symbol: String,
        right_offset: usize,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeStorageValue {
        symbol: String,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer_symbol: String,
        literal: String,
    },
    WriteRuntimeTextLiteralSegment {
        buffer_symbol: String,
        byte_offset: usize,
        literal: String,
    },
    AppendRuntimeTextStoredSuffix {
        buffer_symbol: String,
        buffer_offset: usize,
        source_symbol: String,
        source_offset: usize,
        target_symbol: String,
        target_offset: usize,
        length_delta: usize,
    },
    MaterializeRuntimeTextBuffer {
        buffer_symbol: String,
        target_symbol: String,
        target_offset: usize,
    },
    AppendRuntimeTextStoredPlace {
        buffer_symbol: String,
        source_symbol: String,
        source_offset: usize,
        target_symbol: String,
        target_offset: usize,
    },
    AppendRuntimeTextLiteral {
        buffer_symbol: String,
        target_symbol: String,
        target_offset: usize,
        literal: String,
    },
    WriteRuntimeMachineInteger {
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeMachineString {
        byte_offset: usize,
        data_symbol: String,
        byte_length: usize,
    },
    ReadRuntimeTextLine {
        buffer_symbol: String,
        target_symbol: String,
        target_offset: usize,
        byte_capacity: usize,
        syscall_number: u32,
        syscall_number_register: u8,
        supervisor_call: u16,
    },
    CopyRuntimeStorage {
        source_symbol: String,
        source_offset: usize,
        target_symbol: String,
        target_offset: usize,
        byte_count: usize,
    },
    SetDispatchState {
        dispatch_index: u32,
    },
    TerminateDispatch,
    LeaveDispatchCase,
    LeaveDispatchLoop,
    BeginPlatformCall {
        platform_call: String,
    },
    HostOperation {
        capability: String,
        operation: String,
        operands: HandleSpan<InstructionOperand>,
    },
    LeaveFunction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstructionOperand {
    pub kind: InstructionOperandKind,
}

impl Default for InstructionOperand {
    fn default() -> Self {
        Self {
            kind: InstructionOperandKind::ImmediateInteger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstructionOperandKind {
    DataAddress { symbol: String },
    RuntimeMachineStringPointer { byte_offset: usize },
    RuntimeMachineStringLength { byte_offset: usize },
    ImmediateInteger(i64),
    ByteLength(usize),
}
