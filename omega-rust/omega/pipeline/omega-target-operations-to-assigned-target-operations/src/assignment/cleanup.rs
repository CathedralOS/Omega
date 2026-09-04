use super::placement::validate_structural_placement;
use super::shared::*;

pub(super) fn validate_scalar_cleanup_signature(
    machine: MachineId,
    target: NativeTarget,
    call_plan: &omega_calling_conventions::CallPlan,
    structural_parameters: &[omega_target_operations::TargetStructuralParameter],
    cleanup_actions: &[psi_terminal::TerminalAffineCleanupAction],
) -> Result<(), AssignmentError> {
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: call_plan
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect(),
            result: call_plan.result.as_ref().map(|placement| placement.shape),
        },
    )
    .map_err(|_| AssignmentError::UnsupportedScalarCleanup(machine))?;
    if cleanup_actions.is_empty()
        || expected_call_plan != *call_plan
        || call_plan.result.is_none()
        || call_plan.parameters.len() < structural_parameters.len()
        || call_plan.parameters[call_plan.parameters.len() - structural_parameters.len()..]
            .iter()
            .zip(structural_parameters)
            .any(|(placement, parameter)| placement != &parameter.placement)
        || cleanup_actions.len() != structural_parameters.len()
        || structural_parameters
            .iter()
            .rev()
            .zip(cleanup_actions)
            .any(|(parameter, action)| match action {
                psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place) => {
                    *place != parameter.place
                }
                psi_terminal::TerminalAffineCleanupAction::InvokeNominal(cleanup) => {
                    cleanup.place != parameter.place
                        || cleanup.structural_type != parameter.structural_type
                }
                psi_terminal::TerminalAffineCleanupAction::DiscardResidual(_) => true,
            })
    {
        return Err(AssignmentError::UnsupportedScalarCleanup(machine));
    }
    for parameter in structural_parameters {
        validate_structural_placement(parameter.place, &parameter.placement, target.architecture)?;
    }
    Ok(())
}

/// Return every terminal value edge in canonical true-before-false DFS order.
/// Cleanup control is finite and branch-only: crashes are a distinct terminal
/// carrier, and replaying one return edge on multiple leaves is forbidden.
pub(super) fn finite_boolean_cleanup_edges(control: &TargetBooleanControl) -> Option<Vec<EdgeId>> {
    fn collect(control: &TargetBooleanControl, edges: &mut Vec<EdgeId>) -> Option<()> {
        match control {
            TargetBooleanControl::ReturnImmediate {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnParameter {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnNotParameter {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnExpression {
                psi_return_edge, ..
            } => edges.push(*psi_return_edge),
            TargetBooleanControl::Conditional {
                when_true,
                when_false,
                ..
            }
            | TargetBooleanControl::ConditionalExpression {
                when_true,
                when_false,
                ..
            } => {
                collect(&when_true.control, edges)?;
                collect(&when_false.control, edges)?;
            }
            TargetBooleanControl::Crash { .. } => return None,
        }
        Some(())
    }

    let mut edges = Vec::new();
    collect(control, &mut edges)?;
    (edges.len() >= 2
        && edges
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            == edges.len())
    .then_some(edges)
}
