//! Call requirements proved from the arithmetic walk's argument intervals.
//!
//! Validation's arithmetic walk checks every call argument in the branch-local
//! environment it is evaluated in, where an `&&` or `||` operand and refuted
//! earlier transition arms already narrow the values. A requirement that
//! compares one integer parameter with a literal (`requires index <= 4`) holds
//! at a call when every value the walk allows for that argument satisfies it:
//! `sum(values, index + 1, ..)` after a refuted `index >= 4 ->` arm passes
//! `index + 1` in `[1, 4]`. Conjunctions of such comparisons hold when each
//! does. Every other requirement keeps its existing provers; this is a client
//! of the one interval engine rather than another private one. The call capture
//! owner supplies the exact completed actual for each demanded formal; this
//! consumer never reconstructs source arguments or evaluates them again.
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(super) fn proves(
    program: &dyn ::validation::CheckingInput,
    requirement: ExpressionHandle,
    intervals: &[validation::ExpressionInterval],
    argument_for: &impl Fn(ExpressionHandle) -> Option<ExpressionHandle>,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(requirement) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return proves(program, binary.left, intervals, argument_for)
            && proves(program, binary.right, intervals, argument_for);
    }
    let literal = |side: ExpressionHandle| match program.expression_table.expression(side) {
        ExpressionNode::Integer(value) => value.value_i64(),
        _ => None,
    };
    // `parameter OP literal`, or the mirrored `literal OP parameter`.
    let (parameter, operator, bound) = if let Some(bound) = literal(binary.right) {
        (binary.left, binary.operator, bound)
    } else if let Some(bound) = literal(binary.left) {
        let mirrored = binary.operator.mirrored();
        (binary.right, mirrored, bound)
    } else {
        return false;
    };
    let Some(argument) = argument_for(parameter) else {
        return false;
    };
    let Some(interval) = validation::expression_interval(intervals, argument) else {
        return false;
    };
    match operator {
        BinaryOperator::Less => interval.high.is_some_and(|high| high < bound),
        BinaryOperator::LessOrEqual => interval.high.is_some_and(|high| high <= bound),
        BinaryOperator::Greater => interval.low.is_some_and(|low| low > bound),
        BinaryOperator::GreaterOrEqual => interval.low.is_some_and(|low| low >= bound),
        BinaryOperator::Equal => interval.low == Some(bound) && interval.high == Some(bound),
        BinaryOperator::NotEqual => {
            interval.high.is_some_and(|high| high < bound)
                || interval.low.is_some_and(|low| low > bound)
        }
        _ => false,
    }
}
