use super::cleanup::validate_scalar_cleanup_frontier;
use super::shared::*;

fn collect_finite_boolean_cleanup_return_edges(
    control: &TargetBooleanControl,
    immediate_only: bool,
) -> Option<Vec<EdgeId>> {
    fn collect(
        control: &TargetBooleanControl,
        immediate_only: bool,
        decision_count: &mut usize,
        return_edges: &mut Vec<EdgeId>,
    ) -> Option<()> {
        match control {
            TargetBooleanControl::ReturnImmediate {
                psi_return_edge, ..
            } => return_edges.push(*psi_return_edge),
            TargetBooleanControl::ReturnParameter {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnNotParameter {
                psi_return_edge, ..
            }
            | TargetBooleanControl::ReturnExpression {
                psi_return_edge, ..
            } if !immediate_only => return_edges.push(*psi_return_edge),
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
                *decision_count = decision_count.checked_add(1)?;
                collect(
                    &when_true.control,
                    immediate_only,
                    decision_count,
                    return_edges,
                )?;
                collect(
                    &when_false.control,
                    immediate_only,
                    decision_count,
                    return_edges,
                )?;
            }
            TargetBooleanControl::Crash { .. }
            | TargetBooleanControl::ReturnParameter { .. }
            | TargetBooleanControl::ReturnNotParameter { .. }
            | TargetBooleanControl::ReturnExpression { .. } => return None,
        }
        Some(())
    }

    let mut decision_count = 0;
    let mut return_edges = Vec::new();
    collect(
        control,
        immediate_only,
        &mut decision_count,
        &mut return_edges,
    )?;
    if decision_count == 0 || return_edges.len() < 2 {
        return None;
    }
    Some(return_edges)
}

pub(super) fn shared_boolean_cleanup_return_edges(
    control: &TargetBooleanControl,
) -> Option<Vec<EdgeId>> {
    let mut return_edges = collect_finite_boolean_cleanup_return_edges(control, true)?;
    let unique_edges = return_edges.iter().copied().collect::<BTreeSet<_>>();
    if unique_edges.len() != 1 && unique_edges.len() != return_edges.len() {
        return None;
    }
    if unique_edges.len() == 1 {
        return_edges.truncate(1);
    }
    Some(return_edges)
}

pub(super) fn finite_boolean_cleanup_return_edges(
    control: &TargetBooleanControl,
) -> Option<Vec<EdgeId>> {
    let return_edges = collect_finite_boolean_cleanup_return_edges(control, false)?;
    (return_edges.iter().copied().collect::<BTreeSet<_>>().len() == return_edges.len())
        .then_some(return_edges)
}

pub(super) fn uniform_conditional_cleanup(
    function: &AbstractFunction,
    return_edges: &[EdgeId],
    structural_parameters: &[TargetStructuralParameter],
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<Vec<terminal_psi::TerminalAffineCleanupAction>, LoweringError> {
    let invalid = || LoweringError::UnsupportedOperationInScalarFunction(function.machine);
    let mut returns = BTreeMap::new();
    for operation in &function.operations {
        let AbstractOperation::Return {
            psi_edge,
            cleanup_actions,
            ..
        } = operation
        else {
            continue;
        };
        if returns.insert(*psi_edge, cleanup_actions).is_some() {
            return Err(invalid());
        }
    }
    let first = return_edges
        .first()
        .and_then(|edge| returns.get(edge))
        .copied()
        .ok_or_else(invalid)?;
    if first.is_empty()
        || return_edges
            .iter()
            .any(|edge| returns.get(edge).copied() != Some(first))
    {
        return Err(invalid());
    }
    validate_scalar_cleanup_frontier(
        function.machine,
        first,
        structural_parameters,
        functions,
        structural_types,
    )?;
    Ok(first.to_vec())
}
