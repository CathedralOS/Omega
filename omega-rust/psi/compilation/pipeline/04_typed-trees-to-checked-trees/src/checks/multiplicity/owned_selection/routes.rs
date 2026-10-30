//! Source arm routes shared by selected-storage claim compatibility.

use crate::checks::ownership_occurrences::EvaluationBranch;
use arena::Handle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMatchArm};

/// The same first-matching-arm schedule as ownership occurrence evaluation.
/// The final reachable arm is the remaining edge, not another binary choice.
pub(in crate::checks::multiplicity) fn terminal_arm_route(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
    terminal: Handle<TableMatchArm>,
) -> Option<Vec<EvaluationBranch>> {
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        return None;
    };
    let arms = super::operands::reachable_arms(program, dispatch.arms);
    let mut prefix = Vec::new();
    for (decision, (handle, arm)) in arms.iter().enumerate() {
        let mut route = prefix.clone();
        if decision + 1 < arms.len() {
            route.push(EvaluationBranch::new(expression, decision, true));
        }
        if *handle == terminal {
            return Some(route);
        }
        if let Some(nested) = terminal_arm_route(program, arm.value, terminal) {
            route.extend(nested);
            return Some(route);
        }
        prefix.push(EvaluationBranch::new(expression, decision, false));
    }
    None
}
