use omega_target::Architecture;

pub(crate) fn runtime_storage_binary_left_operand_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 10,
    }
}

pub(crate) fn runtime_pointee_binary_left_operand_offset(
    architecture: Architecture,
    field_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_pointee_operand_start_width(
        architecture,
        field_byte_offset,
    )
}

pub(crate) fn runtime_frame_indexed_binary_left_operand_offset(
    architecture: Architecture,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_frame_indexed_integer_write_width(
        architecture,
        element_byte_size,
        field_byte_offset,
        0,
    )
}

pub(crate) fn runtime_frame_base_indexed_binary_left_operand_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_frame_base_indexed_integer_write_width(
        architecture,
        base_byte_offset,
        element_byte_size,
        field_byte_offset,
        0,
    )
}

pub(crate) fn runtime_machine_indexed_integer_runtime_frame_address_offset(
    architecture: Architecture,
    base_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_machine_indexed_integer_runtime_frame_address_offset(
        architecture,
        base_byte_offset,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        runtime_frame_base_indexed_binary_left_operand_offset,
        runtime_storage_binary_left_operand_offset,
    };
    use omega_target::Architecture;

    #[test]
    fn exposes_runtime_storage_write_operand_offsets_by_architecture() {
        assert_eq!(
            runtime_storage_binary_left_operand_offset(Architecture::Aarch64),
            8
        );
        assert_eq!(
            runtime_storage_binary_left_operand_offset(Architecture::X86_64),
            10
        );
        assert_eq!(
            runtime_frame_base_indexed_binary_left_operand_offset(Architecture::X86_64, 4, 8, 12),
            0
        );
    }
}
