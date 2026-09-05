//! Join destination roots to exact authored expressions, without re-lowering.

use super::*;
use checked_trees::statement::{StatementNode, TransitionTargetNode};

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    site: &Site<'_>,
    role: CheckedScalarExpressionRole,
    root: Computation,
    expected_destination: symbols::SymbolHandle,
) -> Result<(), LoweringError> {
    let program = &checked.typed;
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(root) {
        return unsupported("scalar computation root disagrees with its authored destination");
    }
    let state = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == machine)
        .and_then(|machine| {
            program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == site.state)
        })
        .ok_or(LoweringError::Unsupported(
            "scalar computation root has no authored state",
        ))?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let expression = match statements.get(site.statement as usize) {
        Some(StatementNode::LocalData(local))
            if program.primitive_type_reference(local.type_reference)
                == Some(plans.nodes.get(root).primitive_type) =>
        {
            match role {
                CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }
                    if !local.is_mutable && !expected_destination.is_valid() =>
                {
                    // Computation planning numbers only initialized immutable
                    // primitive locals. Mutable storage does not occupy that namespace.
                    let preceding_bindings = statements[..site.statement as usize]
                        .iter()
                        .filter(|statement| {
                            matches!(statement, StatementNode::LocalData(local)
                                if !local.is_mutable
                                    && local.initial_value.is_valid()
                                    && program
                                        .primitive_type_reference(local.type_reference)
                                        .is_some())
                        })
                        .count();
                    (u32::try_from(preceding_bindings).ok() == Some(binding_ordinal))
                        .then_some(local.initial_value)
                }
                CheckedScalarExpressionRole::StorageInitializer
                    if local.is_mutable
                        && expected_destination.is_valid()
                        && local.symbol == expected_destination =>
                {
                    Some(local.initial_value)
                }
                _ => None,
            }
        }
        Some(StatementNode::Assignment(assignment))
            if role == CheckedScalarExpressionRole::AssignmentValue
                && expected_destination.is_valid() =>
        {
            // The old value remains readable while the RHS runs. Only the
            // authored mutable local receives the completed replacement.
            let checked_trees::expression::ExpressionNode::Name(name) =
                program.expression_table.expression(assignment.target)
            else {
                return unsupported("scalar computed assignment needs an exact local destination");
            };
            (name.symbol == expected_destination
                && statements[..site.statement as usize]
                    .iter()
                    .any(|statement| {
                        matches!(statement, StatementNode::LocalData(local)
                        if local.symbol == expected_destination
                            && local.is_mutable
                            && program.primitive_type_reference(local.type_reference)
                                == Some(plans.nodes.get(root).primitive_type))
                    }))
            .then_some(assignment.value)
        }
        Some(StatementNode::Expression(expression))
            if role == CheckedScalarExpressionRole::Return && !expected_destination.is_valid() =>
        {
            Some(*expression)
        }
        Some(StatementNode::Transition(transition))
            if role == CheckedScalarExpressionRole::Guard
                && !expected_destination.is_valid()
                && plans.nodes.get(root).primitive_type == PrimitiveType::Bool =>
        {
            match transition.guard {
                checked_trees::statement::TransitionGuardNode::When(expression) => Some(expression),
                checked_trees::statement::TransitionGuardNode::Always => None,
            }
        }
        Some(StatementNode::Transition(transition)) if !expected_destination.is_valid() => {
            let target = match role {
                CheckedScalarExpressionRole::ContinuationReturn
                | CheckedScalarExpressionRole::TransitionContinuationArgument { .. } => {
                    transition.continuation
                }
                _ => transition.target,
            };
            match (program.statement_table.transition_target(target), role) {
                (
                    TransitionTargetNode::Value(expression),
                    CheckedScalarExpressionRole::Return
                    | CheckedScalarExpressionRole::ContinuationReturn,
                ) => Some(*expression),
                (
                    TransitionTargetNode::Named { arguments, .. },
                    CheckedScalarExpressionRole::TransitionArgument { argument_ordinal }
                    | CheckedScalarExpressionRole::TransitionContinuationArgument {
                        argument_ordinal,
                    },
                ) => program
                    .statement_table
                    .expression_handles(*arguments)
                    .get(argument_ordinal as usize)
                    .copied(),
                _ => None,
            }
        }
        _ => None,
    };
    if !expression.is_some_and(|expression| {
        program.expression_table.expression_is_valid(expression)
            && plans.nodes.get(root).authored_root == expression
    }) {
        return unsupported("scalar computation root disagrees with its authored destination");
    }
    Ok(())
}
