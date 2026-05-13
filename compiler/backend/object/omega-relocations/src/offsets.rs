use omega_object::RelocationKind;
use omega_target::Architecture;
use omega_target_operations::InstructionOperand;

pub(super) fn external_call_relocation_offset(
    architecture: Architecture,
    selected_text_offset: usize,
    operands: &[InstructionOperand],
) -> usize {
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
        Architecture::X86_64 => 8,
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

pub(super) fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    source: &omega_target_operations::RuntimeTextReadSource,
) -> usize {
    omega_instruction_selection::runtime_text_line_read_target_address_offset(architecture, source)
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
