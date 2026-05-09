use crate::TargetToMachineInput;
use omega_instruction_selection::{
    runtime_text_buffer_materialize_width, runtime_text_line_read_width,
    runtime_text_literal_append_width, runtime_text_literal_compare_width,
    runtime_text_literal_segment_write_width, runtime_text_literal_write_width,
    runtime_text_storage_compare_width, runtime_text_stored_place_append_width,
    runtime_text_stored_suffix_append_width,
};
use omega_machine_program::{MachineInstructionKind, MachineRuntimeTextReadSource};
use omega_target_program::{RuntimeTextReadSource, StateGuardOperator};

pub(super) fn runtime_text_literal_compare_shape(
    input: TargetToMachineInput<'_>,
    literal: &str,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLiteralCompare {
            literal: literal.to_owned(),
        },
        runtime_text_literal_compare_width(input.target.architecture, literal),
    )
}

pub(super) fn runtime_text_storage_compare_shape(
    input: TargetToMachineInput<'_>,
    source_offset: usize,
    operator: StateGuardOperator,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextStorageCompare {
            source_offset,
            operator,
        },
        runtime_text_storage_compare_width(input.target.architecture),
    )
}

pub(super) fn runtime_text_literal_write_shape(
    input: TargetToMachineInput<'_>,
    literal: &str,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLiteralWrite {
            literal: literal.to_owned(),
        },
        runtime_text_literal_write_width(input.target.architecture, literal),
    )
}

pub(super) fn runtime_text_literal_segment_write_shape(
    input: TargetToMachineInput<'_>,
    byte_offset: usize,
    literal: &str,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLiteralSegmentWrite {
            byte_offset,
            literal: literal.to_owned(),
        },
        runtime_text_literal_segment_write_width(input.target.architecture, literal),
    )
}

pub(super) fn runtime_text_stored_suffix_append_shape(
    input: TargetToMachineInput<'_>,
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
        runtime_text_stored_suffix_append_width(input.target.architecture),
    )
}

pub(super) fn runtime_text_buffer_materialize_shape(
    input: TargetToMachineInput<'_>,
    target_offset: usize,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextBufferMaterialize { target_offset },
        runtime_text_buffer_materialize_width(input.target.architecture),
    )
}

pub(super) fn runtime_text_stored_place_append_shape(
    input: TargetToMachineInput<'_>,
    source_offset: usize,
    target_offset: usize,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextStoredPlaceAppend {
            source_offset,
            target_offset,
        },
        runtime_text_stored_place_append_width(input.target.architecture),
    )
}

pub(super) fn runtime_text_literal_append_shape(
    input: TargetToMachineInput<'_>,
    target_offset: usize,
    literal: &str,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLiteralAppend {
            target_offset,
            literal: literal.to_owned(),
        },
        runtime_text_literal_append_width(input.target.architecture, literal),
    )
}

pub(super) fn runtime_text_line_read_shape(
    input: TargetToMachineInput<'_>,
    target_offset: usize,
    byte_capacity: usize,
    source: &RuntimeTextReadSource,
) -> (MachineInstructionKind, usize) {
    (
        MachineInstructionKind::RuntimeTextLineRead {
            target_offset,
            byte_capacity,
            source: lower_runtime_text_read_source(source),
        },
        runtime_text_line_read_width(input.target.architecture, byte_capacity, source),
    )
}

fn lower_runtime_text_read_source(source: &RuntimeTextReadSource) -> MachineRuntimeTextReadSource {
    match source {
        RuntimeTextReadSource::Import { symbol } => MachineRuntimeTextReadSource::Import {
            symbol: symbol.clone(),
        },
        RuntimeTextReadSource::Syscall {
            number,
            number_register,
            supervisor_call,
        } => MachineRuntimeTextReadSource::Syscall {
            number: *number,
            number_register: *number_register,
            supervisor_call: *supervisor_call,
        },
    }
}
