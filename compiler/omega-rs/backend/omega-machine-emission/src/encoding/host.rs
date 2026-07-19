use crate::MachineEmissionContext;
use omega_assigned_target_operations::InstructionOperand;
use omega_calling_conventions::{HostBindingMechanism, HostOperationKey};
use omega_core::diagnostics::Diagnostic;
use omega_instruction_selection as architecture;

use crate::host_bindings::host_binding_mechanism;

pub(super) fn encode_host_operation(
    input: MachineEmissionContext<'_>,
    operation_key: HostOperationKey,
    operands: &[InstructionOperand],
) -> Result<Vec<u8>, Diagnostic> {
    match host_binding_mechanism(input, operation_key) {
        Some(HostBindingMechanism::Syscall { number, .. }) => architecture::encode_syscall_sequence(
            input.target.architecture,
            operands,
            *number,
        ),
        Some(HostBindingMechanism::VtableSlot { index }) => {
            architecture::encode_vtable_call_sequence(input.target.architecture, operands, *index)
        }
        Some(HostBindingMechanism::VtableField {
            byte_offset,
            parameter_count,
            ..
        }) => architecture::encode_vtable_call_sequence_at_offset(
            input.target.architecture,
            operands,
            *byte_offset,
            field_model_result_present(operands.len(), *parameter_count, "vtable-field")?,
        ),
        Some(HostBindingMechanism::TableFunction {
            byte_offset,
            parameter_count,
            ..
        }) => architecture::encode_table_function_call_sequence(
            input.target.architecture,
            operands,
            *byte_offset,
            field_model_result_present(operands.len(), *parameter_count, "table-function")?,
        ),
        Some(HostBindingMechanism::Import { .. })
            if matches!(
                operation_key.capability,
                omega_calling_conventions::HostCapability::Custom(_)
                    | omega_calling_conventions::HostCapability::Unknown
            ) =>
        {
            architecture::encode_authored_import_call_sequence(
                input.target.architecture,
                operation_key,
                operands,
            )
        }
        _ => architecture::encode_host_call_sequence(
            input.target.architecture,
            operation_key,
            operands,
        ),
    }
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
