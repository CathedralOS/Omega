use omega_calling_conventions::HostBindingMechanism;
use omega_target::Architecture;
use omega_target_operations::RuntimeTextReadTarget;

pub(crate) fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    binding: &HostBindingMechanism,
) -> usize {
    omega_instruction_selection::runtime_text_line_read_target_address_offset(architecture, binding)
}

pub(crate) fn runtime_text_line_read_import_call_offset(
    architecture: Architecture,
    selected_text_offset: usize,
    target: RuntimeTextReadTarget,
    target_offset: usize,
) -> usize {
    selected_text_offset
        + omega_instruction_selection::runtime_text_line_read_import_call_offset(
            architecture,
            target,
            target_offset,
        )
}

/// x86_64-only: absolute text offset of the GetStdHandle call rel32 displacement
/// (the stdin handle acquisition that precedes ReadFile). aarch64 returns the
/// instruction start (it has no separate handle call; callers gate on the
/// Import binding mechanism only for x86_64).
pub(crate) fn runtime_text_line_read_get_std_handle_call_offset(
    architecture: Architecture,
    selected_text_offset: usize,
    target: RuntimeTextReadTarget,
) -> usize {
    selected_text_offset
        + omega_instruction_selection::runtime_text_line_read_get_std_handle_call_offset(
            architecture,
            target,
        )
}
