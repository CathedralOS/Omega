//! Complete direct value exits of one state, without interpreting its guards.
//! A named transfer or assembly edge needs its ordinary path owner; it cannot
//! be omitted when summarizing the values this invocation might return.

use super::{
    StatementNode, StatementTable, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use crate::expression::ExpressionHandle;

/// Every direct normal value exit, including a surviving tail expression.
/// Earlier expression statements discard their value. Conditional departures
/// do not erase the remaining path, and an unconditional departure ends it.
/// This is an optional observation frontier, not execution, call admission,
/// branch proof, or an interpreter for named state transfers.
pub fn direct_state_return_values(
    table: &StatementTable,
    statements: &[StatementNode],
) -> Option<Vec<ExpressionHandle>> {
    let mut values = Vec::new();
    for (position, statement) in statements.iter().enumerate() {
        match statement {
            StatementNode::Transition(transition) => {
                if transition.exit != TransitionExit::Ordinary || !transition.target.is_valid() {
                    return None;
                }
                for target in transition.targets() {
                    if !table.transition_target_is_valid(target) {
                        return None;
                    }
                    let TransitionTargetNode::Value(value) = table.transition_target(target) else {
                        return None;
                    };
                    values.push(*value);
                }
                if transition.guard == TransitionGuardNode::Always
                    || transition.continuation.is_valid()
                {
                    return Some(values);
                }
            }
            StatementNode::Expression(value) if position + 1 == statements.len() => {
                values.push(*value);
                return Some(values);
            }
            // Local instruction transfers may bypass a source tail. The
            // assembly control graph, not this direct frontier, owns them.
            StatementNode::Assembly(_) => return None,
            _ => {}
        }
    }
    None
}
