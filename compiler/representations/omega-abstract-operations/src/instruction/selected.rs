use crate::{
    AbstractDataObjectHandle, InstructionOperand, StateGuardLowering, StateGuardOperator,
};
use omega_control_flow::StateKey;
use omega_core::arena::{Handle, HandleSpan};
use std::sync::Arc;

pub type AbstractValueOperandHandle = Handle<AbstractValueOperand>;
pub type RuntimeValueOperandHandle = AbstractValueOperandHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractFunctionPlan {
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub instructions: HandleSpan<AbstractOperation>,
}

pub type FunctionInstructionPlan = AbstractFunctionPlan;

impl Default for AbstractFunctionPlan {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperation {
    pub kind: AbstractOperationKind,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type SelectedInstruction = AbstractOperation;

impl Default for AbstractOperation {
    fn default() -> Self {
        Self {
            kind: AbstractOperationKind::EnterFunction,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractValueOperand {
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
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
    },
}

pub type RuntimeValueOperand = AbstractValueOperand;

impl Default for AbstractValueOperand {
    fn default() -> Self {
        Self::Immediate(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractOperationKind {
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
        buffer: AbstractDataObjectHandle,
        literal: Arc<str>,
    },
    CompareRuntimeTextStorage {
        buffer: AbstractDataObjectHandle,
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
        left: AbstractValueOperandHandle,
        right: AbstractValueOperandHandle,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer: AbstractDataObjectHandle,
        literal: Arc<str>,
    },
    WriteRuntimeTextLiteralSegment {
        buffer: AbstractDataObjectHandle,
        byte_offset: usize,
        literal: Arc<str>,
    },
    AppendRuntimeTextStoredSuffix {
        buffer: AbstractDataObjectHandle,
        buffer_offset: usize,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        length_delta: usize,
    },
    MaterializeRuntimeTextBuffer {
        buffer: AbstractDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    MaterializeRuntimeTextBufferToRuntimePointee {
        buffer: AbstractDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
        buffer: AbstractDataObjectHandle,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextStoredPlace {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    AppendRuntimeTextStoredPlaceToRuntimePointee {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
        buffer: AbstractDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextLiteral {
        buffer: AbstractDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        literal: Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimePointee {
        buffer: AbstractDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        literal: Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimeFrameIndexed {
        buffer: AbstractDataObjectHandle,
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
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
    },
    WriteRuntimePointeeBinary {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
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
        left: AbstractValueOperandHandle,
        operator: StateGuardOperator,
        right: AbstractValueOperandHandle,
    },
    WriteRuntimeMachineString {
        byte_offset: usize,
        data: AbstractDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimePointeeString {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        data: AbstractDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeFrameIndexedString {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        data: AbstractDataObjectHandle,
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
        buffer: AbstractDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_capacity: usize,
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
    WriteReturnRegisterInteger {
        byte_size: usize,
        value: i64,
    },
    TerminateDispatch,
    LeaveDispatchCase,
    LeaveDispatchLoop,
    BeginPlatformCall,
    HostOperation {
        operation_ordinal: u16,
        operands: HandleSpan<InstructionOperand>,
    },
    PreparePlatformOutputHandle {
        operands: HandleSpan<InstructionOperand>,
    },
    WritePlatformNewline {
        use_file_api: bool,
        operands: HandleSpan<InstructionOperand>,
    },
    LeaveFunction,
}

pub type SelectedInstructionKind = AbstractOperationKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RuntimeStorageRegion {
    #[default]
    Machine,
    RuntimeFrame,
}
