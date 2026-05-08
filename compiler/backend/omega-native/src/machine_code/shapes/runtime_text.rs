use crate::machine_code::model::MachineInstructionKind;
use crate::machine_code::widths::{
    runtime_text_buffer_materialize_width, runtime_text_line_read_width,
    runtime_text_literal_append_width, runtime_text_literal_compare_width,
    runtime_text_literal_segment_write_width, runtime_text_literal_write_width,
    runtime_text_storage_compare_width, runtime_text_stored_place_append_width,
    runtime_text_stored_suffix_append_width,
};
use crate::plan::NativePlan;
use crate::state_guards::StateGuardOperator;

pub(super) fn runtime_text_literal_compare_shape(
    native_plan: &NativePlan,
    literal: &str,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLiteralCompare {
            literal: literal.to_owned(),
        },
        runtime_text_literal_compare_width(native_plan.target.architecture, literal),
    )
}

pub(super) fn runtime_text_storage_compare_shape(
    native_plan: &NativePlan,
    source_offset: usize,
    operator: StateGuardOperator,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextStorageCompare {
            source_offset,
            operator,
        },
        runtime_text_storage_compare_width(native_plan.target.architecture),
    )
}

pub(super) fn runtime_text_literal_write_shape(
    native_plan: &NativePlan,
    literal: &str,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLiteralWrite {
            literal: literal.to_owned(),
        },
        runtime_text_literal_write_width(native_plan.target.architecture, literal),
    )
}

pub(super) fn runtime_text_literal_segment_write_shape(
    native_plan: &NativePlan,
    byte_offset: usize,
    literal: &str,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLiteralSegmentWrite {
            byte_offset,
            literal: literal.to_owned(),
        },
        runtime_text_literal_segment_write_width(native_plan.target.architecture, literal),
    )
}

pub(super) fn runtime_text_stored_suffix_append_shape(
    native_plan: &NativePlan,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextStoredSuffixAppend {
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
        },
        runtime_text_stored_suffix_append_width(native_plan.target.architecture),
    )
}

pub(super) fn runtime_text_buffer_materialize_shape(
    native_plan: &NativePlan,
    target_offset: usize,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextBufferMaterialize { target_offset },
        runtime_text_buffer_materialize_width(native_plan.target.architecture),
    )
}

pub(super) fn runtime_text_stored_place_append_shape(
    native_plan: &NativePlan,
    source_offset: usize,
    target_offset: usize,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextStoredPlaceAppend {
            source_offset,
            target_offset,
        },
        runtime_text_stored_place_append_width(native_plan.target.architecture),
    )
}

pub(super) fn runtime_text_literal_append_shape(
    native_plan: &NativePlan,
    target_offset: usize,
    literal: &str,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLiteralAppend {
            target_offset,
            literal: literal.to_owned(),
        },
        runtime_text_literal_append_width(native_plan.target.architecture, literal),
    )
}

pub(super) fn runtime_text_line_read_shape(
    native_plan: &NativePlan,
    target_offset: usize,
    byte_capacity: usize,
    syscall_number: u32,
    syscall_number_register: u8,
    supervisor_call: u16,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLineRead {
            target_offset,
            byte_capacity,
            syscall_number,
            syscall_number_register,
            supervisor_call,
        },
        runtime_text_line_read_width(
            native_plan.target.architecture,
            byte_capacity,
            syscall_number,
        ),
    )
}
