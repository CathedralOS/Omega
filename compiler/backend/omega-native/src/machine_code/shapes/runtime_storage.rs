use crate::plan::NativePlan;
use crate::state_guards::StateGuardOperator;
use omega_instruction_selection::{
    runtime_machine_integer_write_width, runtime_machine_string_write_width,
    runtime_storage_compare_width, runtime_storage_copy_width, runtime_storage_value_compare_width,
};
use omega_machine_program::MachineInstructionKind;

pub(super) fn runtime_storage_compare_shape(
    native_plan: &NativePlan,
    left_offset: usize,
    right_offset: usize,
    byte_size: usize,
    operator: StateGuardOperator,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeStorageCompare {
            left_offset,
            right_offset,
            byte_size,
            operator,
        },
        runtime_storage_compare_width(native_plan.target.architecture),
    )
}

pub(super) fn runtime_storage_value_compare_shape(
    native_plan: &NativePlan,
    byte_offset: usize,
    byte_size: usize,
    expected_value: i64,
    operator: StateGuardOperator,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeStorageValueCompare {
            byte_offset,
            byte_size,
            expected_value,
            operator,
        },
        runtime_storage_value_compare_width(native_plan.target.architecture),
    )
}

pub(super) fn runtime_machine_integer_write_shape(
    native_plan: &NativePlan,
    byte_offset: usize,
    byte_size: usize,
    value: i64,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeMachineIntegerWrite {
            byte_offset,
            byte_size,
            value,
        },
        runtime_machine_integer_write_width(native_plan.target.architecture),
    )
}

pub(super) fn runtime_machine_string_write_shape(
    native_plan: &NativePlan,
    byte_offset: usize,
    byte_length: usize,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeMachineStringWrite {
            byte_offset,
            byte_length,
        },
        runtime_machine_string_write_width(native_plan.target.architecture, byte_length),
    )
}

pub(super) fn runtime_storage_copy_shape(
    native_plan: &NativePlan,
    source_offset: usize,
    target_offset: usize,
    byte_count: usize,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeStorageCopy {
            source_offset,
            target_offset,
            byte_count,
        },
        runtime_storage_copy_width(native_plan.target.architecture, byte_count),
    )
}
