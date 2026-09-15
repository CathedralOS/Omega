//! Incoming guards describe current values at an edge, whereas published
//! scalar crash routes bind invocation-entry operands.

use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
};

/// `mutable_parameters` holds the machine's mutable primitive parameter
/// symbols, which the caller computes once per machine: the set cannot change
/// between the checked sites and calls that share one machine.
pub(super) fn retains_entry_meaning(
    program: &TypedTrees,
    mutable_parameters: &[symbols::SymbolHandle],
    expression: ExpressionHandle,
) -> bool {
    let mut pending = vec![expression];
    let mut seen = Vec::new();
    while let Some(expression) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return false;
        }
        if seen.contains(&expression) {
            continue;
        }
        seen.push(expression);
        match program.expression_table.expression(expression) {
            ExpressionNode::Match(dispatch) => {
                pending.push(dispatch.subject);
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    if let typed_trees::expression::MatchPattern::Value(pattern) = arm.pattern {
                        pending.push(pattern);
                    }
                    pending.push(arm.value);
                }
            }
            ExpressionNode::Name(path) => {
                if mutable_parameters.contains(&path.symbol)
                    || mutable_parameters.contains(&path.head_symbol)
                {
                    // No retained incoming-guard fact proves that this current
                    // storage value still equals its invocation-entry operand.
                    // In particular, the guard may follow an assignment in
                    // the source state, before the guarded edge is selected.
                    return false;
                }
            }
            ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
            ExpressionNode::Unary(unary) => pending.push(unary.operand),
            ExpressionNode::Borrow(borrow) => pending.push(borrow.target),
            ExpressionNode::Cast(cast) => pending.push(cast.value),
            ExpressionNode::Member(member) => pending.push(member.receiver),
            ExpressionNode::Indexed(indexed) => pending.extend([indexed.collection, indexed.index]),
            ExpressionNode::Range(range) => pending.extend(
                [range.start, range.end]
                    .into_iter()
                    .filter(|value| value.is_valid()),
            ),
            ExpressionNode::Atomic(atomic) => pending.extend(
                [atomic.value, atomic.result]
                    .into_iter()
                    .filter(|value| value.is_valid()),
            ),
            ExpressionNode::Call(call) => {
                if call.receiver.is_valid() {
                    pending.push(call.receiver);
                }
                pending.extend(program.expression_table.expression_handles(call.arguments));
            }
            ExpressionNode::ArrayLiteral(elements) => {
                pending.extend(program.expression_table.expression_handles(*elements))
            }
            ExpressionNode::StructLiteral(literal) => pending.extend(
                program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .map(|field| field.value),
            ),
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
    true
}
