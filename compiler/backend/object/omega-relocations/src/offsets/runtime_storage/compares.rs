use omega_target::Architecture;

pub(crate) fn runtime_storage_compare_right_address_offset(architecture: Architecture) -> usize {
    match architecture {
        Architecture::Aarch64 => 8,
        Architecture::X86_64 => 8,
    }
}
