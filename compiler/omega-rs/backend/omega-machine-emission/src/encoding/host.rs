use crate::MachineEmissionContext;
use omega_assigned_target_operations::InstructionOperand;
use omega_calling_conventions::{HostBindingMechanism, HostOperationKey};
use omega_instruction_selection as architecture;
use psi_diagnostics::Diagnostic;

use crate::host_bindings::{field_model_result_present, host_binding, omega_result_present};

pub(super) fn encode_host_operation(
    input: MachineEmissionContext<'_>,
    operation_key: HostOperationKey,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    let binding = host_binding(input, operation_key);
    if binding.is_none() && operation_key.lowers_to_constant_result() {
        return architecture::encode_constant_host_result(input.target.architecture, operands);
    }
    match binding.map(|binding| &binding.mechanism) {
        Some(HostBindingMechanism::Syscall { number, .. }) => {
            let plan = required_syscall_call_plan(binding)?;
            if operation_key.uses_linux_timespec_result() {
                architecture::encode_linux_timespec_syscall_with_plan(
                    input.target.architecture,
                    operands,
                    *number,
                    plan,
                )
            } else if operation_key.uses_linux_timespec_argument() {
                architecture::encode_linux_timespec_argument_syscall_with_plan(
                    input.target.architecture,
                    operands,
                    *number,
                    plan,
                )
            } else if omega_result_present(operation_key, plan) {
                architecture::encode_value_syscall_sequence_with_plan(
                    input.target.architecture,
                    operands,
                    *number,
                    plan,
                )
            } else {
                architecture::encode_syscall_sequence_with_plan(
                    input.target.architecture,
                    operands,
                    *number,
                    plan,
                )
            }
        }
        Some(HostBindingMechanism::VtableSlot { index }) => {
            architecture::encode_vtable_call_sequence_with_plan(
                input.target,
                operands,
                *index,
                binding
                    .map(omega_calling_conventions::HostBinding::call_plan)
                    .expect("the matched vtable-slot binding is plan-bearing"),
            )
        }
        Some(HostBindingMechanism::VtableField { byte_offset, .. }) => {
            let plan = binding
                .map(omega_calling_conventions::HostBinding::call_plan)
                .expect("the matched vtable-field binding is plan-bearing");
            architecture::encode_vtable_call_sequence_at_offset_with_plan(
                input.target,
                operands,
                *byte_offset,
                field_model_result_present(operands.len(), plan, 0, "vtable-field")?,
                plan,
            )
        }
        Some(HostBindingMechanism::TableFunction { byte_offset, .. }) => {
            let plan = binding
                .map(omega_calling_conventions::HostBinding::call_plan)
                .expect("the matched table-function binding is plan-bearing");
            architecture::encode_table_function_call_sequence_with_plan(
                input.target,
                operands,
                *byte_offset,
                field_model_result_present(operands.len(), plan, 1, "table-function")?,
                plan,
            )
        }
        Some(HostBindingMechanism::Import { .. })
            if matches!(
                operation_key.capability,
                omega_calling_conventions::HostCapability::Custom(_)
                    | omega_calling_conventions::HostCapability::Unknown
            ) =>
        {
            architecture::encode_authored_import_call_sequence(
                input.target,
                operands,
                binding
                    .map(omega_calling_conventions::HostBinding::call_plan)
                    .expect("the matched authored-import binding is plan-bearing"),
            )
        }
        Some(HostBindingMechanism::Import { .. }) => {
            architecture::encode_host_call_sequence_with_plan(
                input.target,
                operation_key,
                operands,
                required_import_call_plan(binding)?,
            )
        }
        _ => Err(Diagnostic::error(format!(
            "host operation {}.{} has no selected host binding",
            operation_key.capability_name(),
            operation_key.operation_name(),
        ))),
    }
}

fn required_import_call_plan(
    binding: Option<&omega_calling_conventions::HostBinding>,
) -> Result<&omega_calling_conventions::CallPlan, Diagnostic> {
    binding
        .map(omega_calling_conventions::HostBinding::call_plan)
        .ok_or_else(|| Diagnostic::error("built-in import has no selected host binding"))
}

fn required_syscall_call_plan(
    binding: Option<&omega_calling_conventions::HostBinding>,
) -> Result<&omega_calling_conventions::CallPlan, Diagnostic> {
    binding
        .map(omega_calling_conventions::HostBinding::call_plan)
        .ok_or_else(|| Diagnostic::error("syscall has no selected host binding"))
}

#[cfg(test)]
mod tests {
    use super::{required_import_call_plan, required_syscall_call_plan};

    #[test]
    fn built_in_import_requires_a_selected_binding() {
        let error = required_import_call_plan(None).expect_err("a missing binding must reject");
        assert_eq!(
            error.message,
            "built-in import has no selected host binding"
        );
    }

    #[test]
    fn syscall_requires_a_selected_binding() {
        let error = required_syscall_call_plan(None).expect_err("a missing binding must reject");
        assert_eq!(error.message, "syscall has no selected host binding");
    }
}
