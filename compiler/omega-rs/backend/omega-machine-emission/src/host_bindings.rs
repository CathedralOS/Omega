use crate::MachineEmissionContext;
use omega_assigned_target_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    CallPlan, HostBinding, HostBindingMechanism, HostOperation, HostOperationKey,
};
use omega_target::{Architecture, ObjectFormat};
use psi_diagnostics::Diagnostic;

pub(super) fn host_binding<'plan>(
    input: MachineEmissionContext<'plan>,
    operation_key: HostOperationKey,
) -> Option<&'plan HostBinding> {
    input
        .host_abi
        .bindings
        .iter()
        .find(|(_, binding)| binding.operation_key == operation_key)
        .map(|(_, binding)| binding)
}

pub(super) fn windows_get_std_handle_plan<'plan>(
    input: MachineEmissionContext<'plan>,
    operation_key: HostOperationKey,
) -> Result<Option<&'plan CallPlan>, Diagnostic> {
    if input.target.architecture != Architecture::X86_64
        || input.target.object_format != ObjectFormat::Coff
        || !host_binding(input, operation_key)
            .is_some_and(|binding| matches!(binding.mechanism, HostBindingMechanism::Import { .. }))
    {
        return Ok(None);
    }
    let get_std_handle_key =
        HostOperationKey::new(operation_key.capability, HostOperation::GetStdHandle);
    host_binding(input, get_std_handle_key)
        .and_then(HostBinding::call_plan)
        .map(Some)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "Win64 runtime text adapter for {}.{} has no retained GetStdHandle plan",
                operation_key.capability_name(),
                operation_key.operation_name()
            ))
        })
}

pub(super) fn instruction_requires_float_control_restore(
    input: MachineEmissionContext<'_>,
    instruction: &SelectedInstructionKind,
) -> bool {
    instruction
        .host_operation_key()
        .and_then(|operation_key| host_binding(input, operation_key))
        .is_some_and(|binding| binding.mechanism.requires_float_control_restore())
}
