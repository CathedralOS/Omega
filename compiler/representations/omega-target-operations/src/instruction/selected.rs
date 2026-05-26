use crate::{
    HostOperationKey, InstructionOperand, RuntimeStorageRegion, RuntimeTextReadSource,
    StateGuardLowering, StateGuardOperator, TargetDataObjectHandle, TargetValueOperandHandle,
    target_data_handle_from_abstract,
};
use omega_control_flow::StateKey;
use omega_core::arena::HandleSpan;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperationFunction {
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub instructions: HandleSpan<TargetOperation>,
}

pub type FunctionInstructionPlan = TargetOperationFunction;

impl Default for TargetOperationFunction {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetOperation {
    pub kind: TargetOperationKind,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type SelectedInstruction = TargetOperation;

impl Default for TargetOperation {
    fn default() -> Self {
        Self {
            kind: TargetOperationKind::EnterFunction,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetOperationKind {
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
        literal: std::sync::Arc<str>,
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
        left: TargetValueOperandHandle,
        right: TargetValueOperandHandle,
        byte_size: usize,
        operator: StateGuardOperator,
    },
    WriteRuntimeTextLiteral {
        buffer: TargetDataObjectHandle,
        literal: std::sync::Arc<str>,
    },
    WriteRuntimeTextLiteralSegment {
        buffer: TargetDataObjectHandle,
        byte_offset: usize,
        literal: std::sync::Arc<str>,
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
        literal: std::sync::Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimePointee {
        buffer: TargetDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        literal: std::sync::Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimeFrameIndexed {
        buffer: TargetDataObjectHandle,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        literal: std::sync::Arc<str>,
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
        left: TargetValueOperandHandle,
        operator: StateGuardOperator,
        right: TargetValueOperandHandle,
    },
    WriteRuntimePointeeBinary {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
        left: TargetValueOperandHandle,
        operator: StateGuardOperator,
        right: TargetValueOperandHandle,
    },
    WriteRuntimeFrameIndexedInteger {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeFrameBaseIndexedInteger {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        value: i64,
    },
    WriteRuntimeMachineIndexedInteger {
        base_byte_offset: usize,
        index_region: RuntimeStorageRegion,
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
        left: TargetValueOperandHandle,
        operator: StateGuardOperator,
        right: TargetValueOperandHandle,
    },
    WriteRuntimeFrameBaseIndexedBinary {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
        left: TargetValueOperandHandle,
        operator: StateGuardOperator,
        right: TargetValueOperandHandle,
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
    WriteRuntimeMachineIndexedString {
        base_byte_offset: usize,
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
    WriteRuntimeFrameIndexedAddressToRuntimeFrame {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_offset: usize,
    },
    WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
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
    CopyRuntimeFrameIndexedToRuntimeStorage {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_region: RuntimeStorageRegion,
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
    CopyRuntimeFrameFixedIndexedToRuntimeStorage {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        byte_count: usize,
    },
    CopyRuntimeMachineIndexedToRuntimeStorage {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        target_region: RuntimeStorageRegion,
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
        operation_key: HostOperationKey,
        operands: HandleSpan<InstructionOperand>,
    },
    LeaveFunction,
}

pub type SelectedInstructionKind = TargetOperationKind;

impl From<&omega_abstract_operations::AbstractOperation> for TargetOperation {
    fn from(operation: &omega_abstract_operations::AbstractOperation) -> Self {
        Self {
            kind: TargetOperationKind::from(&operation.kind),
            source_key: operation.source_key,
            source_statement: operation.source_statement,
        }
    }
}

impl From<&omega_abstract_operations::AbstractOperationKind> for TargetOperationKind {
    fn from(kind: &omega_abstract_operations::AbstractOperationKind) -> Self {
        match kind {
            omega_abstract_operations::AbstractOperationKind::EnterFunction => Self::EnterFunction,
            omega_abstract_operations::AbstractOperationKind::EnterDispatchLoop {
                entry_dispatch_index,
                terminal_dispatch_index,
            } => Self::EnterDispatchLoop {
                entry_dispatch_index: *entry_dispatch_index,
                terminal_dispatch_index: *terminal_dispatch_index,
            },
            omega_abstract_operations::AbstractOperationKind::EnterDispatchCase {
                dispatch_index,
            } => Self::EnterDispatchCase {
                dispatch_index: *dispatch_index,
            },
            omega_abstract_operations::AbstractOperationKind::EvaluateDispatchGuard {
                guard_lowering,
                operator,
                storage_region,
                byte_offset,
                byte_size,
                expected_value,
                has_storage,
            } => Self::EvaluateDispatchGuard {
                guard_lowering: *guard_lowering,
                operator: *operator,
                storage_region: *storage_region,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
                expected_value: *expected_value,
                has_storage: *has_storage,
            },
            omega_abstract_operations::AbstractOperationKind::CompareRuntimeTextLiteral {
                buffer,
                literal,
            } => Self::CompareRuntimeTextLiteral {
                buffer: remap_data_handle(*buffer),
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::CompareRuntimeTextStorage {
                buffer,
                source_region,
                source_offset,
                operator,
            } => Self::CompareRuntimeTextStorage {
                buffer: remap_data_handle(*buffer),
                source_region: *source_region,
                source_offset: *source_offset,
                operator: *operator,
            },
            omega_abstract_operations::AbstractOperationKind::CompareRuntimeStorage {
                left_region,
                left_offset,
                right_region,
                right_offset,
                byte_size,
                operator,
            } => Self::CompareRuntimeStorage {
                left_region: *left_region,
                left_offset: *left_offset,
                right_region: *right_region,
                right_offset: *right_offset,
                byte_size: *byte_size,
                operator: *operator,
            },
            omega_abstract_operations::AbstractOperationKind::CompareRuntimeStorageValue {
                region,
                byte_offset,
                byte_size,
                expected_value,
                operator,
            } => Self::CompareRuntimeStorageValue {
                region: *region,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
                expected_value: *expected_value,
                operator: *operator,
            },
            omega_abstract_operations::AbstractOperationKind::CompareRuntimeValues {
                left,
                right,
                byte_size,
                operator,
            } => Self::CompareRuntimeValues {
                left: remap_runtime_value_handle(*left),
                right: remap_runtime_value_handle(*right),
                byte_size: *byte_size,
                operator: *operator,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeTextLiteral {
                buffer,
                literal,
            } => Self::WriteRuntimeTextLiteral {
                buffer: remap_data_handle(*buffer),
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeTextLiteralSegment {
                buffer,
                byte_offset,
                literal,
            } => Self::WriteRuntimeTextLiteralSegment {
                buffer: remap_data_handle(*buffer),
                byte_offset: *byte_offset,
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::AppendRuntimeTextStoredSuffix {
                buffer,
                buffer_offset,
                source_region,
                source_offset,
                target_region,
                target_offset,
                length_delta,
            } => Self::AppendRuntimeTextStoredSuffix {
                buffer: remap_data_handle(*buffer),
                buffer_offset: *buffer_offset,
                source_region: *source_region,
                source_offset: *source_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                length_delta: *length_delta,
            },
            omega_abstract_operations::AbstractOperationKind::MaterializeRuntimeTextBuffer {
                buffer,
                target_region,
                target_offset,
            } => Self::MaterializeRuntimeTextBuffer {
                buffer: remap_data_handle(*buffer),
                target_region: *target_region,
                target_offset: *target_offset,
            },
            omega_abstract_operations::AbstractOperationKind::MaterializeRuntimeTextBufferToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
            } => Self::MaterializeRuntimeTextBufferToRuntimePointee {
                buffer: remap_data_handle(*buffer),
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => Self::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
                buffer: remap_data_handle(*buffer),
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::AppendRuntimeTextStoredPlace {
                buffer,
                source_region,
                source_offset,
                target_region,
                target_offset,
            } => Self::AppendRuntimeTextStoredPlace {
                buffer: remap_data_handle(*buffer),
                source_region: *source_region,
                source_offset: *source_offset,
                target_region: *target_region,
                target_offset: *target_offset,
            },
            omega_abstract_operations::AbstractOperationKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
                buffer,
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            } => Self::AppendRuntimeTextStoredPlaceToRuntimePointee {
                buffer: remap_data_handle(*buffer),
                source_region: *source_region,
                source_offset: *source_offset,
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                buffer,
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => Self::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                buffer: remap_data_handle(*buffer),
                source_region: *source_region,
                source_offset: *source_offset,
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::AppendRuntimeTextLiteral {
                buffer,
                target_region,
                target_offset,
                literal,
            } => Self::AppendRuntimeTextLiteral {
                buffer: remap_data_handle(*buffer),
                target_region: *target_region,
                target_offset: *target_offset,
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::AppendRuntimeTextLiteralToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
                literal,
            } => Self::AppendRuntimeTextLiteralToRuntimePointee {
                buffer: remap_data_handle(*buffer),
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                literal,
            } => Self::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                buffer: remap_data_handle(*buffer),
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeMachineInteger {
                byte_offset: *byte_offset,
                byte_size: *byte_size,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeStorageInteger {
                target_region,
                byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeStorageInteger {
                target_region: *target_region,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimePointeeInteger {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimePointeeInteger {
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeStorageBinary {
                target_region,
                target_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimeStorageBinary {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                left: remap_runtime_value_handle(*left),
                operator: *operator,
                right: remap_runtime_value_handle(*right),
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimePointeeBinary {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimePointeeBinary {
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
                left: remap_runtime_value_handle(*left),
                operator: *operator,
                right: remap_runtime_value_handle(*right),
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeFrameIndexedInteger {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeFrameIndexedInteger {
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeFrameBaseIndexedInteger {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeFrameBaseIndexedInteger {
                base_byte_offset: *base_byte_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeMachineIndexedInteger {
                base_byte_offset,
                index_region,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeMachineIndexedInteger {
                base_byte_offset: *base_byte_offset,
                index_region: *index_region,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeFrameIndexedBinary {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimeFrameIndexedBinary {
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
                left: remap_runtime_value_handle(*left),
                operator: *operator,
                right: remap_runtime_value_handle(*right),
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeFrameBaseIndexedBinary {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimeFrameBaseIndexedBinary {
                base_byte_offset: *base_byte_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                byte_size: *byte_size,
                left: remap_runtime_value_handle(*left),
                operator: *operator,
                right: remap_runtime_value_handle(*right),
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeMachineString {
                byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeMachineString {
                byte_offset: *byte_offset,
                data: remap_data_handle(*data),
                byte_length: *byte_length,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimePointeeString {
                pointer_byte_offset,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimePointeeString {
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
                data: remap_data_handle(*data),
                byte_length: *byte_length,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeFrameIndexedString {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeFrameIndexedString {
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                data: remap_data_handle(*data),
                byte_length: *byte_length,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeMachineIndexedString {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeMachineIndexedString {
                base_byte_offset: *base_byte_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                data: remap_data_handle(*data),
                byte_length: *byte_length,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeStorageAddressToRuntimeFrame {
                source_region,
                source_offset,
                target_offset,
            } => Self::WriteRuntimeStorageAddressToRuntimeFrame {
                source_region: *source_region,
                source_offset: *source_offset,
                target_offset: *target_offset,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            } => Self::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
                target_offset: *target_offset,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } => Self::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                target_offset: *target_offset,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } => Self::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
                base_byte_offset: *base_byte_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                target_offset: *target_offset,
            },
            omega_abstract_operations::AbstractOperationKind::ReadRuntimeTextLine {
                buffer,
                target_region,
                target_offset,
                byte_capacity,
            } => Self::ReadRuntimeTextLine {
                buffer: remap_data_handle(*buffer),
                target_region: *target_region,
                target_offset: *target_offset,
                byte_capacity: *byte_capacity,
                source: RuntimeTextReadSource::HostOperation {
                    operation_key: HostOperationKey::new(
                        omega_calling_conventions::HostCapability::Stdin,
                        omega_calling_conventions::HostOperation::Read,
                    ),
                },
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeStorage {
                source_region,
                source_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeStorage {
                source_region: *source_region,
                source_offset: *source_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeStorageToRuntimeFrameIndexed {
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_count,
            } => Self::CopyRuntimeStorageToRuntimeFrameIndexed {
                source_region: *source_region,
                source_offset: *source_offset,
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                target_offset: *target_offset,
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeFrameIndexedToRuntimeStorage {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameIndexedToRuntimeStorage {
                descriptor_offset: *descriptor_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                descriptor_offset: *descriptor_offset,
                element_index: *element_index,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                target_offset: *target_offset,
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
                descriptor_offset: *descriptor_offset,
                element_index: *element_index,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeMachineIndexedToRuntimeStorage {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeMachineIndexedToRuntimeStorage {
                base_byte_offset: *base_byte_offset,
                index_offset: *index_offset,
                element_byte_size: *element_byte_size,
                field_byte_offset: *field_byte_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeStorageToRuntimePointee {
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
                byte_count,
            } => Self::CopyRuntimeStorageToRuntimePointee {
                source_region: *source_region,
                source_offset: *source_offset,
                pointer_byte_offset: *pointer_byte_offset,
                field_byte_offset: *field_byte_offset,
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::SetDispatchState {
                dispatch_index,
            } => Self::SetDispatchState {
                dispatch_index: *dispatch_index,
            },
            omega_abstract_operations::AbstractOperationKind::WriteReturnRegisterInteger {
                byte_size,
                value,
            } => Self::WriteReturnRegisterInteger {
                byte_size: *byte_size,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::TerminateDispatch => Self::TerminateDispatch,
            omega_abstract_operations::AbstractOperationKind::LeaveDispatchCase => Self::LeaveDispatchCase,
            omega_abstract_operations::AbstractOperationKind::LeaveDispatchLoop => Self::LeaveDispatchLoop,
            omega_abstract_operations::AbstractOperationKind::BeginPlatformCall => Self::BeginPlatformCall,
            omega_abstract_operations::AbstractOperationKind::HostOperation { .. } => {
                panic!("abstract host operation ordinals must be lowered in omega-abstract-operations-to-target-operations")
            }
            omega_abstract_operations::AbstractOperationKind::PreparePlatformOutputHandle { .. }
            | omega_abstract_operations::AbstractOperationKind::WritePlatformNewline { .. } => {
                panic!("logical abstract host operations must be lowered in omega-abstract-operations-to-target-operations")
            }
            omega_abstract_operations::AbstractOperationKind::LeaveFunction => Self::LeaveFunction,
        }
    }
}

fn remap_data_handle(
    handle: omega_abstract_operations::AbstractDataObjectHandle,
) -> TargetDataObjectHandle {
    target_data_handle_from_abstract(handle)
}

fn remap_runtime_value_handle(
    handle: omega_abstract_operations::AbstractValueOperandHandle,
) -> TargetValueOperandHandle {
    omega_core::arena::Handle::from_parts(handle.arena_index(), handle.generation())
}
