//! Operation meaning required before call operands supply fixed storage coordinates.

use super::*;

pub(super) fn call_operands_have_builtin_coordinates(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    site: &CallSite<'_>,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return false;
    };
    let Some(state) = find_state(program, state_symbol) else {
        return false;
    };
    if !call_site_argument_expressions(program, site)
        .iter()
        .all(|argument| place_operands_are_builtin(program, machine, state, *argument, 0))
    {
        return false;
    }
    match site {
        CallSite::Expression { call, .. } if call.receiver.is_valid() => {
            place_operands_are_builtin(program, machine, state, call.receiver, 0)
        }
        // Statement receivers retain named field paths; captured reference
        // selectors are checked by the shared local-origin adapter.
        _ => true,
    }
}

fn place_operands_are_builtin(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            place_operands_are_builtin(program, machine, state, borrow.target, depth + 1)
        }
        ExpressionNode::Member(member) => {
            place_operands_are_builtin(program, machine, state, member.receiver, depth + 1)
        }
        ExpressionNode::Indexed(indexed)
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) =>
        {
            // Range normalization owns its endpoint evidence. This gate
            // checks newly retained scalar element geometry beneath it.
            place_operands_are_builtin(program, machine, state, indexed.collection, depth + 1)
        }
        ExpressionNode::Indexed(_) => {
            validation::place_has_builtin_coordinates(program, machine, Some(state), expression)
        }
        // Nested non-place evaluation contributes its own call effects; its
        // syntax supplies no independently guessed storage geometry here.
        _ => true,
    }
}
