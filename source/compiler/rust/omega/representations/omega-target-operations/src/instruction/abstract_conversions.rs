use super::{TargetOperation, TargetOperationKind};
use crate::{
    HostOperationKey, RuntimeTextReadSource, TargetDataObjectHandle, TargetValueOperandHandle,
    target_data_handle_from_abstract,
};

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
                is_float,
            } => Self::EvaluateDispatchGuard {
                guard_lowering: *guard_lowering,
                operator: *operator,
                storage_region: *storage_region,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
                expected_value: *expected_value,
                has_storage: *has_storage,
                is_float: *is_float,
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
            omega_abstract_operations::AbstractOperationKind::ComparePlaces {
                left,
                right,
                byte_size,
                operator,
                is_float,
            } => Self::ComparePlaces {
                left: *left,
                right: *right,
                byte_size: *byte_size,
                operator: *operator,
                is_float: *is_float,
            },
            omega_abstract_operations::AbstractOperationKind::ComparePlaceValue {
                place,
                byte_size,
                expected_value,
                operator,
            } => Self::ComparePlaceValue {
                place: *place,
                byte_size: *byte_size,
                expected_value: *expected_value,
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
            omega_abstract_operations::AbstractOperationKind::MaterializeTextBufferToPlace {
                buffer,
                target,
            } => Self::MaterializeTextBufferToPlace {
                buffer: remap_data_handle(*buffer),
                target: *target,
            },
            omega_abstract_operations::AbstractOperationKind::AppendTextStoredToPlace {
                buffer,
                source_region,
                source_offset,
                target,
            } => Self::AppendTextStoredToPlace {
                buffer: remap_data_handle(*buffer),
                source_region: *source_region,
                source_offset: *source_offset,
                target: *target,
            },
            omega_abstract_operations::AbstractOperationKind::AppendTextLiteralToPlace {
                buffer,
                target,
                literal,
            } => Self::AppendTextLiteralToPlace {
                buffer: remap_data_handle(*buffer),
                target: *target,
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::AppendWireLiteralByte {
                out_region,
                out_offset,
                written_region,
                written_offset,
                value,
            } => Self::AppendWireLiteralByte {
                out_region: *out_region,
                out_offset: *out_offset,
                written_region: *written_region,
                written_offset: *written_offset,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::AppendWireScalarVarint {
                source_region,
                source_offset,
                byte_size,
                zigzag,
                out_region,
                out_offset,
                written_region,
                written_offset,
            } => Self::AppendWireScalarVarint {
                source_region: *source_region,
                source_offset: *source_offset,
                byte_size: *byte_size,
                zigzag: *zigzag,
                out_region: *out_region,
                out_offset: *out_offset,
                written_region: *written_region,
                written_offset: *written_offset,
            },
            omega_abstract_operations::AbstractOperationKind::AppendWireTextBytes {
                source_region,
                source_offset,
                out_region,
                out_offset,
                out_length,
                written_region,
                written_offset,
            } => Self::AppendWireTextBytes {
                source_region: *source_region,
                source_offset: *source_offset,
                out_region: *out_region,
                out_offset: *out_offset,
                out_length: *out_length,
                written_region: *written_region,
                written_offset: *written_offset,
            },
            omega_abstract_operations::AbstractOperationKind::AppendWireScalarSlice {
                source_region,
                source_offset,
                element_byte_size,
                zigzag,
                out_region,
                out_offset,
                out_length,
                written_region,
                written_offset,
            } => Self::AppendWireScalarSlice {
                source_region: (*source_region).into(),
                source_offset: *source_offset,
                element_byte_size: *element_byte_size,
                zigzag: *zigzag,
                out_region: (*out_region).into(),
                out_offset: *out_offset,
                out_length: *out_length,
                written_region: (*written_region).into(),
                written_offset: *written_offset,
            },
            omega_abstract_operations::AbstractOperationKind::ReadWireExpectedByte {
                buffer_region,
                buffer_offset,
                buffer_length,
                read_region,
                read_offset,
                ok_region,
                ok_offset,
                expected,
            } => Self::ReadWireExpectedByte {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                expected: *expected,
            },
            omega_abstract_operations::AbstractOperationKind::ReadWireScalarVarint {
                buffer_region,
                buffer_offset,
                buffer_length,
                read_region,
                read_offset,
                ok_region,
                ok_offset,
                target_region,
                target_offset,
                byte_size,
                zigzag,
                range,
            } => Self::ReadWireScalarVarint {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                zigzag: *zigzag,
                range: *range,
            },
            omega_abstract_operations::AbstractOperationKind::ReadWireByteSlice {
                buffer_region,
                buffer_offset,
                buffer_length,
                read_region,
                read_offset,
                ok_region,
                ok_offset,
                target_region,
                target_offset,
                predicate_mask,
            } => Self::ReadWireByteSlice {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                predicate_mask: *predicate_mask,
            },
            omega_abstract_operations::AbstractOperationKind::ReadWireNestedOpen {
                buffer_region,
                buffer_offset,
                buffer_length,
                read_region,
                read_offset,
                ok_region,
                ok_offset,
                end_region,
                end_offset,
            } => Self::ReadWireNestedOpen {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                end_region: *end_region,
                end_offset: *end_offset,
            },
            omega_abstract_operations::AbstractOperationKind::ReadWireNestedClose {
                buffer_region,
                buffer_offset,
                read_region,
                read_offset,
                ok_region,
                ok_offset,
                end_region,
                end_offset,
            } => Self::ReadWireNestedClose {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                end_region: *end_region,
                end_offset: *end_offset,
            },
            omega_abstract_operations::AbstractOperationKind::AppendWireRepeatedScalarVarint {
                source_region,
                source_offset,
                byte_size,
                zigzag,
                index,
                count_region,
                count_offset,
                out_region,
                out_offset,
                written_region,
                written_offset,
            } => Self::AppendWireRepeatedScalarVarint {
                source_region: *source_region,
                source_offset: *source_offset,
                byte_size: *byte_size,
                zigzag: *zigzag,
                index: *index,
                count_region: *count_region,
                count_offset: *count_offset,
                out_region: *out_region,
                out_offset: *out_offset,
                written_region: *written_region,
                written_offset: *written_offset,
            },
            omega_abstract_operations::AbstractOperationKind::ReadWireRepeatedScalarVarint {
                buffer_region,
                buffer_offset,
                buffer_length,
                read_region,
                read_offset,
                ok_region,
                ok_offset,
                end_region,
                end_offset,
                count_region,
                count_offset,
                target_region,
                target_offset,
                byte_size,
                zigzag,
                range,
            } => Self::ReadWireRepeatedScalarVarint {
                buffer_region: *buffer_region,
                buffer_offset: *buffer_offset,
                buffer_length: *buffer_length,
                read_region: *read_region,
                read_offset: *read_offset,
                ok_region: *ok_region,
                ok_offset: *ok_offset,
                end_region: *end_region,
                end_offset: *end_offset,
                count_region: *count_region,
                count_offset: *count_offset,
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                zigzag: *zigzag,
                range: *range,
            },
            omega_abstract_operations::AbstractOperationKind::WriteEntryArgumentRegister {
                register,
                byte_offset,
                byte_size,
            } => Self::WriteEntryArgumentRegister {
                register: *register,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
            omega_abstract_operations::AbstractOperationKind::WriteEntryStackArgument {
                stack_byte_offset,
                byte_offset,
                byte_size,
            } => Self::WriteEntryStackArgument {
                stack_byte_offset: *stack_byte_offset,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
            omega_abstract_operations::AbstractOperationKind::WriteEntryIndirectArgument {
                pointer,
                byte_offset,
                byte_size,
            } => Self::WriteEntryIndirectArgument {
                pointer: *pointer,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
            omega_abstract_operations::AbstractOperationKind::WriteEntryArgumentsSliceDescriptor {
                descriptor_offset,
                spill_offset,
                byte_length,
            } => Self::WriteEntryArgumentsSliceDescriptor {
                descriptor_offset: *descriptor_offset,
                spill_offset: *spill_offset,
                byte_length: *byte_length,
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeStorageConvert {
                target_region,
                target_offset,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            } => Self::WriteRuntimeStorageConvert {
                target_region: *target_region,
                target_offset: *target_offset,
                target_byte_size: *target_byte_size,
                source: remap_runtime_value_handle(*source),
                source_byte_size: *source_byte_size,
                source_is_float: *source_is_float,
                target_is_float: *target_is_float,
                source_signed: *source_signed,
                target_signed: *target_signed,
                trapping: *trapping,
                saturating: *saturating,
            },
            omega_abstract_operations::AbstractOperationKind::WritePlaceConvert {
                target,
                target_byte_size,
                source,
                source_byte_size,
                source_is_float,
                target_is_float,
                source_signed,
                target_signed,
                trapping,
                saturating,
            } => Self::WritePlaceConvert {
                target: (*target).into(),
                target_byte_size: *target_byte_size,
                source: remap_runtime_value_handle(*source),
                source_byte_size: *source_byte_size,
                source_is_float: *source_is_float,
                target_is_float: *target_is_float,
                source_signed: *source_signed,
                target_signed: *target_signed,
                trapping: *trapping,
                saturating: *saturating,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicLoad {
                source_region,
                source_offset,
                byte_size,
                result_region,
                result_offset,
                ordering,
            } => Self::AtomicLoad {
                source_region: *source_region,
                source_offset: *source_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicStore {
                target_region,
                target_offset,
                byte_size,
                value,
                ordering,
            } => Self::AtomicStore {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                value: remap_runtime_value_handle(*value),
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicFetchAdd {
                target_region,
                target_offset,
                byte_size,
                result_region,
                result_offset,
                delta,
                ordering,
            } => Self::AtomicFetchAdd {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                delta: remap_runtime_value_handle(*delta),
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicFetchSub {
                target_region,
                target_offset,
                byte_size,
                result_region,
                result_offset,
                delta,
                ordering,
            } => Self::AtomicFetchSub {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                delta: remap_runtime_value_handle(*delta),
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicFetchXor {
                target_region,
                target_offset,
                byte_size,
                result_region,
                result_offset,
                value,
                ordering,
            } => Self::AtomicFetchXor {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                value: remap_runtime_value_handle(*value),
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicFetchOr {
                target_region,
                target_offset,
                byte_size,
                result_region,
                result_offset,
                value,
                ordering,
            } => Self::AtomicFetchOr {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                value: remap_runtime_value_handle(*value),
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicFetchAnd {
                target_region,
                target_offset,
                byte_size,
                result_region,
                result_offset,
                value,
                ordering,
            } => Self::AtomicFetchAnd {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                value: remap_runtime_value_handle(*value),
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicSwap {
                target_region,
                target_offset,
                byte_size,
                result_region,
                result_offset,
                new_value,
                ordering,
            } => Self::AtomicSwap {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                new_value: remap_runtime_value_handle(*new_value),
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AtomicCompareExchange {
                target_region,
                target_offset,
                byte_size,
                result_region,
                result_offset,
                expected,
                new_value,
                ordering,
            } => Self::AtomicCompareExchange {
                target_region: *target_region,
                target_offset: *target_offset,
                byte_size: *byte_size,
                result_region: *result_region,
                result_offset: *result_offset,
                expected: remap_runtime_value_handle(*expected),
                new_value: remap_runtime_value_handle(*new_value),
                ordering: *ordering,
            },
            omega_abstract_operations::AbstractOperationKind::AppendPlaceBoundedBufferSource {
                target,
                source,
            } => Self::AppendPlaceBoundedBufferSource {
                target: (*target).into(),
                source: (*source).into(),
            },
            omega_abstract_operations::AbstractOperationKind::AppendPlaceBoundedBufferLiteral {
                target,
                literal,
            } => Self::AppendPlaceBoundedBufferLiteral {
                target: (*target).into(),
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::ReadRuntimeTextLine {
                buffer,
                target_region,
                target_offset,
                byte_capacity,
                target,
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
                target: *target,
            },
            omega_abstract_operations::AbstractOperationKind::ReadRuntimeByte {
                target_region,
                target_offset,
                payload_offset,
            } => Self::ReadRuntimeByte {
                target_region: *target_region,
                target_offset: *target_offset,
                payload_offset: *payload_offset,
                source: RuntimeTextReadSource::HostOperation {
                    operation_key: HostOperationKey::new(
                        omega_calling_conventions::HostCapability::Stdin,
                        omega_calling_conventions::HostOperation::Read,
                    ),
                },
            },
            omega_abstract_operations::AbstractOperationKind::WriteRuntimeByte {
                source_region,
                source_offset,
                literal,
                source_is_place,
            } => Self::WriteRuntimeByte {
                source_region: *source_region,
                source_offset: *source_offset,
                literal: remap_data_handle(*literal),
                source_is_place: *source_is_place,
                source: RuntimeTextReadSource::HostOperation {
                    operation_key: HostOperationKey::new(
                        omega_calling_conventions::HostCapability::Stdout,
                        omega_calling_conventions::HostOperation::Write,
                    ),
                },
            },
            omega_abstract_operations::AbstractOperationKind::CopyPlaces {
                source,
                target,
                byte_count,
                role,
            } => Self::CopyPlaces {
                source: *source,
                target: *target,
                byte_count: *byte_count,
                role: *role,
            },
            omega_abstract_operations::AbstractOperationKind::WritePlaceInteger {
                target,
                value,
                byte_size,
            } => Self::WritePlaceInteger {
                target: *target,
                value: *value,
                byte_size: *byte_size,
            },
            omega_abstract_operations::AbstractOperationKind::WriteStorageBitField {
                region,
                base_byte_offset,
                fragments,
                value,
            } => Self::WriteStorageBitField {
                region: *region,
                base_byte_offset: *base_byte_offset,
                fragments: fragments.clone(),
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::WritePlaceBinary {
                target,
                byte_size,
                left,
                operator,
                right,
                is_float,
                domain,
                target_signed,
            } => Self::WritePlaceBinary {
                target: *target,
                byte_size: *byte_size,
                left: remap_runtime_value_handle(*left),
                operator: *operator,
                right: remap_runtime_value_handle(*right),
                is_float: *is_float,
                domain: *domain,
                target_signed: *target_signed,
            },
            omega_abstract_operations::AbstractOperationKind::WritePlaceString {
                target,
                data,
                byte_length,
            } => Self::WritePlaceString {
                target: *target,
                data: remap_data_handle(*data),
                byte_length: *byte_length,
            },
            omega_abstract_operations::AbstractOperationKind::WritePlaceBoundedBuffer {
                target,
                literal,
            } => Self::WritePlaceBoundedBuffer {
                target: *target,
                literal: literal.clone(),
            },
            omega_abstract_operations::AbstractOperationKind::WritePlaceAddress {
                source,
                target_offset,
            } => Self::WritePlaceAddress {
                source: *source,
                target_offset: *target_offset,
            },
            omega_abstract_operations::AbstractOperationKind::WriteDataAddressToRuntimeFrame {
                data,
                target_offset,
            } => Self::WriteDataAddressToRuntimeFrame {
                data: remap_data_handle(*data),
                target_offset: *target_offset,
            },
            omega_abstract_operations::AbstractOperationKind::SetDispatchState {
                dispatch_index,
            } => Self::SetDispatchState {
                dispatch_index: *dispatch_index,
            },
            omega_abstract_operations::AbstractOperationKind::WriteReturnRegisterInteger {
                register,
                byte_size,
                value,
            } => Self::WriteReturnRegisterInteger {
                register: *register,
                byte_size: *byte_size,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::CopyRuntimeStorageToReturnRegister {
                register,
                region,
                byte_offset,
                byte_size,
            } => Self::CopyRuntimeStorageToReturnRegister {
                register: *register,
                region: *region,
                byte_offset: *byte_offset,
                byte_size: *byte_size,
            },
            omega_abstract_operations::AbstractOperationKind::TerminateDispatch => Self::TerminateDispatch,
            omega_abstract_operations::AbstractOperationKind::LeaveDispatchCase => Self::LeaveDispatchCase,
            omega_abstract_operations::AbstractOperationKind::LeaveDispatchLoop => Self::LeaveDispatchLoop,
            omega_abstract_operations::AbstractOperationKind::CallInternalFunction { target } => {
                Self::CallInternalFunction { target: *target }
            }
            omega_abstract_operations::AbstractOperationKind::LoadOutgoingStackAddress {
                register,
                stack_byte_offset,
            } => Self::LoadOutgoingStackAddress {
                register: *register,
                stack_byte_offset: *stack_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::ReserveOutgoingStackFrame {
                byte_count,
            } => Self::ReserveOutgoingStackFrame {
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::WriteOutgoingStackU64 {
                stack_byte_offset,
                value,
            } => Self::WriteOutgoingStackU64 {
                stack_byte_offset: *stack_byte_offset,
                value: *value,
            },
            omega_abstract_operations::AbstractOperationKind::CopyEntryIndirectU64ToOutgoingStack {
                source_register,
                source_byte_offset,
                stack_byte_offset,
            } => Self::CopyEntryIndirectU64ToOutgoingStack {
                source_register: *source_register,
                source_byte_offset: *source_byte_offset,
                stack_byte_offset: *stack_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::ReleaseOutgoingStackFrame {
                byte_count,
            } => Self::ReleaseOutgoingStackFrame {
                byte_count: *byte_count,
            },
            omega_abstract_operations::AbstractOperationKind::BeginPlatformCall => Self::BeginPlatformCall,
            omega_abstract_operations::AbstractOperationKind::HostOperation { .. } => {
                panic!("abstract host operation ordinals must be lowered in omega-abstract-operations-to-target-operations")
            }
            omega_abstract_operations::AbstractOperationKind::PreparePlatformOutputHandle { .. }
            | omega_abstract_operations::AbstractOperationKind::WritePlatformNewline { .. } => {
                panic!("logical abstract host operations must be lowered in omega-abstract-operations-to-target-operations")
            }
            omega_abstract_operations::AbstractOperationKind::MachineHalt => Self::MachineHalt,
            omega_abstract_operations::AbstractOperationKind::MemoryFence(kind) => {
                Self::MemoryFence(*kind)
            }
            omega_abstract_operations::AbstractOperationKind::InterruptControl(kind) => {
                Self::InterruptControl(*kind)
            }
            omega_abstract_operations::AbstractOperationKind::FlagsSnapshot {
                dest_region,
                dest_byte_offset,
            } => Self::FlagsSnapshot {
                dest_region: *dest_region,
                dest_byte_offset: *dest_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::FlagsRestore { source } => {
                Self::FlagsRestore {
                    source: remap_runtime_value_handle(*source),
                }
            }
            omega_abstract_operations::AbstractOperationKind::MsrRead {
                index,
                dest_region,
                dest_byte_offset,
            } => Self::MsrRead {
                index: remap_runtime_value_handle(*index),
                dest_region: *dest_region,
                dest_byte_offset: *dest_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::MsrWrite { index, value } => {
                Self::MsrWrite {
                    index: remap_runtime_value_handle(*index),
                    value: remap_runtime_value_handle(*value),
                }
            }
            omega_abstract_operations::AbstractOperationKind::ControlRegisterRead {
                register,
                dest_region,
                dest_byte_offset,
            } => Self::ControlRegisterRead {
                register: *register,
                dest_region: *dest_region,
                dest_byte_offset: *dest_byte_offset,
            },
            omega_abstract_operations::AbstractOperationKind::ControlRegisterWrite {
                register,
                source,
            } => Self::ControlRegisterWrite {
                register: *register,
                source: remap_runtime_value_handle(*source),
            },
            omega_abstract_operations::AbstractOperationKind::PortWrite { port, value } => {
                Self::PortWrite {
                    port: remap_runtime_value_handle(*port),
                    value: remap_runtime_value_handle(*value),
                }
            }
            omega_abstract_operations::AbstractOperationKind::PortRead {
                port,
                dest_region,
                dest_byte_offset,
            } => Self::PortRead {
                port: remap_runtime_value_handle(*port),
                dest_region: *dest_region,
                dest_byte_offset: *dest_byte_offset,
            },
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
    psi_arena::Handle::from_parts(handle.arena_index(), handle.generation())
}
