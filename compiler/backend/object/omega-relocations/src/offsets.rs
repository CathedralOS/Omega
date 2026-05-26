use omega_calling_conventions::HostBindingMechanism;
use omega_calling_conventions::HostOperationKey;
use omega_object_file::RelocationKind;
use omega_target::Architecture;
use omega_target_operations::InstructionOperandLike;

pub(super) fn external_call_relocation_offset<T: InstructionOperandLike>(
    architecture: Architecture,
    operation_key: HostOperationKey,
    selected_text_offset: usize,
    operands: &[T],
) -> usize {
    if architecture == Architecture::X86_64
        && let Some(site) =
            omega_isa_x86_64::host_call_external_relocation_site(operation_key, operands)
    {
        return selected_text_offset + site.byte_offset;
    }

    let operand_bytes = operands
        .iter()
        .map(|operand| omega_instruction_selection::operand_width(architecture, operand))
        .sum::<usize>();

    selected_text_offset
        + operand_bytes
        + match architecture {
            Architecture::Aarch64 => 0,
            Architecture::X86_64 => 1,
        }
}

pub(super) fn string_descriptor_machine_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 10,
    }
}

pub(super) fn string_descriptor_pointee_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_storage_copy_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 10,
    }
}

pub(super) fn runtime_storage_copy_from_runtime_frame_indexed_target_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_storage_copy_from_runtime_frame_fixed_indexed_target_address_offset(
    architecture: Architecture,
    element_index: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            let source_offset = element_index
                .saturating_mul(element_byte_size)
                .saturating_add(field_byte_offset);
            12 + add_constant_width(source_offset)
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_storage_copy_from_runtime_machine_indexed_runtime_frame_address_offset(
                architecture,
                base_byte_offset,
            )
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_storage_copy_from_runtime_machine_indexed_target_address_offset(
                architecture,
                base_byte_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_storage_compare_right_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_stored_suffix_source_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_stored_suffix_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 52,
        Architecture::X86_64 => 16,
    }
}

pub(super) fn runtime_text_stored_place_source_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 28,
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_stored_place_target_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_literal_append_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_buffer_materialize_target_address_offset(
    architecture: Architecture,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_indexed_literal_append_buffer_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 4
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_indexed_stored_place_buffer_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 8
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_indexed_stored_place_source_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 24
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_text_indexed_buffer_materialize_buffer_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset) + 12
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_frame_indexed_string_data_address_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            runtime_frame_index_setup_width(element_byte_size, field_byte_offset)
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_machine_indexed_string_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_machine_indexed_string_runtime_frame_address_offset(
                architecture,
                base_byte_offset,
            )
        }
        Architecture::X86_64 => 8,
    }
}

pub(super) fn runtime_machine_indexed_string_data_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => {
            omega_instruction_selection::runtime_machine_indexed_string_data_address_offset(
                architecture,
                base_byte_offset,
                element_byte_size,
                field_byte_offset,
            )
        }
        Architecture::X86_64 => 8,
    }
}

fn runtime_frame_index_setup_width(element_byte_size: usize, field_byte_offset: usize) -> usize {
    match element_byte_size {
        0 => 20 + add_constant_width(field_byte_offset),
        _ => 20 + scale_index_width(element_byte_size) + add_constant_width(field_byte_offset),
    }
}

fn scale_index_width(element_byte_size: usize) -> usize {
    let highest_bit = usize::BITS - element_byte_size.leading_zeros();
    let doubles = highest_bit.saturating_sub(1) as usize;
    let additions = element_byte_size.count_ones() as usize;
    8 + (doubles + additions) * 4
}

fn add_constant_width(value: usize) -> usize {
    if value == 0 {
        0
    } else if value <= 4095 {
        4
    } else {
        unsigned_immediate_width(value as u64) + 4
    }
}

fn unsigned_immediate_width(value: u64) -> usize {
    if value == 0 {
        return 4;
    }

    let mut width = 0;
    for halfword_index in 0..4 {
        if ((value >> (halfword_index * 16)) & 0xffff) != 0 {
            width += 4;
        }
    }
    width
}

pub(super) fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    binding: &HostBindingMechanism,
) -> usize {
    omega_instruction_selection::runtime_text_line_read_target_address_offset(architecture, binding)
}

pub(super) fn runtime_text_line_read_import_call_offset(
    architecture: Architecture,
    selected_text_offset: usize,
) -> usize {
    selected_text_offset
        + omega_instruction_selection::runtime_text_line_read_import_call_offset(architecture)
}

pub(super) fn external_call_relocation_width(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 4,
        Architecture::X86_64 => 4,
    }
}

pub(super) fn external_call_relocation_kind(architecture: Architecture) -> RelocationKind {
    match architecture {
        Architecture::Aarch64 => RelocationKind::Aarch64Branch26,
        Architecture::X86_64 => RelocationKind::X86_64Relative32,
    }
}
