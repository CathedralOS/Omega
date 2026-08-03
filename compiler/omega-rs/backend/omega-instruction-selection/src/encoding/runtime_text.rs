use omega_calling_conventions::HostBindingMechanism;
use omega_isa_aarch64::aarch64;
use omega_isa_x86_64 as x86_64;
use omega_target::Architecture;
use omega_target_operations::RuntimeTextReadTarget;
use psi_diagnostics::Diagnostic;

use super::host::normalized_syscall_registers;

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
        Architecture::Aarch64 => aarch64::encode_runtime_text_storage_compare_bytes(
            source_offset,
            literal_len,
            compare_failure_branch_distance,
            delimiter_failure_branch_distance,
            branch_when_equal,
        ),
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
    index_byte_size: usize,
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
                index_byte_size,
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
    index_byte_size: usize,
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
                index_byte_size,
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
                index_byte_size,
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
        Architecture::X86_64 => x86_64::encode_runtime_text_buffer_materialize(target_offset),
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
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => {
            aarch64::encode_runtime_text_buffer_materialize_to_runtime_frame_indexed(
                descriptor_offset,
                index_offset,
                index_byte_size,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => unsupported_x86_64_encoding(),
    }
}

/// One stdin byte into a `ByteRead` sum slot (std console `read_byte()`).
/// X86_64 is not encoded yet (TASKS_FS #0a follow-up) -- loud by doctrine.
pub fn encode_runtime_byte_read(
    architecture: Architecture,
    target_offset: usize,
    payload_offset: usize,
    binding: &HostBindingMechanism,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                aarch64::encode_runtime_byte_read_import(target_offset, payload_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers(architecture, 3, true)?;
                aarch64::encode_runtime_byte_read_syscall(
                    target_offset,
                    payload_offset,
                    *number,
                    &registers.parameters,
                    registers.required_result()?,
                    registers.number,
                    registers.immediate,
                )
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("read_byte cannot be vtable-bound"))
            }
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => {
                x86_64::encode_runtime_byte_read_import(target_offset, payload_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers(architecture, 3, true)?;
                x86_64::encode_runtime_byte_read_syscall(
                    target_offset,
                    payload_offset,
                    *number,
                    &registers.parameters,
                    registers.required_result()?,
                    registers.number,
                    registers.immediate,
                )
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("read_byte cannot be vtable-bound"))
            }
        },
    }
}

/// One byte to stdout (std console `write_byte(b)`); same conventions as
/// the read.
pub fn encode_runtime_byte_write(
    architecture: Architecture,
    source_offset: usize,
    binding: &HostBindingMechanism,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => {
                aarch64::encode_runtime_byte_write_import(source_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers(architecture, 3, true)?;
                aarch64::encode_runtime_byte_write_syscall(
                    source_offset,
                    *number,
                    &registers.parameters,
                    registers.required_result()?,
                    registers.number,
                    registers.immediate,
                )
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("write_byte cannot be vtable-bound"))
            }
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => {
                x86_64::encode_runtime_byte_write_import(source_offset)
            }
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers(architecture, 3, true)?;
                x86_64::encode_runtime_byte_write_syscall(
                    source_offset,
                    *number,
                    &registers.parameters,
                    registers.required_result()?,
                    registers.number,
                    registers.immediate,
                )
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("write_byte cannot be vtable-bound"))
            }
        },
    }
}

pub fn encode_runtime_text_line_read(
    architecture: Architecture,
    target_offset: usize,
    byte_capacity: usize,
    binding: &HostBindingMechanism,
    target: RuntimeTextReadTarget,
) -> Result<Vec<u8>, Diagnostic> {
    match architecture {
        Architecture::Aarch64 => match binding {
            HostBindingMechanism::Import { .. } => match target {
                RuntimeTextReadTarget::BoundedByteBuffer => {
                    aarch64::encode_runtime_text_line_read_carrier_import(
                        target_offset,
                        byte_capacity,
                    )
                }
                RuntimeTextReadTarget::FixedByteArray => {
                    aarch64::encode_runtime_text_line_read_fixed_array_import(
                        target_offset,
                        byte_capacity,
                    )
                }
                RuntimeTextReadTarget::StringDescriptor => {
                    aarch64::encode_runtime_text_line_read_import(target_offset, byte_capacity)
                }
            },
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers(architecture, 3, true)?;
                let result_register = registers.required_result()?;
                match target {
                    RuntimeTextReadTarget::BoundedByteBuffer => {
                        aarch64::encode_runtime_text_line_read_carrier_syscall(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                    RuntimeTextReadTarget::FixedByteArray => {
                        aarch64::encode_runtime_text_line_read_fixed_array_syscall(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                    RuntimeTextReadTarget::StringDescriptor => {
                        aarch64::encode_runtime_text_line_read_syscall(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                }
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("read_line cannot be vtable-bound"))
            }
        },
        Architecture::X86_64 => match binding {
            HostBindingMechanism::Import { .. } => match target {
                RuntimeTextReadTarget::BoundedByteBuffer => {
                    x86_64::encode_runtime_text_line_read_carrier(target_offset, byte_capacity)
                }
                RuntimeTextReadTarget::FixedByteArray => {
                    x86_64::encode_runtime_text_line_read_fixed_array(target_offset, byte_capacity)
                }
                RuntimeTextReadTarget::StringDescriptor => {
                    x86_64::encode_runtime_text_line_read(target_offset, byte_capacity)
                }
            },
            HostBindingMechanism::Syscall { number, .. } => {
                let registers = normalized_syscall_registers(architecture, 3, true)?;
                let result_register = registers.required_result()?;
                match target {
                    RuntimeTextReadTarget::BoundedByteBuffer => {
                        x86_64::encode_runtime_text_line_read_syscall_carrier(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                    RuntimeTextReadTarget::FixedByteArray => {
                        x86_64::encode_runtime_text_line_read_syscall_fixed_array(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                    RuntimeTextReadTarget::StringDescriptor => {
                        x86_64::encode_runtime_text_line_read_syscall(
                            target_offset,
                            byte_capacity,
                            *number,
                            &registers.parameters,
                            result_register,
                            registers.number,
                            registers.immediate,
                        )
                    }
                }
            }
            HostBindingMechanism::VtableSlot { .. }
            | HostBindingMechanism::VtableField { .. }
            | HostBindingMechanism::TableFunction { .. } => {
                Err(Diagnostic::error("read_line cannot be vtable-bound"))
            }
        },
    }
}

fn unsupported_x86_64_encoding() -> Result<Vec<u8>, Diagnostic> {
    Err(Diagnostic::error(
        "X86_64 runtime text encoding is not implemented",
    ))
}
