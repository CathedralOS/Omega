//! A named transition's operand calls move a source local on their edge alone.
//!
//! `next(consume(local), ...)` evaluates `consume` on the selected edge, after
//! selection. When it takes a state local whole by value, the checker records
//! that move as an owned `Transfer` at the call's own occurrence
//! (`Call { statement, call_ordinal, target }`) beside the local's lexical
//! `StateExit` drop: the local dies on every sibling edge, and its custody
//! goes into the call on this one. Edge cleanup producers and consumers
//! rebuild those occurrences from source here and rejoin each to its exact
//! ledger row; a ledger row this reconstruction does not name stays
//! unaccounted, so the edge declines rather than guessing an owner.

use std::collections::HashSet;

use checked_trees::CheckFacts;
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{StatementNode, TransitionTargetNode};

/// The call occurrences inside the named target arguments of the transition
/// at `statement_index` that pass `local` whole as a positional argument, as
/// the permission sources their moves are recorded under, in authored order.
///
/// Only a bare local name is a whole move; a borrow, member projection or
/// receiver spelling is not, and callers keep those on their own routes. A
/// call in the transition's guard is not an edge operand: it runs before
/// selection, so it is never reported here. `None` names a malformed
/// statement or a call occurrence the flow facts do not number exactly once.
pub fn transition_operand_moves(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: SymbolHandle,
) -> Option<Vec<PermissionEventSource>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::Transition(transition) = statements.get(statement_index)? else {
        return None;
    };
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        return Some(Vec::new());
    };
    let mut nodes = Vec::new();
    let mut seen = HashSet::new();
    for argument in program.statement_table.expression_handles(*arguments) {
        crate::expression_traversal::collect_expression_nodes(
            program, *argument, &mut nodes, &mut seen,
        );
    }
    let flow = facts
        .flow
        .control
        .states
        .iter()
        .find_map(|(_, candidate)| {
            (candidate.machine_symbol == machine && candidate.state_symbol == state.symbol)
                .then_some(candidate)
        })?;
    let calls = facts.flow.control.calls.span_or_empty(flow.calls);
    let mut moves = Vec::new();
    for node in nodes {
        let ExpressionNode::Call(call) = program.expression_table.expression(node) else {
            continue;
        };
        let names_local = program
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .any(|argument| {
                matches!(program.expression_table.expression(*argument),
                    ExpressionNode::Name(name) if name.symbol == local
                        && name.head_symbol == local
                        && program.expression_table.name_path_members(name.members).len() == 1)
            });
        if !names_local {
            continue;
        }
        let mut occurrences = calls.iter().filter(|fact| {
            fact.statement_index == statement_index
                && fact.call_ordinal != 0
                && fact.authored_expression == node
        });
        let occurrence = occurrences.next()?;
        if occurrences.next().is_some() {
            return None;
        }
        moves.push(PermissionEventSource::Call {
            statement_index,
            call_ordinal: occurrence.call_ordinal,
            target_symbol: occurrence.target_symbol,
        });
    }
    Some(moves)
}
