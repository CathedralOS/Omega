use omega_calling_conventions::HostBindingMechanism;
use omega_target::Architecture;

pub(crate) fn runtime_text_line_read_target_address_offset(
    architecture: Architecture,
    binding: &HostBindingMechanism,
) -> usize {
    omega_instruction_selection::runtime_text_line_read_target_address_offset(architecture, binding)
}

pub(crate) fn runtime_text_line_read_import_call_offset(
    architecture: Architecture,
    selected_text_offset: usize,
) -> usize {
    selected_text_offset
        + omega_instruction_selection::runtime_text_line_read_import_call_offset(architecture)
}
