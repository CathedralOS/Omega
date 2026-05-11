use omega_machine_program::{MachineInstructionKind, MachineRuntimeTextReadSource};
use omega_target_operations::{RuntimeTextReadSource, StateGuardOperator};

pub(super) fn runtime_text_literal_compare_kind(literal: &str) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralCompare {
        literal: literal.to_owned(),
    }
}

pub(super) fn runtime_text_storage_compare_kind(
    source_offset: usize,
    operator: StateGuardOperator,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStorageCompare {
        source_offset,
        operator,
    }
}

pub(super) fn runtime_text_literal_write_kind(literal: &str) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralWrite {
        literal: literal.to_owned(),
    }
}

pub(super) fn runtime_text_literal_segment_write_kind(
    byte_offset: usize,
    literal: &str,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralSegmentWrite {
        byte_offset,
        literal: literal.to_owned(),
    }
}

pub(super) fn runtime_text_stored_suffix_append_kind(
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredSuffixAppend {
        buffer_offset,
        source_offset,
        target_offset,
        length_delta,
    }
}

pub(super) fn runtime_text_buffer_materialize_kind(target_offset: usize) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextBufferMaterialize { target_offset }
}

pub(super) fn runtime_text_stored_place_append_kind(
    source_offset: usize,
    target_offset: usize,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextStoredPlaceAppend {
        source_offset,
        target_offset,
    }
}

pub(super) fn runtime_text_literal_append_kind(
    target_offset: usize,
    literal: &str,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLiteralAppend {
        target_offset,
        literal: literal.to_owned(),
    }
}

pub(super) fn runtime_text_line_read_kind(
    target_offset: usize,
    byte_capacity: usize,
    source: &RuntimeTextReadSource,
) -> MachineInstructionKind {
    MachineInstructionKind::RuntimeTextLineRead {
        target_offset,
        byte_capacity,
        source: lower_runtime_text_read_source(source),
    }
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
