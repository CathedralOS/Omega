//! Executable selectors along one authored place spine, independent of its
//! proven storage coordinates. Borrow/recast wrappers preserve that spine;
//! a call is its own value producer and owns its ordinary argument schedule.

use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn authored_projection_selectors(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<Vec<ExpressionHandle>> {
    authored_projection_selectors_below(program, expression, ExpressionHandle::invalid())
}

/// Selectors below an already completed receiver run at this observation.
/// The receiver's own selectors belong to its producer, not to this read.
pub(crate) fn authored_projection_selectors_below(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
    receiver: ExpressionHandle,
) -> Option<Vec<ExpressionHandle>> {
    let mut cursor = expression;
    let mut visited = Vec::new();
    let mut selectors = Vec::new();
    loop {
        if !program.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
            return None;
        }
        if cursor == receiver {
            break;
        }
        visited.push(cursor);
        cursor = match program.expression_table.expression(cursor) {
            ExpressionNode::Borrow(borrow) => borrow.target,
            ExpressionNode::Cast(cast) if cast.form.is_recast() => cast.value,
            ExpressionNode::Member(member) => member.receiver,
            ExpressionNode::Indexed(indexed) => {
                if !matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    selectors.push(indexed.index);
                }
                indexed.collection
            }
            _ if !receiver.is_valid() => break,
            _ => return None,
        };
    }
    selectors.reverse();
    Some(selectors)
}
