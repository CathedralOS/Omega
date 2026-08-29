use super::cleanup::validate_scalar_cleanup_frontier;
use super::shared::*;

pub(super) fn shared_boolean_cleanup_convergence_return_edge(
    function: &AbstractFunction,
) -> Option<EdgeId> {
    let mut conditional_count = 0_usize;
    let mut jump_target = None;
    let mut jump_bindings = Vec::new();
    let mut return_edge = None;
    for operation in &function.operations {
        match operation {
            AbstractOperation::Conditional { .. } => conditional_count += 1,
            AbstractOperation::Jump {
                target, bindings, ..
            } => {
                if bindings.len() != 1 || jump_target.is_some_and(|existing| existing != *target) {
                    return None;
                }
                jump_target = Some(*target);
                jump_bindings.push(bindings[0]);
            }
            AbstractOperation::Return {
                psi_edge,
                value,
                scalar_type: ScalarType::Boolean,
                cleanup_actions,
                ..
            } if !cleanup_actions.is_empty() => {
                if return_edge.replace((*psi_edge, *value)).is_some() {
                    return None;
                }
            }
            AbstractOperation::BooleanConstant { .. }
            | AbstractOperation::BooleanStructuralField { .. }
            | AbstractOperation::BooleanNot { .. }
            | AbstractOperation::IntegerConstant { .. }
            | AbstractOperation::IntegerEqual { .. }
            | AbstractOperation::IntegerLessThan { .. }
            | AbstractOperation::IntegerLessOrEqual { .. }
            | AbstractOperation::IntegerBitwiseNot { .. }
            | AbstractOperation::IntegerWiden { .. }
            | AbstractOperation::IntegerExactCast { .. }
            | AbstractOperation::IntegerBitwiseAnd { .. }
            | AbstractOperation::IntegerBitwiseOr { .. }
            | AbstractOperation::IntegerBitwiseXor { .. }
            | AbstractOperation::WrappingIntegerShiftLeft { .. }
            | AbstractOperation::WrappingIntegerShiftRight { .. }
            | AbstractOperation::ExactIntegerShiftLeft { .. }
            | AbstractOperation::ExactIntegerShiftRight { .. }
            | AbstractOperation::WrappingIntegerAdd { .. }
            | AbstractOperation::ExactIntegerAdd { .. }
            | AbstractOperation::SaturatingIntegerAdd { .. }
            | AbstractOperation::WrappingIntegerSubtract { .. }
            | AbstractOperation::ExactIntegerSubtract { .. }
            | AbstractOperation::SaturatingIntegerSubtract { .. }
            | AbstractOperation::WrappingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerMultiply { .. }
            | AbstractOperation::SaturatingIntegerMultiply { .. }
            | AbstractOperation::ExactIntegerDivide { .. }
            | AbstractOperation::ExactIntegerRemainder { .. } => {}
            _ => return None,
        }
    }
    let target = jump_target?;
    let (edge, returned_value) = return_edge?;
    if conditional_count == 0
        || Some(jump_bindings.len()) != conditional_count.checked_add(1)
        || jump_bindings.iter().any(|binding| {
            binding.parameter != returned_value || binding.scalar_type != ScalarType::Boolean
        })
    {
        return None;
    }
    let target_entry = function
        .block_entries
        .iter()
        .position(|entry| entry.block == target)?;
    let start = function.block_entries[target_entry].operation_offset;
    let end = function
        .block_entries
        .get(target_entry + 1)
        .map_or(function.operations.len(), |entry| entry.operation_offset);
    matches!(
        function.operations.get(start..end),
        Some([AbstractOperation::Return { psi_edge, .. }]) if *psi_edge == edge
    )
    .then_some(edge)
}

pub(super) fn shared_boolean_control_return_edge(control: &TargetBooleanControl) -> Option<EdgeId> {
    match control {
        TargetBooleanControl::ReturnImmediate {
            psi_return_edge, ..
        } => Some(*psi_return_edge),
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
            let when_true = shared_boolean_control_return_edge(&when_true.control)?;
            let when_false = shared_boolean_control_return_edge(&when_false.control)?;
            (when_true == when_false).then_some(when_true)
        }
        TargetBooleanControl::Crash { .. }
        | TargetBooleanControl::ReturnParameter { .. }
        | TargetBooleanControl::ReturnNotParameter { .. }
        | TargetBooleanControl::ReturnExpression { .. } => None,
    }
}

pub(super) fn finite_boolean_cleanup_return_edges(
    control: &TargetBooleanControl,
) -> Option<Vec<EdgeId>> {
    fn collect(
        control: &TargetBooleanControl,
        decision_count: &mut usize,
        return_edges: &mut Vec<EdgeId>,
    ) -> Option<()> {
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
            } => return_edges.push(*psi_return_edge),
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
                collect(&when_true.control, decision_count, return_edges)?;
                collect(&when_false.control, decision_count, return_edges)?;
            }
            TargetBooleanControl::Crash { .. } => return None,
        }
        Some(())
    }

    let mut decision_count = 0;
    let mut return_edges = Vec::new();
    collect(control, &mut decision_count, &mut return_edges)?;
    if decision_count == 0
        || return_edges.len() < 2
        || return_edges.iter().copied().collect::<BTreeSet<_>>().len() != return_edges.len()
    {
        return None;
    }
    Some(return_edges)
}

pub(super) fn uniform_conditional_cleanup(
    function: &AbstractFunction,
    return_edges: &[EdgeId],
    structural_parameters: &[TargetStructuralParameter],
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<Vec<psi_terminal::TerminalAffineCleanupAction>, LoweringError> {
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
