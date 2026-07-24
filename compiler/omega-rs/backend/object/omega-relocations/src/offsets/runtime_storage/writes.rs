use omega_target::Architecture;

pub(crate) fn runtime_storage_binary_left_operand_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 10,
    }
}

pub(crate) fn runtime_pointee_binary_left_operand_offset(
    architecture: Architecture,
    pointer_byte_offset: usize,
    field_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_pointee_operand_start_width(
        architecture,
        pointer_byte_offset,
        field_byte_offset,
    )
}

pub(crate) fn runtime_frame_indexed_binary_left_operand_offset(
    architecture: Architecture,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_frame_indexed_binary_left_operand_offset(
        architecture,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
    )
}

pub(crate) fn runtime_frame_base_indexed_binary_left_operand_offset(
    architecture: Architecture,
    base_byte_offset: usize,
    index_offset: usize,
    index_byte_size: usize,
    element_byte_size: usize,
    field_byte_offset: usize,
) -> usize {
    omega_instruction_selection::runtime_frame_base_indexed_binary_left_operand_offset(
        architecture,
        base_byte_offset,
        index_offset,
        index_byte_size,
        element_byte_size,
        field_byte_offset,
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
        // Binary rung 1b (the place materializer's canonicalized prefix):
        // mov r15,imm64 (10) + mov r11d,[r15+idx] (7, 32-bit ZX) + imul (7)
        // + add r15,r11 (3) + mov r14,r15 (3).
        assert_eq!(
            runtime_frame_base_indexed_binary_left_operand_offset(
                Architecture::X86_64,
                4,
                8,
                4,
                12,
                0
            ),
            30
        );
    }
}
