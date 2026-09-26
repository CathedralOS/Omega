//! Call requirements over arithmetic values bounded independently of snapshots.

use crate::checked_trees::{
    CheckFacts, CheckedOperatorResolutionStatus, FlowCallFact, FlowStateFact,
};
use language_core::OperatorSpelling;
use symbol_resolved_trees_to_typed_trees::typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
};

mod context;
#[cfg(test)]
mod tests;
pub(super) use context::proves as proves_in_context;

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
    let site = crate::semantic::calls::find_call_site(
        program,
        caller.machine_symbol,
        caller.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    // A bare call to a machine names its entry state; a named transition
    // names a state of the caller's own machine, whose receiver is the
    // caller's own `self`.
    let (callee, callee_state) = match &site {
        crate::semantic::calls::CallSite::Expression {
            call: source_call, ..
        } => {
            if source_call.target_symbol != call.target_symbol
                || source_call.receiver.is_valid()
                || source_call.static_requirement_dispatch.is_some()
            {
                return None;
            }
            crate::semantic::calls::find_state_with_machine(program, call.target_symbol).filter(
                |(machine, state)| {
                    program
                        .machine_states(machine)
                        .first()
                        .is_some_and(|entry| entry.symbol == state.symbol)
                },
            )?
        }
        crate::semantic::calls::CallSite::TransitionNamed {
            path,
            evidence_arguments,
            ..
        } => {
            if path.symbol != call.target_symbol || !evidence_arguments.is_empty() {
                return None;
            }
            crate::semantic::calls::find_state_with_machine(program, call.target_symbol)
                .filter(|(machine, _)| machine.symbol == caller.machine_symbol)?
        }
        crate::semantic::calls::CallSite::Statement(_) => return None,
    };
    let transition = matches!(
        site,
        crate::semantic::calls::CallSite::TransitionNamed { .. }
    );
    let parameters = program.state_parameters(callee_state);
    if !transition && parameters.iter().any(|parameter| parameter.is_self) {
        return None;
    }
    let explicit = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, &site);
    if arguments.len() != explicit.len() {
        return None;
    }
    let machine = crate::lookup::machine_by_symbol(program, caller.machine_symbol)?;
    let state = crate::semantic::calls::find_state_in_machine(
        program,
        caller.machine_symbol,
        caller.state_symbol,
    )?;
    let operand = |expression| match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal
            .value_i64()
            .map(|value| ((Some(value), Some(value)), None)),
        ExpressionNode::Name(path) if path.symbol.is_valid() && path.head_symbol == path.symbol => {
            let position = explicit
                .iter()
                .position(|parameter| parameter.symbol == path.symbol)?;
            let parameter = explicit[position];
            if parameter.is_mutable || parameter.is_const {
                return None;
            }
            let argument = arguments[position];
            if !super::prover::has_builtin_operators(program, &facts.operators, argument) {
                return None;
            }
            let (low, high) = crate::validation::immutable_integer_expression_bounds(
                program, machine, state, argument,
            )?;
            Some(((Some(low), Some(high)), Some(parameter.type_reference)))
        }
        // A field of the shared receiver holds what every store to it
        // enforces at each read, so those bounds hold at arrival. The
        // callee's own `requires` are never read here.
        ExpressionNode::Member(_) if transition && receiver_rooted(program, callee, expression) => {
            let bounds = crate::validation::stored_integer_bounds(
                program,
                callee,
                callee_state,
                expression,
            )?;
            let place_type = crate::validation::declared_place_type_raw(
                program,
                callee,
                Some(callee_state),
                expression,
            );
            Some((bounds, place_type))
        }
        _ => None,
    };
    let ((left_low, left_high), left_type) = operand(binary.left)?;
    let ((right_low, right_high), right_type) = operand(binary.right)?;
    if !symbol_resolved_trees_to_typed_trees::typed_trees::operator::has_builtin_spelled_expression_meaning(
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
            left_low? == left_high? && right_low? == right_high? && left_low == right_low
        }
        BinaryOperator::NotEqual => left_high? < right_low? || right_high? < left_low?,
        BinaryOperator::Less => left_high? < right_low?,
        BinaryOperator::LessOrEqual => left_high? <= right_low?,
        BinaryOperator::Greater => left_low? > right_high?,
        BinaryOperator::GreaterOrEqual => left_low? >= right_high?,
        _ => return None,
    })
}

/// Whether a member chain reads a field of the machine's own receiver.
fn receiver_rooted(
    program: &TypedTrees,
    machine: &symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine,
    mut expression: ExpressionHandle,
) -> bool {
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
                expression = member.receiver;
            }
            ExpressionNode::Name(path) => {
                return path.symbol == machine.symbol && path.head_symbol == machine.symbol;
            }
            _ => return false,
        }
    }
}
