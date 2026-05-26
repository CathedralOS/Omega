use omega_control_flow::StateKey;
use omega_core::arena::{Arena, Handle, HandleSpan};
use omega_target::NativeTarget;
use std::sync::Arc;

pub use omega_target_operations::{
    HostOperationKey, RuntimeStorageRegion, RuntimeTextReadSource, StateGuardLowering,
    StateGuardOperator, TargetHostBinding,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedInstructionOperand {
    pub kind: AssignedInstructionOperandKind,
}

impl Default for AssignedInstructionOperand {
    fn default() -> Self {
        Self {
            kind: AssignedInstructionOperandKind::ImmediateInteger(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedInstructionOperandKind {
    DataAddress {
        data: omega_target_operations::TargetDataObjectHandle,
    },
    RuntimeStringPointer {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    RuntimeStringLength {
        region: RuntimeStorageRegion,
        byte_offset: usize,
    },
    ImmediateInteger(i64),
    ByteLength(usize),
}

impl From<omega_target_operations::TargetInstructionOperandKind>
    for AssignedInstructionOperandKind
{
    fn from(kind: omega_target_operations::TargetInstructionOperandKind) -> Self {
        match kind {
            omega_target_operations::TargetInstructionOperandKind::DataAddress { data } => {
                Self::DataAddress { data }
            }
            omega_target_operations::TargetInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Self::RuntimeStringPointer {
                region,
                byte_offset,
            },
            omega_target_operations::TargetInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Self::RuntimeStringLength {
                region,
                byte_offset,
            },
            omega_target_operations::TargetInstructionOperandKind::ImmediateInteger(value) => {
                Self::ImmediateInteger(value)
            }
            omega_target_operations::TargetInstructionOperandKind::ByteLength(value) => {
                Self::ByteLength(value)
            }
        }
    }
}

impl From<AssignedInstructionOperandKind>
    for omega_target_operations::TargetInstructionOperandKind
{
    fn from(kind: AssignedInstructionOperandKind) -> Self {
        match kind {
            AssignedInstructionOperandKind::DataAddress { data } => Self::DataAddress { data },
            AssignedInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Self::RuntimeStringPointer {
                region,
                byte_offset,
            },
            AssignedInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Self::RuntimeStringLength {
                region,
                byte_offset,
            },
            AssignedInstructionOperandKind::ImmediateInteger(value) => {
                Self::ImmediateInteger(value)
            }
            AssignedInstructionOperandKind::ByteLength(value) => Self::ByteLength(value),
        }
    }
}

pub type InstructionOperand = AssignedInstructionOperand;
pub type InstructionOperandKind = AssignedInstructionOperandKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedValueOperandKind {
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
    FrameFixedIndexed {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    Binary {
        left: AssignedValueOperandHandle,
        operator: StateGuardOperator,
        right: AssignedValueOperandHandle,
    },
}

impl From<omega_target_operations::TargetValueOperand> for AssignedValueOperandKind {
    fn from(kind: omega_target_operations::TargetValueOperand) -> Self {
        match kind {
            omega_target_operations::TargetValueOperand::Immediate(value) => Self::Immediate(value),
            omega_target_operations::TargetValueOperand::Storage {
                region,
                byte_offset,
                byte_size,
            } => Self::Storage {
                region,
                byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => Self::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            omega_target_operations::TargetValueOperand::Binary {
                left,
                operator,
                right,
            } => Self::Binary {
                left,
                operator,
                right,
            },
        }
    }
}

impl From<AssignedValueOperandKind> for omega_target_operations::TargetValueOperand {
    fn from(kind: AssignedValueOperandKind) -> Self {
        match kind {
            AssignedValueOperandKind::Immediate(value) => Self::Immediate(value),
            AssignedValueOperandKind::Storage {
                region,
                byte_offset,
                byte_size,
            } => Self::Storage {
                region,
                byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => Self::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Self::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            },
            AssignedValueOperandKind::Binary {
                left,
                operator,
                right,
            } => Self::Binary {
                left,
                operator,
                right,
            },
        }
    }
}

pub type AssignedValueOperandHandle = omega_target_operations::TargetValueOperandHandle;
pub type RuntimeValueOperand = AssignedValueOperandKind;
pub type RuntimeValueOperandHandle = AssignedValueOperandHandle;
pub type TargetValueOperand = AssignedValueOperandKind;
pub type TargetValueOperandHandle = AssignedValueOperandHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignedOperationKind {
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
        buffer: omega_target_operations::TargetDataObjectHandle,
        literal: std::sync::Arc<str>,
    },
    CompareRuntimeTextStorage {
        buffer: omega_target_operations::TargetDataObjectHandle,
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
        buffer: omega_target_operations::TargetDataObjectHandle,
        literal: std::sync::Arc<str>,
    },
    WriteRuntimeTextLiteralSegment {
        buffer: omega_target_operations::TargetDataObjectHandle,
        byte_offset: usize,
        literal: std::sync::Arc<str>,
    },
    AppendRuntimeTextStoredSuffix {
        buffer: omega_target_operations::TargetDataObjectHandle,
        buffer_offset: usize,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        length_delta: usize,
    },
    MaterializeRuntimeTextBuffer {
        buffer: omega_target_operations::TargetDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    MaterializeRuntimeTextBufferToRuntimePointee {
        buffer: omega_target_operations::TargetDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
        buffer: omega_target_operations::TargetDataObjectHandle,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextStoredPlace {
        buffer: omega_target_operations::TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
    },
    AppendRuntimeTextStoredPlaceToRuntimePointee {
        buffer: omega_target_operations::TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
        buffer: omega_target_operations::TargetDataObjectHandle,
        source_region: RuntimeStorageRegion,
        source_offset: usize,
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
    },
    AppendRuntimeTextLiteral {
        buffer: omega_target_operations::TargetDataObjectHandle,
        target_region: RuntimeStorageRegion,
        target_offset: usize,
        literal: std::sync::Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimePointee {
        buffer: omega_target_operations::TargetDataObjectHandle,
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        literal: std::sync::Arc<str>,
    },
    AppendRuntimeTextLiteralToRuntimeFrameIndexed {
        buffer: omega_target_operations::TargetDataObjectHandle,
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
    WriteRuntimeMachineIndexedInteger {
        base_byte_offset: usize,
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
    WriteRuntimeMachineString {
        byte_offset: usize,
        data: omega_target_operations::TargetDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimePointeeString {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        data: omega_target_operations::TargetDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeFrameIndexedString {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        data: omega_target_operations::TargetDataObjectHandle,
        byte_length: usize,
    },
    WriteRuntimeMachineIndexedString {
        base_byte_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        data: omega_target_operations::TargetDataObjectHandle,
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
    ReadRuntimeTextLine {
        buffer: omega_target_operations::TargetDataObjectHandle,
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
        operands: HandleSpan<omega_target_operations::TargetInstructionOperand>,
    },
    LeaveFunction,
}

pub type SelectedInstructionKind = AssignedOperationKind;
pub type TargetOperationKind = AssignedOperationKind;
pub type TargetOperationPlan = omega_target_operations::TargetOperationPlan;

pub type AssignedValueHomeHandle = AssignedValueOperandHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AssignedRegisterBank {
    #[default]
    GeneralPurpose,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86_64AssignedRegister {
    R10,
    R11,
    R12,
    R13,
    R14,
    R15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedRegisterName {
    Aarch64X(u8),
    X86_64(X86_64AssignedRegister),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignedValueHomeKind {
    Immediate,
    StackSlot {
        byte_offset: usize,
        byte_size: usize,
    },
    RuntimeStorage {
        region: RuntimeStorageRegion,
        byte_offset: usize,
        byte_size: usize,
    },
    RuntimePointee {
        pointer_byte_offset: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    RuntimeFrameIndexed {
        descriptor_offset: usize,
        index_offset: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    RuntimeFrameFixedIndexed {
        descriptor_offset: usize,
        element_index: usize,
        element_byte_size: usize,
        field_byte_offset: usize,
        byte_size: usize,
    },
    ScratchRegister {
        bank: AssignedRegisterBank,
        name: AssignedRegisterName,
    },
}

impl Default for AssignedValueHomeKind {
    fn default() -> Self {
        Self::Immediate
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedOperation {
    pub kind: AssignedOperationKind,
    pub source_key: StateKey,
    pub source_statement: usize,
}

pub type SelectedInstruction = AssignedOperation;
pub type TargetOperation = AssignedOperation;

impl Default for AssignedOperation {
    fn default() -> Self {
        Self {
            kind: AssignedOperationKind::EnterFunction,
            source_key: StateKey::default(),
            source_statement: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedValueOperand {
    pub kind: AssignedValueOperandKind,
    pub home: AssignedValueHomeKind,
}

impl Default for AssignedValueOperand {
    fn default() -> Self {
        Self {
            kind: AssignedValueOperandKind::Immediate(0),
            home: AssignedValueHomeKind::Immediate,
        }
    }
}

pub fn assigned_operation_span_from_target(
    span: HandleSpan<omega_target_operations::TargetOperation>,
) -> HandleSpan<AssignedOperation> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(
            Handle::from_parts(span.start().arena_index(), span.start().generation()),
            span.count(),
        )
    }
}

pub fn target_operation_span_from_assigned(
    span: HandleSpan<AssignedOperation>,
) -> HandleSpan<omega_target_operations::TargetOperation> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(
            Handle::from_parts(span.start().arena_index(), span.start().generation()),
            span.count(),
        )
    }
}

impl From<omega_target_operations::TargetOperationKind> for AssignedOperationKind {
    fn from(kind: omega_target_operations::TargetOperationKind) -> Self {
        match kind {
            omega_target_operations::TargetOperationKind::EnterFunction => Self::EnterFunction,
            omega_target_operations::TargetOperationKind::EnterDispatchLoop {
                entry_dispatch_index,
                terminal_dispatch_index,
            } => Self::EnterDispatchLoop {
                entry_dispatch_index,
                terminal_dispatch_index,
            },
            omega_target_operations::TargetOperationKind::EnterDispatchCase { dispatch_index } => {
                Self::EnterDispatchCase { dispatch_index }
            }
            omega_target_operations::TargetOperationKind::EvaluateDispatchGuard {
                guard_lowering,
                operator,
                storage_region,
                byte_offset,
                byte_size,
                expected_value,
                has_storage,
            } => Self::EvaluateDispatchGuard {
                guard_lowering,
                operator,
                storage_region,
                byte_offset,
                byte_size,
                expected_value,
                has_storage,
            },
            omega_target_operations::TargetOperationKind::CompareRuntimeTextLiteral {
                buffer,
                literal,
            } => Self::CompareRuntimeTextLiteral { buffer, literal },
            omega_target_operations::TargetOperationKind::CompareRuntimeTextStorage {
                buffer,
                source_region,
                source_offset,
                operator,
            } => Self::CompareRuntimeTextStorage {
                buffer,
                source_region,
                source_offset,
                operator,
            },
            omega_target_operations::TargetOperationKind::CompareRuntimeStorage {
                left_region,
                left_offset,
                right_region,
                right_offset,
                byte_size,
                operator,
            } => Self::CompareRuntimeStorage {
                left_region,
                left_offset,
                right_region,
                right_offset,
                byte_size,
                operator,
            },
            omega_target_operations::TargetOperationKind::CompareRuntimeStorageValue {
                region,
                byte_offset,
                byte_size,
                expected_value,
                operator,
            } => Self::CompareRuntimeStorageValue {
                region,
                byte_offset,
                byte_size,
                expected_value,
                operator,
            },
            omega_target_operations::TargetOperationKind::CompareRuntimeValues {
                left,
                right,
                byte_size,
                operator,
            } => Self::CompareRuntimeValues {
                left,
                right,
                byte_size,
                operator,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeTextLiteral {
                buffer,
                literal,
            } => Self::WriteRuntimeTextLiteral { buffer, literal },
            omega_target_operations::TargetOperationKind::WriteRuntimeTextLiteralSegment {
                buffer,
                byte_offset,
                literal,
            } => Self::WriteRuntimeTextLiteralSegment {
                buffer,
                byte_offset,
                literal,
            },
            omega_target_operations::TargetOperationKind::AppendRuntimeTextStoredSuffix {
                buffer,
                buffer_offset,
                source_region,
                source_offset,
                target_region,
                target_offset,
                length_delta,
            } => Self::AppendRuntimeTextStoredSuffix {
                buffer,
                buffer_offset,
                source_region,
                source_offset,
                target_region,
                target_offset,
                length_delta,
            },
            omega_target_operations::TargetOperationKind::MaterializeRuntimeTextBuffer {
                buffer,
                target_region,
                target_offset,
            } => Self::MaterializeRuntimeTextBuffer {
                buffer,
                target_region,
                target_offset,
            },
            omega_target_operations::TargetOperationKind::MaterializeRuntimeTextBufferToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
            } => Self::MaterializeRuntimeTextBufferToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
            },
            omega_target_operations::TargetOperationKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => Self::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            },
            omega_target_operations::TargetOperationKind::AppendRuntimeTextStoredPlace {
                buffer,
                source_region,
                source_offset,
                target_region,
                target_offset,
            } => Self::AppendRuntimeTextStoredPlace {
                buffer,
                source_region,
                source_offset,
                target_region,
                target_offset,
            },
            omega_target_operations::TargetOperationKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
                buffer,
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            } => Self::AppendRuntimeTextStoredPlaceToRuntimePointee {
                buffer,
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            },
            omega_target_operations::TargetOperationKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                buffer,
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => Self::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                buffer,
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            },
            omega_target_operations::TargetOperationKind::AppendRuntimeTextLiteral {
                buffer,
                target_region,
                target_offset,
                literal,
            } => Self::AppendRuntimeTextLiteral {
                buffer,
                target_region,
                target_offset,
                literal,
            },
            omega_target_operations::TargetOperationKind::AppendRuntimeTextLiteralToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
                literal,
            } => Self::AppendRuntimeTextLiteralToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
                literal,
            },
            omega_target_operations::TargetOperationKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                literal,
            } => Self::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                literal,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeStorageInteger {
                target_region,
                byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeStorageInteger {
                target_region,
                byte_offset,
                byte_size,
                value,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimePointeeInteger {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimePointeeInteger {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                value,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeStorageBinary {
                target_region,
                target_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimeStorageBinary {
                target_region,
                target_offset,
                byte_size,
                left,
                operator,
                right,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimePointeeBinary {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimePointeeBinary {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeFrameIndexedInteger {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeFrameIndexedInteger {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeMachineIndexedInteger {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeMachineIndexedInteger {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeFrameIndexedBinary {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimeFrameIndexedBinary {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeMachineString {
                byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeMachineString {
                byte_offset,
                data,
                byte_length,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimePointeeString {
                pointer_byte_offset,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimePointeeString {
                pointer_byte_offset,
                field_byte_offset,
                data,
                byte_length,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeFrameIndexedString {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeFrameIndexedString {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeMachineIndexedString {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeMachineIndexedString {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeStorageAddressToRuntimeFrame {
                source_region,
                source_offset,
                target_offset,
            } => Self::WriteRuntimeStorageAddressToRuntimeFrame {
                source_region,
                source_offset,
                target_offset,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            } => Self::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            },
            omega_target_operations::TargetOperationKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } => Self::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            },
            omega_target_operations::TargetOperationKind::ReadRuntimeTextLine {
                buffer,
                target_region,
                target_offset,
                byte_capacity,
                source,
            } => Self::ReadRuntimeTextLine {
                buffer,
                target_region,
                target_offset,
                byte_capacity,
                source,
            },
            omega_target_operations::TargetOperationKind::CopyRuntimeStorage {
                source_region,
                source_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeStorage {
                source_region,
                source_offset,
                target_region,
                target_offset,
                byte_count,
            },
            omega_target_operations::TargetOperationKind::CopyRuntimeStorageToRuntimeFrameIndexed {
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_count,
            } => Self::CopyRuntimeStorageToRuntimeFrameIndexed {
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_count,
            },
            omega_target_operations::TargetOperationKind::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            },
            omega_target_operations::TargetOperationKind::CopyRuntimeFrameIndexedToRuntimeStorage {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameIndexedToRuntimeStorage {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            },
            omega_target_operations::TargetOperationKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            },
            omega_target_operations::TargetOperationKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            },
            omega_target_operations::TargetOperationKind::CopyRuntimeMachineIndexedToRuntimeStorage {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeMachineIndexedToRuntimeStorage {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            },
            omega_target_operations::TargetOperationKind::CopyRuntimeStorageToRuntimePointee {
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
                byte_count,
            } => Self::CopyRuntimeStorageToRuntimePointee {
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
                byte_count,
            },
            omega_target_operations::TargetOperationKind::SetDispatchState { dispatch_index } => {
                Self::SetDispatchState { dispatch_index }
            }
            omega_target_operations::TargetOperationKind::WriteReturnRegisterInteger {
                byte_size,
                value,
            } => Self::WriteReturnRegisterInteger { byte_size, value },
            omega_target_operations::TargetOperationKind::TerminateDispatch => Self::TerminateDispatch,
            omega_target_operations::TargetOperationKind::LeaveDispatchCase => Self::LeaveDispatchCase,
            omega_target_operations::TargetOperationKind::LeaveDispatchLoop => Self::LeaveDispatchLoop,
            omega_target_operations::TargetOperationKind::BeginPlatformCall => Self::BeginPlatformCall,
            omega_target_operations::TargetOperationKind::HostOperation {
                operation_key,
                operands,
            } => Self::HostOperation {
                operation_key,
                operands,
            },
            omega_target_operations::TargetOperationKind::LeaveFunction => Self::LeaveFunction,
        }
    }
}

impl From<AssignedOperationKind> for omega_target_operations::TargetOperationKind {
    fn from(kind: AssignedOperationKind) -> Self {
        match kind {
            AssignedOperationKind::EnterFunction => Self::EnterFunction,
            AssignedOperationKind::EnterDispatchLoop {
                entry_dispatch_index,
                terminal_dispatch_index,
            } => Self::EnterDispatchLoop {
                entry_dispatch_index,
                terminal_dispatch_index,
            },
            AssignedOperationKind::EnterDispatchCase { dispatch_index } => {
                Self::EnterDispatchCase { dispatch_index }
            }
            AssignedOperationKind::EvaluateDispatchGuard {
                guard_lowering,
                operator,
                storage_region,
                byte_offset,
                byte_size,
                expected_value,
                has_storage,
            } => Self::EvaluateDispatchGuard {
                guard_lowering,
                operator,
                storage_region,
                byte_offset,
                byte_size,
                expected_value,
                has_storage,
            },
            AssignedOperationKind::CompareRuntimeTextLiteral { buffer, literal } => {
                Self::CompareRuntimeTextLiteral { buffer, literal }
            }
            AssignedOperationKind::CompareRuntimeTextStorage {
                buffer,
                source_region,
                source_offset,
                operator,
            } => Self::CompareRuntimeTextStorage {
                buffer,
                source_region,
                source_offset,
                operator,
            },
            AssignedOperationKind::CompareRuntimeStorage {
                left_region,
                left_offset,
                right_region,
                right_offset,
                byte_size,
                operator,
            } => Self::CompareRuntimeStorage {
                left_region,
                left_offset,
                right_region,
                right_offset,
                byte_size,
                operator,
            },
            AssignedOperationKind::CompareRuntimeStorageValue {
                region,
                byte_offset,
                byte_size,
                expected_value,
                operator,
            } => Self::CompareRuntimeStorageValue {
                region,
                byte_offset,
                byte_size,
                expected_value,
                operator,
            },
            AssignedOperationKind::CompareRuntimeValues {
                left,
                right,
                byte_size,
                operator,
            } => Self::CompareRuntimeValues {
                left,
                right,
                byte_size,
                operator,
            },
            AssignedOperationKind::WriteRuntimeTextLiteral { buffer, literal } => {
                Self::WriteRuntimeTextLiteral { buffer, literal }
            }
            AssignedOperationKind::WriteRuntimeTextLiteralSegment {
                buffer,
                byte_offset,
                literal,
            } => Self::WriteRuntimeTextLiteralSegment {
                buffer,
                byte_offset,
                literal,
            },
            AssignedOperationKind::AppendRuntimeTextStoredSuffix {
                buffer,
                buffer_offset,
                source_region,
                source_offset,
                target_region,
                target_offset,
                length_delta,
            } => Self::AppendRuntimeTextStoredSuffix {
                buffer,
                buffer_offset,
                source_region,
                source_offset,
                target_region,
                target_offset,
                length_delta,
            },
            AssignedOperationKind::MaterializeRuntimeTextBuffer {
                buffer,
                target_region,
                target_offset,
            } => Self::MaterializeRuntimeTextBuffer {
                buffer,
                target_region,
                target_offset,
            },
            AssignedOperationKind::MaterializeRuntimeTextBufferToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
            } => Self::MaterializeRuntimeTextBufferToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
            },
            AssignedOperationKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => Self::MaterializeRuntimeTextBufferToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            },
            AssignedOperationKind::AppendRuntimeTextStoredPlace {
                buffer,
                source_region,
                source_offset,
                target_region,
                target_offset,
            } => Self::AppendRuntimeTextStoredPlace {
                buffer,
                source_region,
                source_offset,
                target_region,
                target_offset,
            },
            AssignedOperationKind::AppendRuntimeTextStoredPlaceToRuntimePointee {
                buffer,
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            } => Self::AppendRuntimeTextStoredPlaceToRuntimePointee {
                buffer,
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            },
            AssignedOperationKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                buffer,
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            } => Self::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed {
                buffer,
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            },
            AssignedOperationKind::AppendRuntimeTextLiteral {
                buffer,
                target_region,
                target_offset,
                literal,
            } => Self::AppendRuntimeTextLiteral {
                buffer,
                target_region,
                target_offset,
                literal,
            },
            AssignedOperationKind::AppendRuntimeTextLiteralToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
                literal,
            } => Self::AppendRuntimeTextLiteralToRuntimePointee {
                buffer,
                pointer_byte_offset,
                field_byte_offset,
                literal,
            },
            AssignedOperationKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                literal,
            } => Self::AppendRuntimeTextLiteralToRuntimeFrameIndexed {
                buffer,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                literal,
            },
            AssignedOperationKind::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            },
            AssignedOperationKind::WriteRuntimeStorageInteger {
                target_region,
                byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeStorageInteger {
                target_region,
                byte_offset,
                byte_size,
                value,
            },
            AssignedOperationKind::WriteRuntimePointeeInteger {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimePointeeInteger {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                value,
            },
            AssignedOperationKind::WriteRuntimeStorageBinary {
                target_region,
                target_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimeStorageBinary {
                target_region,
                target_offset,
                byte_size,
                left,
                operator,
                right,
            },
            AssignedOperationKind::WriteRuntimePointeeBinary {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimePointeeBinary {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            },
            AssignedOperationKind::WriteRuntimeFrameIndexedInteger {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeFrameIndexedInteger {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            },
            AssignedOperationKind::WriteRuntimeMachineIndexedInteger {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            } => Self::WriteRuntimeMachineIndexedInteger {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                value,
            },
            AssignedOperationKind::WriteRuntimeFrameIndexedBinary {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            } => Self::WriteRuntimeFrameIndexedBinary {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
                left,
                operator,
                right,
            },
            AssignedOperationKind::WriteRuntimeMachineString {
                byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeMachineString {
                byte_offset,
                data,
                byte_length,
            },
            AssignedOperationKind::WriteRuntimePointeeString {
                pointer_byte_offset,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimePointeeString {
                pointer_byte_offset,
                field_byte_offset,
                data,
                byte_length,
            },
            AssignedOperationKind::WriteRuntimeFrameIndexedString {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeFrameIndexedString {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            },
            AssignedOperationKind::WriteRuntimeMachineIndexedString {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            } => Self::WriteRuntimeMachineIndexedString {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                data,
                byte_length,
            },
            AssignedOperationKind::WriteRuntimeStorageAddressToRuntimeFrame {
                source_region,
                source_offset,
                target_offset,
            } => Self::WriteRuntimeStorageAddressToRuntimeFrame {
                source_region,
                source_offset,
                target_offset,
            },
            AssignedOperationKind::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            } => Self::WriteRuntimePointeeAddressToRuntimeFrame {
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
            },
            AssignedOperationKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            } => Self::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
            },
            AssignedOperationKind::ReadRuntimeTextLine {
                buffer,
                target_region,
                target_offset,
                byte_capacity,
                source,
            } => Self::ReadRuntimeTextLine {
                buffer,
                target_region,
                target_offset,
                byte_capacity,
                source,
            },
            AssignedOperationKind::CopyRuntimeStorage {
                source_region,
                source_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeStorage {
                source_region,
                source_offset,
                target_region,
                target_offset,
                byte_count,
            },
            AssignedOperationKind::CopyRuntimeStorageToRuntimeFrameIndexed {
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_count,
            } => Self::CopyRuntimeStorageToRuntimeFrameIndexed {
                source_region,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_count,
            },
            AssignedOperationKind::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameIndexedToRuntimeFrame {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            },
            AssignedOperationKind::CopyRuntimeFrameIndexedToRuntimeStorage {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameIndexedToRuntimeStorage {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            },
            AssignedOperationKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_offset,
                byte_count,
            },
            AssignedOperationKind::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeFrameFixedIndexedToRuntimeStorage {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            },
            AssignedOperationKind::CopyRuntimeMachineIndexedToRuntimeStorage {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            } => Self::CopyRuntimeMachineIndexedToRuntimeStorage {
                base_byte_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                target_region,
                target_offset,
                byte_count,
            },
            AssignedOperationKind::CopyRuntimeStorageToRuntimePointee {
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
                byte_count,
            } => Self::CopyRuntimeStorageToRuntimePointee {
                source_region,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
                byte_count,
            },
            AssignedOperationKind::SetDispatchState { dispatch_index } => {
                Self::SetDispatchState { dispatch_index }
            }
            AssignedOperationKind::WriteReturnRegisterInteger { byte_size, value } => {
                Self::WriteReturnRegisterInteger { byte_size, value }
            }
            AssignedOperationKind::TerminateDispatch => Self::TerminateDispatch,
            AssignedOperationKind::LeaveDispatchCase => Self::LeaveDispatchCase,
            AssignedOperationKind::LeaveDispatchLoop => Self::LeaveDispatchLoop,
            AssignedOperationKind::BeginPlatformCall => Self::BeginPlatformCall,
            AssignedOperationKind::HostOperation {
                operation_key,
                operands,
            } => Self::HostOperation {
                operation_key,
                operands,
            },
            AssignedOperationKind::LeaveFunction => Self::LeaveFunction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationPlan {
    pub target: NativeTarget,
    pub functions: Arena<AssignedTargetOperationFunction>,
    pub instructions: Arena<AssignedOperation>,
    pub operands: Arena<AssignedInstructionOperand>,
    pub runtime_value_operands: Arena<AssignedValueOperand>,
    pub host_bindings: Arena<TargetHostBinding>,
}

impl Default for AssignedTargetOperationPlan {
    fn default() -> Self {
        Self::with_capacity(NativeTarget::host(), 0, 0, 0, 0, 0)
    }
}

impl AssignedTargetOperationPlan {
    pub fn with_capacity(
        target: NativeTarget,
        function_capacity: usize,
        instruction_capacity: usize,
        operand_capacity: usize,
        runtime_value_operand_capacity: usize,
        host_binding_capacity: usize,
    ) -> Self {
        Self {
            target,
            functions: Arena::with_capacity(function_capacity),
            instructions: Arena::with_capacity(instruction_capacity),
            operands: Arena::with_capacity(operand_capacity),
            runtime_value_operands: Arena::with_capacity(runtime_value_operand_capacity),
            host_bindings: Arena::with_capacity(host_binding_capacity),
        }
    }

    pub fn host_binding(&self, operation_key: HostOperationKey) -> Option<&TargetHostBinding> {
        self.host_bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation_key)
            .map(|(_, binding)| binding)
    }

    pub fn instruction_operand(
        &self,
        handle: Handle<omega_target_operations::TargetInstructionOperand>,
    ) -> Option<&AssignedInstructionOperand> {
        let handle = assigned_instruction_handle(handle);
        self.operands
            .is_valid(handle)
            .then(|| self.operands.get(handle))
    }

    pub fn instruction_operands(
        &self,
        span: HandleSpan<omega_target_operations::TargetInstructionOperand>,
    ) -> Option<&[AssignedInstructionOperand]> {
        let span = assigned_instruction_span(span);
        self.operands.span(span)
    }

    pub fn runtime_value_home_handle(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> AssignedValueHomeHandle {
        if assigned_value_handle(handle).is_valid()
            && self
                .runtime_value_operands
                .is_valid(assigned_value_handle(handle))
        {
            handle
        } else {
            AssignedValueHomeHandle::invalid()
        }
    }

    pub fn runtime_value_home(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<AssignedValueHomeKind> {
        self.runtime_value_operand(handle)
            .map(|operand| operand.home)
    }

    pub fn runtime_value_operand(
        &self,
        handle: RuntimeValueOperandHandle,
    ) -> Option<&AssignedValueOperand> {
        let handle = assigned_value_handle(handle);
        self.runtime_value_operands
            .is_valid(handle)
            .then(|| self.runtime_value_operands.get(handle))
    }

    pub fn runtime_values_with_homes(
        &self,
    ) -> impl Iterator<Item = (RuntimeValueOperandHandle, &AssignedValueOperand)> + '_ {
        self.runtime_value_operands
            .iter()
            .map(|(handle, operand)| (target_value_handle(handle), operand))
    }

    pub fn scratch_home_count(&self) -> usize {
        self.runtime_values_with_homes()
            .filter(|(_, operand)| {
                matches!(operand.home, AssignedValueHomeKind::ScratchRegister { .. })
            })
            .count()
    }
}

impl omega_target_operations::InstructionOperandLike for AssignedInstructionOperand {
    fn data_address(&self) -> Option<omega_target_operations::TargetDataObjectHandle> {
        match self.kind {
            AssignedInstructionOperandKind::DataAddress { data } => Some(data),
            _ => None,
        }
    }

    fn runtime_string_pointer(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            AssignedInstructionOperandKind::RuntimeStringPointer {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn runtime_string_length(&self) -> Option<(RuntimeStorageRegion, usize)> {
        match self.kind {
            AssignedInstructionOperandKind::RuntimeStringLength {
                region,
                byte_offset,
            } => Some((region, byte_offset)),
            _ => None,
        }
    }

    fn immediate_integer(&self) -> Option<i64> {
        match self.kind {
            AssignedInstructionOperandKind::ImmediateInteger(value) => Some(value),
            _ => None,
        }
    }

    fn byte_length(&self) -> Option<usize> {
        match self.kind {
            AssignedInstructionOperandKind::ByteLength(value) => Some(value),
            _ => None,
        }
    }
}

impl omega_target_operations::RuntimeValueOperandSource for AssignedTargetOperationPlan {
    fn immediate_integer(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<i64> {
        match AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Immediate(value) => Some(value),
            _ => None,
        }
    }

    fn storage(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(RuntimeStorageRegion, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Storage {
                region,
                byte_offset,
                byte_size,
            } => Some((*region, *byte_offset, *byte_size)),
            _ => None,
        }
    }

    fn pointee(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Pointee {
                pointer_byte_offset,
                field_byte_offset,
                byte_size,
            } => Some((*pointer_byte_offset, *field_byte_offset, *byte_size)),
            _ => None,
        }
    }

    fn frame_indexed(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::FrameIndexed {
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *descriptor_offset,
                *index_offset,
                *element_byte_size,
                *field_byte_offset,
                *byte_size,
            )),
            _ => None,
        }
    }

    fn frame_fixed_indexed(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(usize, usize, usize, usize, usize)> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::FrameFixedIndexed {
                descriptor_offset,
                element_index,
                element_byte_size,
                field_byte_offset,
                byte_size,
            } => Some((
                *descriptor_offset,
                *element_index,
                *element_byte_size,
                *field_byte_offset,
                *byte_size,
            )),
            _ => None,
        }
    }

    fn binary(
        &self,
        handle: omega_target_operations::RuntimeValueOperandHandle,
    ) -> Option<(
        omega_target_operations::RuntimeValueOperandHandle,
        StateGuardOperator,
        omega_target_operations::RuntimeValueOperandHandle,
    )> {
        match &AssignedTargetOperationPlan::runtime_value_operand(self, handle)?.kind {
            AssignedValueOperandKind::Binary {
                left,
                operator,
                right,
            } => Some((*left, *operator, *right)),
            _ => None,
        }
    }
}

fn assigned_value_handle(handle: RuntimeValueOperandHandle) -> Handle<AssignedValueOperand> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn assigned_instruction_handle(
    handle: Handle<omega_target_operations::TargetInstructionOperand>,
) -> Handle<AssignedInstructionOperand> {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

fn assigned_instruction_span(
    span: HandleSpan<omega_target_operations::TargetInstructionOperand>,
) -> HandleSpan<AssignedInstructionOperand> {
    if span.is_empty() {
        HandleSpan::empty()
    } else {
        HandleSpan::from_parts(assigned_instruction_handle(span.start()), span.count())
    }
}

fn target_value_handle(handle: Handle<AssignedValueOperand>) -> RuntimeValueOperandHandle {
    Handle::from_parts(handle.arena_index(), handle.generation())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignedTargetOperationFunction {
    pub symbol: Arc<str>,
    pub source_key: StateKey,
    pub instructions: HandleSpan<AssignedOperation>,
}

impl Default for AssignedTargetOperationFunction {
    fn default() -> Self {
        Self {
            symbol: Arc::from(""),
            source_key: StateKey::default(),
            instructions: HandleSpan::empty(),
        }
    }
}

impl From<omega_target_operations::TargetOperationPlan> for AssignedTargetOperationPlan {
    fn from(plan: omega_target_operations::TargetOperationPlan) -> Self {
        let mut functions = Arena::with_capacity(plan.functions.len());
        for (_, function) in plan.functions.iter() {
            functions.insert(AssignedTargetOperationFunction {
                symbol: Arc::clone(&function.symbol),
                source_key: function.source_key,
                instructions: assigned_operation_span_from_target(function.instructions),
            });
        }

        let mut instructions = Arena::with_capacity(plan.instructions.len());
        for (_, instruction) in plan.instructions.iter() {
            instructions.insert(AssignedOperation {
                kind: instruction.kind.clone().into(),
                source_key: instruction.source_key,
                source_statement: instruction.source_statement,
            });
        }

        let mut runtime_value_operands = Arena::with_capacity(plan.runtime_value_operands.len());
        for (_, operand) in plan.runtime_value_operands.iter() {
            runtime_value_operands.insert(AssignedValueOperand {
                kind: operand.clone().into(),
                home: AssignedValueHomeKind::Immediate,
            });
        }

        Self {
            target: plan.target,
            functions,
            instructions,
            operands: {
                let mut operands = Arena::with_capacity(plan.operands.len());
                for (_, operand) in plan.operands.iter() {
                    operands.insert(AssignedInstructionOperand {
                        kind: operand.kind.clone().into(),
                    });
                }
                operands
            },
            runtime_value_operands,
            host_bindings: plan.host_bindings,
        }
    }
}

impl From<AssignedTargetOperationPlan> for omega_target_operations::TargetOperationPlan {
    fn from(plan: AssignedTargetOperationPlan) -> Self {
        let mut functions = Arena::with_capacity(plan.functions.len());
        for (_, function) in plan.functions.iter() {
            functions.insert(omega_target_operations::TargetOperationFunction {
                symbol: Arc::clone(&function.symbol),
                source_key: function.source_key,
                instructions: target_operation_span_from_assigned(function.instructions),
            });
        }

        let mut instructions = Arena::with_capacity(plan.instructions.len());
        for (_, instruction) in plan.instructions.iter() {
            instructions.insert(omega_target_operations::TargetOperation {
                kind: instruction.kind.clone().into(),
                source_key: instruction.source_key,
                source_statement: instruction.source_statement,
            });
        }

        let runtime_value_operands = {
            let mut runtime_value_operands =
                Arena::with_capacity(plan.runtime_value_operands.len());
            for (_, operand) in plan.runtime_value_operands.iter() {
                runtime_value_operands.insert(operand.kind.clone().into());
            }
            runtime_value_operands
        };

        Self {
            target: plan.target,
            functions,
            instructions,
            operands: {
                let mut operands = Arena::with_capacity(plan.operands.len());
                for (_, operand) in plan.operands.iter() {
                    operands.insert(omega_target_operations::TargetInstructionOperand {
                        kind: operand.kind.clone().into(),
                    });
                }
                operands
            },
            runtime_value_operands,
            host_bindings: plan.host_bindings,
        }
    }
}
