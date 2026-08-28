use crate::MachineEmissionContext;
use omega_assigned_target_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    CallPlan, HostBinding, HostBindingMechanism, HostOperation, HostOperationKey,
};
use omega_target::{Architecture, ObjectFormat};
use psi_diagnostics::Diagnostic;

pub(super) fn field_model_result_present(
    operand_count: usize,
    plan: &CallPlan,
    dispatch_only_operand_count: usize,
    label: &str,
) -> Result<bool, Diagnostic> {
    let parameter_operand_count = plan
        .parameters
        .len()
        .checked_add(dispatch_only_operand_count)
        .ok_or_else(|| Diagnostic::error(format!("{label} operand count overflowed")))?;
    if operand_count == parameter_operand_count {
        Ok(false)
    } else if operand_count == parameter_operand_count + 1 {
        Ok(true)
    } else {
        Err(Diagnostic::error(format!(
            "cannot encode {label} call: {operand_count} operand(s) for {} retained wire parameter(s) and {dispatch_only_operand_count} dispatch-only operand(s)",
            plan.parameters.len()
        )))
    }
}

/// Whether a retained native call result is also represented by a leading
/// Omega result operand. Some statement-shaped adapters (notably console
/// writes) retain the native status/count in the ABI plan for validation but
/// intentionally discard it at the language boundary.
pub(super) fn omega_result_present(operation_key: HostOperationKey, plan: &CallPlan) -> bool {
    plan.result.is_some() && !operation_key.discards_native_result()
}

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
        .map(HostBinding::call_plan)
        .map(Some)
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "Win64 runtime text adapter for {}.{} has no retained GetStdHandle plan",
                operation_key.capability_name(),
                operation_key.operation_name()
            ))
        })
}

pub(super) fn runtime_text_call_plans<'plan>(
    input: MachineEmissionContext<'plan>,
    operation_key: HostOperationKey,
    binding: &'plan HostBinding,
) -> Result<omega_instruction_selection::RuntimeTextCallPlans<'plan>, Diagnostic> {
    let operation_plan = binding.call_plan();
    match windows_get_std_handle_plan(input, operation_key)? {
        Some(get_std_handle) => Ok(
            omega_instruction_selection::RuntimeTextCallPlans::WindowsFileAdapter {
                get_std_handle,
                file_io: operation_plan,
            },
        ),
        None => Ok(omega_instruction_selection::RuntimeTextCallPlans::Direct(
            operation_plan,
        )),
    }
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

#[cfg(test)]
mod tests {
    use super::{field_model_result_present, omega_result_present};
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, HostCapability, HostOperation, HostOperationKey, ValueShape,
        evaluate_call_plan,
    };

    fn one_parameter_result_plan() -> omega_calling_conventions::CallPlan {
        evaluate_call_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .expect("one-parameter call plan")
    }

    #[test]
    fn field_result_detection_comes_from_the_retained_wire_signature() {
        let plan = one_parameter_result_plan();
        assert_eq!(
            field_model_result_present(1, &plan, 0, "vtable").expect("statement vtable call"),
            false
        );
        assert_eq!(
            field_model_result_present(2, &plan, 0, "vtable").expect("value vtable call"),
            true
        );
        assert_eq!(
            field_model_result_present(2, &plan, 1, "table").expect("statement table call"),
            false
        );
        assert_eq!(
            field_model_result_present(3, &plan, 1, "table").expect("value table call"),
            true
        );
        assert!(field_model_result_present(4, &plan, 1, "table").is_err());
    }

    #[test]
    fn discarded_native_result_is_not_an_omega_result_operand() {
        let plan = one_parameter_result_plan();
        assert!(!omega_result_present(
            HostOperationKey::new(HostCapability::Stdout, HostOperation::Write),
            &plan,
        ));
        assert!(omega_result_present(
            HostOperationKey::new(HostCapability::Filesystem, HostOperation::Open),
            &plan,
        ));
    }
}
