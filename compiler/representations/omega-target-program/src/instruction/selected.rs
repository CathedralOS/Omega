use crate::{InstructionOperand, TargetDataObjectHandle};
use crate::{StateGuardLowering, StateGuardOperator};
use omega_calling_conventions::HostOperationKey;
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedInstruction {
    pub kind: SelectedInstructionKind,
    pub source_key: StateKey,
    pub source_statement: usize,
}

impl Default for SelectedInstruction {
    fn default() -> Self {
        Self {
            kind: SelectedInstructionKind::EnterFunction,
            source_key: StateKey::default(),
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
    },
    EnterDispatchCase {
        dispatch_index: u32,
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
        buffer: TargetDataObjectHandle,
        literal: String,
    },
    CompareRuntimeTextStorage {
        buffer: TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeStorage {
        left_region: RuntimeStorageRegion,
        left_offset: usize,
        right_region: RuntimeStorageRegion,
        right_offset: usize,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    CompareRuntimeStorageValue {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer: TargetDataObjectHandle,
        literal: String,
    },
    WriteRuntimeTextLiteralSegment {
        buffer: TargetDataObjectHandle,
        byte_offset: usize,
        literal: String,
    },
    AppendRuntimeTextStoredSuffix {
        buffer: TargetDataObjectHandle,
        buffer_offset: usize,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        length_delta: usize,
    },
    MaterializeRuntimeTextBuffer {
        buffer: TargetDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    AppendRuntimeTextStoredPlace {
        buffer: TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    AppendRuntimeTextLiteral {
        buffer: TargetDataObjectHandle,
        target_region: RuntimeStorageRegion,
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
        data: TargetDataObjectHandle,
        byte_length: usize,
    },
    ReadRuntimeTextLine {
        buffer: TargetDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_capacity: usize,
        source: RuntimeTextReadSource,
    },
    CopyRuntimeStorage {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_count: usize,
    },
    SetDispatchState {
        dispatch_index: u32,
    },
    TerminateDispatch,
    LeaveDispatchCase,
    LeaveDispatchLoop,
    BeginPlatformCall,
    HostOperation {
        operation_key: HostOperationKey,
        operands: HandleSpan<InstructionOperand>,
    },
    LeaveFunction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeTextReadSource {
    Import {
        symbol: String,
    },
    Syscall {
        number: u32,
        number_register: u8,
        supervisor_call: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeStorageRegion {
    #[default]
    Machine,
    RuntimeFrame,
}
