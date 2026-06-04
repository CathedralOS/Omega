use omega_calling_conventions::HostBindingMechanism;
use omega_core::diagnostics::Diagnostic;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;

pub fn encode_runtime_text_literal_compare(
    architecture: Architecture,
    literal: &str,
    failure_branch_distances: impl ExactSizeIterator<Item = isize>,
    delimiter_failure_branch_distance: isize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_compare(
            literal,
            failure_branch_distances,
            delimiter_failure_branch_distance,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_text_literal_compare(
            literal,
            failure_branch_distances,
            delimiter_failure_branch_distance,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn encode_runtime_text_storage_compare_bytes(
    architecture: Architecture,
    source_offset: usize,
    literal_len: usize,
    compare_failure_branch_distance: isize,
    delimiter_failure_branch_distance: isize,
    branch_when_equal: bool,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            let _ = literal_len;
            aarch64::encode_runtime_text_storage_compare_bytes(
                source_offset,
                compare_failure_branch_distance,
                delimiter_failure_branch_distance,
                branch_when_equal,
            )
        }
        Architecture::X86_64 => x86_64::encode_runtime_text_storage_compare_bytes(
            source_offset,
            literal_len,
            compare_failure_branch_distance,
            branch_when_equal,
        ),
    }
}

pub fn encode_runtime_text_literal_write(
    architecture: Architecture,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_write(literal),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_text_literal_segment_write(
    architecture: Architecture,
    byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_segment_write(byte_offset, literal)
        }
        Architecture::X86_64 => {
            x86_64::encode_runtime_text_literal_segment_write(byte_offset, literal)
        }
    }
}

pub fn encode_runtime_text_stored_suffix_append(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
    length_delta: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_stored_suffix_append(
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
        ),
        Architecture::X86_64 => x86_64::encode_runtime_text_stored_suffix_append(
            buffer_offset,
            source_offset,
            target_offset,
            length_delta,
        ),
    }
}

pub fn encode_runtime_text_stored_place_append(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_stored_place_append(
            buffer_offset,
            source_offset,
            target_offset,
        ),
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_stored_place_append(source_offset, target_offset)
        }
    }
}

pub fn encode_runtime_text_stored_place_append_to_runtime_pointee(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_stored_place_append_to_runtime_pointee(
                buffer_offset,
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_stored_place_append_to_runtime_pointee(
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
            )
        }
    }
}

pub fn encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
    architecture: Architecture,
    buffer_offset: usize,
    source_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_stored_place_append_to_runtime_frame_indexed(
                buffer_offset,
                source_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_text_literal_append(
    architecture: Architecture,
    buffer_offset: usize,
    target_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_append(buffer_offset, target_offset, literal)
        }
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_literal_append(target_offset, literal)
        }
    }
}

pub fn encode_runtime_text_literal_append_to_runtime_pointee(
    architecture: Architecture,
    buffer_offset: usize,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_literal_append_to_runtime_pointee(
            buffer_offset,
            pointer_byte_offset,
            field_byte_offset,
            literal,
        ),
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_literal_append_to_runtime_pointee(
                pointer_byte_offset,
                field_byte_offset,
                literal,
            )
        }
    }
}

pub fn encode_runtime_text_literal_append_to_runtime_frame_indexed(
    architecture: Architecture,
    buffer_offset: usize,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
    literal: &str,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_literal_append_to_runtime_frame_indexed(
                buffer_offset,
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                literal,
            )
        }
        Architecture::X86_64 => {
            let _ = buffer_offset;
            x86_64::encode_runtime_text_literal_append_to_runtime_frame_indexed(
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
                literal,
            )
        }
    }
}

pub fn encode_runtime_text_buffer_materialize(
    architecture: Architecture,
    target_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => aarch64::encode_runtime_text_buffer_materialize(target_offset),
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_pointee(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_buffer_materialize_to_runtime_pointee(
                pointer_byte_offset,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
    architecture: Architecture,
    descriptor_offset: usize,
    index_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
                descriptor_offset,
                index_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

pub fn encode_runtime_text_line_read(
    architecture: Architecture,
    target_offset: usize,
    byte_capacity: usize,
    binding: &HostBindingMechanism,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                aarch64::encode_runtime_text_line_read_import(target_offset, byte_capacity)
            }
            HostBindingMechanism::Syscall {
                number,
                number_register,
                supervisor_call,
                ..
            } => aarch64::encode_runtime_text_line_read_syscall(
                target_offset,
                byte_capacity,
                *number,
                *number_register,
                *supervisor_call,
            ),
        },
        Architecture::X86_64 => x86_64::encode_runtime_text_line_read(target_offset, byte_capacity),
    }
}

fn unsupported_x86_64_encoding() -> Result<Vec<u8>, Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 runtime text encoding is not implemented",
    ))
}

fn unsupported_x86_64_runtime_text_storage_compare_encoding() -> Result<[u8; 84], Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 runtime text encoding is not implemented",
    ))
}
