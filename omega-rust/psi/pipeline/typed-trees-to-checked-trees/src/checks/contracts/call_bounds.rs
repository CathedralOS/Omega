//! Call requirements over arithmetic values bounded independently of snapshots.

use checked_trees::{CheckFacts, CheckedOperatorResolutionStatus, FlowCallFact, FlowStateFact};
use language_core::OperatorSpelling;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
};

#[cfg(test)]
mod tests;

pub(super) fn proves(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    expression: ExpressionHandle,
) -> bool {
    prove(program, facts, caller, call, expression).unwrap_or(false)
}

fn prove(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    expression: ExpressionHandle,
) -> Option<bool> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if facts.operators.uses.iter().any(|(_, operator)| {
        operator.expression == expression
            && operator.status != CheckedOperatorResolutionStatus::BuiltinFallback
    }) {
        return None;
    }
    if binary.operator == BinaryOperator::And {
        return Some(
            proves(program, facts, caller, call, binary.left)
                && proves(program, facts, caller, call, binary.right),
        );
    }
    if binary.operator == BinaryOperator::Or {
        return Some(
            proves(program, facts, caller, call, binary.left)
                || proves(program, facts, caller, call, binary.right),
        );
    }
    let spelling = match binary.operator {
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return None,
    };
    let site = crate::find_call_site(
        program,
        caller.machine_symbol,
        caller.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let crate::CallSite::Expression {
        call: source_call, ..
    } = &site
    else {
        return None;
    };
    if source_call.target_symbol != call.target_symbol
        || source_call.receiver.is_valid()
        || source_call.static_requirement_dispatch.is_some()
    {
        return None;
    }
    let callee = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .first()
            .is_some_and(|state| state.symbol == call.target_symbol)
    })?;
    let parameters = program.state_parameters(program.machine_states(callee).first()?);
    if parameters.iter().any(|parameter| parameter.is_self) {
        return None;
    }
    let arguments = crate::call_site_argument_expressions(program, &site);
    if arguments.len() != parameters.len() {
        return None;
    }
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller.machine_symbol)?;
    let state = crate::find_state_in_machine(program, caller.machine_symbol, caller.state_symbol)?;
    let operand = |expression| match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal.value_i64().map(|value| ((value, value), None)),
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let position = parameters
                .iter()
                .position(|parameter| parameter.symbol == path.symbol)?;
            let parameter = &parameters[position];
            if parameter.is_mutable || parameter.is_self || parameter.is_const {
                return None;
            }
            let argument = arguments[position];
            if !super::prover::has_builtin_operators(program, &facts.operators, argument) {
                return None;
            }
            let bounds =
                validation::immutable_integer_expression_bounds(program, machine, state, argument)?;
            Some((bounds, Some(parameter.type_reference)))
        }
        _ => None,
    };
    let ((left_low, left_high), left_type) = operand(binary.left)?;
    let ((right_low, right_high), right_type) = operand(binary.right)?;
    if !typed_trees::operator::has_builtin_spelled_expression_meaning(
        program,
        callee.symbol,
        expression,
        spelling,
        &[left_type, right_type],
    ) {
        return None;
    }
    Some(match binary.operator {
        BinaryOperator::Equal => {
            left_low == left_high && right_low == right_high && left_low == right_low
        }
        BinaryOperator::NotEqual => left_high < right_low || right_high < left_low,
        BinaryOperator::Less => left_high < right_low,
        BinaryOperator::LessOrEqual => left_high <= right_low,
        BinaryOperator::Greater => left_low > right_high,
        BinaryOperator::GreaterOrEqual => left_low >= right_high,
        _ => return None,
    })
}
