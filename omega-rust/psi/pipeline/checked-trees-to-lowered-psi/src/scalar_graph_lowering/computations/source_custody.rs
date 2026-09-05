//! Join destination roots to exact authored expressions, without re-lowering.

use super::*;
use checked_trees::statement::{StatementNode, TransitionTargetNode};

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    site: &Site<'_>,
    role: CheckedScalarExpressionRole,
    root: Computation,
) -> Result<(), LoweringError> {
    let program = &checked.typed;
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
    let expression = match program
        .statement_table
        .statements(state.statement_nodes)
        .get(site.statement as usize)
    {
        Some(StatementNode::Expression(expression))
            if role == CheckedScalarExpressionRole::Return =>
        {
            Some(*expression)
        }
        Some(StatementNode::Transition(transition)) => {
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
    let plans = &checked.facts.values.scalar_computations;
    if !plans.nodes.is_valid(root)
        || !expression.is_some_and(|expression| {
            program.expression_table.expression_is_valid(expression)
                && plans.nodes.get(root).authored_root == expression
        })
    {
        return unsupported("scalar computation root disagrees with its authored destination");
    }
    Ok(())
}
