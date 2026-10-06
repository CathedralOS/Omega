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
//! of the one interval engine rather than another private one.
use checked_trees::{FlowCallFact, FlowStateFact};
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(super) fn proves(
    program: &TypedTrees,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    requirement: ExpressionHandle,
    intervals: &[validation::ExpressionInterval],
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(requirement) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return proves(program, state_flow, call_flow, binary.left, intervals)
            && proves(program, state_flow, call_flow, binary.right, intervals);
    }
    let literal = |side: ExpressionHandle| match program.expression_table.expression(side) {
        ExpressionNode::Integer(value) => value.value_i64(),
        _ => None,
    };
    // `parameter OP literal`, or the mirrored `literal OP parameter`.
    let (parameter, operator, bound) = if let Some(bound) = literal(binary.right) {
        (binary.left, binary.operator, bound)
    } else if let Some(bound) = literal(binary.left) {
        let mirrored = match binary.operator {
            BinaryOperator::Less => BinaryOperator::Greater,
            BinaryOperator::LessOrEqual => BinaryOperator::GreaterOrEqual,
            BinaryOperator::Greater => BinaryOperator::Less,
            BinaryOperator::GreaterOrEqual => BinaryOperator::LessOrEqual,
            operator => operator,
        };
        (binary.right, mirrored, bound)
    } else {
        return false;
    };
    let Some(argument) = argument_for(program, state_flow, call_flow, parameter) else {
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

/// The argument expression the call passes for the plain parameter name
/// `parameter` of its target.
fn argument_for(
    program: &TypedTrees,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    parameter: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Name(name) = program.expression_table.expression(parameter) else {
        return None;
    };
    if !name.symbol.is_valid()
        || name.symbol != name.head_symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return None;
    }
    let parameters =
        crate::semantic::calls::call_target_parameters(program, call_flow.target_symbol)?;
    let position = parameters
        .iter()
        .position(|candidate| candidate.symbol == name.symbol)?;
    let site = crate::semantic::calls::find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        call_flow.statement_index,
        call_flow.call_ordinal,
    )?;
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, &site);
    // Arguments and parameters correspond positionally only when every
    // parameter has its own argument.
    if arguments.len() != parameters.len() {
        return None;
    }
    arguments.get(position).copied()
}
