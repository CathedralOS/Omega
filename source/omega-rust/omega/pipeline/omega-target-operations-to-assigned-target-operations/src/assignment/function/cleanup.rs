use super::assign_function;
use crate::assignment::cleanup::{finite_boolean_cleanup_edges, validate_scalar_cleanup_signature};
use crate::assignment::control::assign_boolean_control;
use crate::assignment::shared::*;

pub(super) fn assign(
    function: &TargetFunction,
    operation: &TargetOperation,
    target: NativeTarget,
) -> Result<AssignedOperation, AssignmentError> {
    let architecture = target.architecture;
    Ok(match operation {
        TargetOperation::ScalarReturnWithCleanup {
            scalar,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
            psi_edge,
        } => {
            if matches!(
                scalar.as_ref(),
                TargetOperation::ScalarReturnWithCleanup { .. }
                    | TargetOperation::BooleanControlWithCleanup { .. }
                    | TargetOperation::ReturnBoundaryPortReadU8 { .. }
            ) {
                return Err(AssignmentError::UnsupportedScalarCleanup(function.machine));
            }
            validate_scalar_cleanup_signature(
                function.machine,
                target,
                call_plan,
                structural_parameters,
                cleanup_actions,
            )?;
            let assigned_scalar = assign_function(
                &TargetFunction {
                    machine: function.machine,
                    attachment: function.attachment,
                    fixed_integer_scalar_abi: function.fixed_integer_scalar_abi.clone(),
                    provenance: function.provenance.clone(),
                    operation: scalar.as_ref().clone(),
                },
                target,
            )?
            .operation;
            AssignedOperation::ScalarReturnWithCleanup {
                scalar: Box::new(assigned_scalar),
                structural_types: structural_types.clone(),
                call_plan: call_plan.clone(),
                structural_parameters: structural_parameters.clone(),
                cleanup_actions: cleanup_actions.clone(),
                psi_edge: *psi_edge,
            }
        }
        TargetOperation::BooleanControlWithCleanup {
            control,
            structural_types,
            call_plan,
            structural_parameters,
            cleanup_actions,
        } => {
            validate_scalar_cleanup_signature(
                function.machine,
                target,
                call_plan,
                structural_parameters,
                cleanup_actions,
            )?;
            finite_boolean_cleanup_edges(control)
                .ok_or(AssignmentError::UnsupportedScalarCleanup(function.machine))?;
            AssignedOperation::BooleanControlWithCleanup {
                control: assign_boolean_control(control, architecture)?,
                structural_types: structural_types.clone(),
                call_plan: call_plan.clone(),
                structural_parameters: structural_parameters.clone(),
                cleanup_actions: cleanup_actions.clone(),
            }
        }
        _ => unreachable!("cleanup assignment receives a cleanup carrier"),
    })
}
