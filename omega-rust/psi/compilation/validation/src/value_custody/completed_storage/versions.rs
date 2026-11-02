//! Positions in a program's storage graph and preservation between them.
//!
//! A state's history starts at incoming storage: the invalid snapshot. Each
//! statement and each completed operand names the snapshot its effects reached.
//! Observations of a place stay current from the snapshot that established
//! them while no later event on every reaching path writes that place.

use super::{StoredObservation, StoredOperand, at_snapshot, contents_preserved_from};
use crate::CanonicalPlace;
use arena::Handle;
use checked_trees::{FlowControlFacts, FlowStateFact, FlowStorageSnapshot};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

pub type Snapshot = Handle<FlowStorageSnapshot>;

/// The graph's row for `state`, when the graph records its runtime body.
pub fn state_row<'graph>(
    program: &dyn crate::CheckingInput,
    graph: &'graph FlowControlFacts,
    state: SymbolHandle,
) -> Option<&'graph FlowStateFact> {
    let (machine, _) = crate::find_state(program, state)?;
    graph.unique_state(machine.symbol, state)
}

/// The snapshot after statement `statement_index` of `state` completes.
pub fn statement_snapshot(
    program: &dyn crate::CheckingInput,
    graph: &FlowControlFacts,
    state: SymbolHandle,
    statement_index: usize,
) -> Option<Snapshot> {
    let row = state_row(program, graph, state)?;
    graph
        .statements
        .span(row.statements)?
        .iter()
        .find(|statement| statement.statement_index == statement_index)
        .map(|statement| statement.storage_snapshot)
}

/// The snapshot at which `expression`'s completed operand in `state` was
/// captured: after every effect its evaluation ran, before its consumer.
pub fn operand_snapshot(
    graph: &FlowControlFacts,
    state: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<Snapshot> {
    graph.with_expression_operands(expression, |rows| {
        let mut matching = rows
            .iter()
            .map(|handle| graph.operands.get(*handle))
            .filter(|operand| operand.state_symbol == state);
        let operand = matching.next()?;
        matching
            .all(|other| other.storage_snapshot == operand.storage_snapshot)
            .then_some(operand.storage_snapshot)
    })
}

/// The snapshot after call `call_ordinal` of statement `statement_index`
/// evaluated its operands, before the invocation runs.
pub fn invocation_snapshot(
    program: &dyn crate::CheckingInput,
    graph: &FlowControlFacts,
    state: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<Snapshot> {
    let row = state_row(program, graph, state)?;
    graph
        .calls
        .span(row.calls)?
        .iter()
        .find(|call| call.statement_index == statement_index && call.call_ordinal == call_ordinal)
        .map(|call| call.invocation_storage)
}

/// Whether an observation of `place` established at `anchor` (the invalid
/// snapshot: incoming storage) still holds at `at`.
pub fn place_preserved_since(
    program: &dyn crate::CheckingInput,
    graph: &FlowControlFacts,
    at: Snapshot,
    place: &CanonicalPlace,
    anchor: Snapshot,
) -> bool {
    if anchor == at {
        return true;
    }
    if !anchor.is_valid() {
        return !at.is_valid()
            || matches!(
                at_snapshot(program, graph, at, place, StoredObservation::Contents),
                None | Some(StoredOperand::Incoming(_))
            );
    }
    at.is_valid()
        && contents_preserved_from(
            program,
            graph,
            at,
            place,
            &[anchor],
            StoredObservation::Contents,
        )
}
