use crate::abi::HostBindingMechanism;
use crate::architecture;
use crate::instructions::SelectedInstructionKind;
use crate::plan::NativePlan;
use crate::state_guards::{StateGuardLowering, StateGuardOperator};

use super::host_bindings::host_binding_mechanism;
use super::model::MachineInstructionKind;
use super::widths::{
    dispatch_case_enter_width, dispatch_case_leave_width, dispatch_guard_compare_static_width,
    dispatch_loop_enter_width, dispatch_state_write_width, host_call_sequence_width, return_width,
    runtime_machine_integer_write_width, runtime_machine_string_write_width,
    runtime_storage_compare_width, runtime_storage_copy_width, runtime_storage_value_compare_width,
    runtime_text_buffer_materialize_width, runtime_text_line_read_width,
    runtime_text_literal_append_width, runtime_text_literal_compare_width,
    runtime_text_literal_segment_write_width, runtime_text_literal_write_width,
    runtime_text_storage_compare_width, runtime_text_stored_place_append_width,
    runtime_text_stored_suffix_append_width,
};

pub(super) fn machine_instruction_shape(
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
