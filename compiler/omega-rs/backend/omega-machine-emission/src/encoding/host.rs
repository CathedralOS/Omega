use crate::MachineEmissionContext;
use omega_assigned_target_operations::InstructionOperand;
use omega_calling_conventions::{HostBindingMechanism, HostOperationKey};
use omega_instruction_selection as architecture;
use psi_diagnostics::Diagnostic;

use crate::host_bindings::host_binding;

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
        Some(HostBindingMechanism::Syscall { number, .. })
            if operation_key.uses_linux_timespec_result() =>
        {
            architecture::encode_linux_timespec_syscall_with_plan(
                input.target.architecture,
                operands,
                *number,
                Some(required_syscall_call_plan(binding)?),
            )
        }
        Some(HostBindingMechanism::Syscall { number, .. })
            if operation_key.uses_linux_timespec_argument() =>
        {
            architecture::encode_linux_timespec_argument_syscall_with_plan(
                input.target.architecture,
                operands,
                *number,
                Some(required_syscall_call_plan(binding)?),
            )
        }
        Some(HostBindingMechanism::Syscall { number, .. }) if operation_key.returns_value() => {
            architecture::encode_value_syscall_sequence_with_plan(
                input.target.architecture,
                operands,
                *number,
                Some(required_syscall_call_plan(binding)?),
            )
        }
        Some(HostBindingMechanism::Syscall { number, .. }) => {
            architecture::encode_syscall_sequence_with_plan(
                input.target.architecture,
                operands,
                *number,
                Some(required_syscall_call_plan(binding)?),
            )
        }
        Some(HostBindingMechanism::VtableSlot { index }) => {
            architecture::encode_vtable_call_sequence_with_plan(
                input.target,
                operands,
                *index,
                binding
                    .and_then(omega_calling_conventions::HostBinding::call_plan)
                    .ok_or_else(|| {
                        Diagnostic::error("selected vtable-slot binding has no evaluated call plan")
                    })?,
            )
        }
        Some(HostBindingMechanism::VtableField {
            byte_offset,
            parameter_count,
            ..
        }) => architecture::encode_vtable_call_sequence_at_offset_with_plan(
            input.target,
            operands,
            *byte_offset,
            field_model_result_present(operands.len(), *parameter_count, "vtable-field")?,
            binding
                .and_then(omega_calling_conventions::HostBinding::call_plan)
                .ok_or_else(|| {
                    Diagnostic::error("selected vtable-field binding has no evaluated call plan")
                })?,
        ),
        Some(HostBindingMechanism::TableFunction {
            byte_offset,
            parameter_count,
            ..
        }) => architecture::encode_table_function_call_sequence_with_plan(
            input.target,
            operands,
            *byte_offset,
            field_model_result_present(operands.len(), *parameter_count, "table-function")?,
            binding
                .and_then(omega_calling_conventions::HostBinding::call_plan)
                .ok_or_else(|| {
                    Diagnostic::error("selected table-function binding has no evaluated call plan")
                })?,
        ),
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
                    .and_then(omega_calling_conventions::HostBinding::call_plan)
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "selected authored-import binding has no evaluated call plan",
                        )
                    })?,
            )
        }
        Some(HostBindingMechanism::Import { .. }) => {
            architecture::encode_host_call_sequence_with_plan(
                input.target,
                operation_key,
                operands,
                Some(required_import_call_plan(binding)?),
            )
        }
        _ => architecture::encode_host_call_sequence_with_plan(
            input.target,
            operation_key,
            operands,
            binding.and_then(omega_calling_conventions::HostBinding::call_plan),
        ),
    }
}

fn required_import_call_plan(
    binding: Option<&omega_calling_conventions::HostBinding>,
) -> Result<&omega_calling_conventions::CallPlan, Diagnostic> {
    binding
        .and_then(omega_calling_conventions::HostBinding::call_plan)
        .ok_or_else(|| {
            Diagnostic::error("selected built-in import binding has no evaluated call plan")
        })
}

fn required_syscall_call_plan(
    binding: Option<&omega_calling_conventions::HostBinding>,
) -> Result<&omega_calling_conventions::CallPlan, Diagnostic> {
    binding
        .and_then(omega_calling_conventions::HostBinding::call_plan)
        .ok_or_else(|| Diagnostic::error("selected syscall binding has no evaluated call plan"))
}

/// Whether a field-model call's operand list carries a prepended RESULT
/// place: the list is exactly the declared parameters (`_ = ...`, no result)
/// or the declared parameters plus one leading result (`let status = ...`).
/// Anything else means the selection and the binding disagree -- refuse
/// loudly rather than marshal shifted arguments.
fn field_model_result_present(
    operand_count: usize,
    parameter_count: usize,
    label: &str,
) -> Result<bool, Diagnostic> {
    if operand_count == parameter_count {
        Ok(false)
    } else if operand_count == parameter_count + 1 {
        Ok(true)
    } else {
        Err(Diagnostic::error(format!(
            "cannot encode {label} call: {operand_count} operand(s) for {parameter_count} \
             declared parameter(s) -- expected the declared parameters, optionally led by \
             one result place"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::{required_import_call_plan, required_syscall_call_plan};

    #[test]
    fn built_in_import_cannot_reconstruct_a_missing_plan() {
        let binding = omega_calling_conventions::HostBinding::default();
        let error = required_import_call_plan(Some(&binding))
            .expect_err("an import without a retained plan must reject");
        assert_eq!(
            error.message,
            "selected built-in import binding has no evaluated call plan"
        );
    }

    #[test]
    fn syscall_cannot_reconstruct_a_missing_plan() {
        let binding = omega_calling_conventions::HostBinding::default();
        let error = required_syscall_call_plan(Some(&binding))
            .expect_err("a syscall without a retained plan must reject");
        assert_eq!(
            error.message,
            "selected syscall binding has no evaluated call plan"
        );
    }
}
