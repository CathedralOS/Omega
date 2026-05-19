use crate::{InstructionOperand, TargetDataObjectHandle};
use crate::{StateGuardLowering, StateGuardOperator};
use omega_calling_conventions::HostOperationKey;
use omega_control_flow::StateKey;
use omega_core::arena::{Handle, HandleSpan};
use std::sync::Arc;

pub type RuntimeValueOperandHandle = Handle<RuntimeValueOperand>;

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
pub enum RuntimeValueOperand {
    Immediate(i64),
    Storage {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
    },
    Pointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    FrameIndexed {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    Binary {
        left: RuntimeValueOperandHandle,
        operator: StateGuardOperator,
        right: RuntimeValueOperandHandle,
    },
}

impl Default for RuntimeValueOperand {
    fn default() -> Self {
        Self::Immediate(0)
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
        storage_region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
        expected_value: i64,
        has_storage: bool,
    },
    CompareRuntimeTextLiteral {
        buffer: TargetDataObjectHandle,
        literal: Arc<str>,
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
    CompareRuntimeValues {
        left: RuntimeValueOperandHandle,
        right: RuntimeValueOperandHandle,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer: TargetDataObjectHandle,
        literal: Arc<str>,
    },
    WriteRuntimeTextLiteralSegment {
        buffer: TargetDataObjectHandle,
        byte_offset: usize,
        literal: Arc<str>,
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
    MaterializeRuntimeTextBufferToRuntimePointee {
        buffer: TargetDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
        buffer: TargetDataObjectHandle,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextStoredPlace {
        buffer: TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    AppendRuntimeTextStoredPlaceToRuntimePointee {
        buffer: TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
        buffer: TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextLiteral {
        buffer: TargetDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        literal: Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimePointee {
        buffer: TargetDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        literal: Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimeFrameIndexed {
        buffer: TargetDataObjectHandle,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        literal: Arc<str>,
    },
    WriteRuntimeMachineInteger {
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeStorageInteger {
        target_region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimePointeeInteger {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeStorageBinary {
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_size: usize,
        left: RuntimeValueOperandHandle,
        operator: StateGuardOperator,
        right: RuntimeValueOperandHandle,
    },
    WriteRuntimePointeeBinary {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
        left: RuntimeValueOperandHandle,
        operator: StateGuardOperator,
        right: RuntimeValueOperandHandle,
    },
    WriteRuntimeFrameIndexedInteger {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeFrameIndexedBinary {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        left: RuntimeValueOperandHandle,
        operator: StateGuardOperator,
        right: RuntimeValueOperandHandle,
    },
    WriteRuntimeMachineString {
        byte_offset: usize,
        data: TargetDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimePointeeString {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        data: TargetDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeFrameIndexedString {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        data: TargetDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeStorageAddressToRuntimeFrame {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_offset: usize,
    },
    WriteRuntimePointeeAddressToRuntimeFrame {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        target_offset: usize,
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
    CopyRuntimeStorageToRuntimeFrameIndexed {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeFrameIndexedToRuntimeFrame {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeFrameFixedIndexedToRuntimeFrame {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeStorageToRuntimePointee {
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
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
        operation_key: HostOperationKey,
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
