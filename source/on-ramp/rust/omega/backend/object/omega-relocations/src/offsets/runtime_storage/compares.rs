use omega_target::Architecture;

pub(crate) fn runtime_storage_compare_right_address_offset(
    architecture: Architecture,
    byte_size: usize,
) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        // mov r15,imm64 (10) + load r10,[r15+disp32] (7, or 8 for the
        // 0x66-prefixed 2-byte load) puts the right operand's `mov r15,imm64`
        // at 17 (18 for 2-byte operands).
        Architecture::X86_64 => {
            if byte_size == 2 {
                18
            } else {
                17
            }
        }
    }
}
