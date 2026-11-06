//! Positions in a program's storage graph and preservation between them.
//!
//! A state's history starts at its own empty entry cut over abstract incoming
//! storage, the invalid predecessor. Each statement and each completed operand
//! names the snapshot its effects reached.
//! Observations of a place stay current from the snapshot that established
//! them while no later event on every reaching path writes that place.

use super::{StoredObservation, StoredOperand, at_snapshot, contents_preserved_from};
use crate::CanonicalPlace;
use arena::Handle;
use checked_trees::{
    FlowCallFact, FlowControlFacts, FlowStateFact, FlowStorageOperation, FlowStorageSnapshot,
};
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

/// The one call in `state` whose authored expression is `expression`.
pub fn unique_call<'graph>(
    program: &dyn crate::CheckingInput,
    graph: &'graph FlowControlFacts,
    state: SymbolHandle,
    expression: ExpressionHandle,
) -> Option<&'graph FlowCallFact> {
    let row = state_row(program, graph, state)?;
    let mut calls = graph
        .calls
        .span(row.calls)?
        .iter()
        .filter(|call| call.authored_expression == expression);
    let call = calls.next()?;
    calls.next().is_none().then_some(call)
}

/// The events from `to` back to `from` along the path through each join's
/// one predecessor that reaches `from`, as (event, its predecessor on the
/// path) pairs. None unless the walk crosses such a join and reaches
/// `from`, or when a join has several predecessors reaching `from`.
pub fn followed_path(
    graph: &FlowControlFacts,
    from: Snapshot,
    to: Snapshot,
) -> Option<Vec<(Snapshot, Snapshot)>> {
    let mut path = Vec::new();
    let mut crossed_join = false;
    let mut current = to;
    while current != from {
        if !current.is_valid() || !graph.storage_snapshots.is_valid(current) {
            return None;
        }
        let event = graph.storage_snapshots.get(current);
        let previous = match event.operation {
            FlowStorageOperation::Join { predecessors } => {
                // Only a join this path enters through exactly one predecessor
                // is a path choice. Value alternatives of one call all descend
                // from `from` and choose nothing.
                let mut reaching = graph
                    .storage_predecessors
                    .span_or_empty(predecessors)
                    .iter()
                    .filter(|predecessor| reaches(graph, **predecessor, from));
                let predecessor = *reaching.next()?;
                if reaching.next().is_some() {
                    return None;
                }
                crossed_join = true;
                predecessor
            }
            _ => {
                // A preservation query needs a valid cut below the event.
                if !event.previous.is_valid() && from.is_valid() {
                    return None;
                }
                path.push((current, event.previous));
                event.previous
            }
        };
        current = previous;
    }
    (crossed_join && path.iter().all(|(_, before)| before.is_valid())).then_some(path)
}

/// Whether `target` lies on some path back from `start`.
fn reaches(graph: &FlowControlFacts, start: Snapshot, target: Snapshot) -> bool {
    let mut visited = std::collections::BTreeSet::new();
    let mut pending = vec![start];
    while let Some(current) = pending.pop() {
        if current == target {
            return true;
        }
        if !current.is_valid()
            || !graph.storage_snapshots.is_valid(current)
            || !visited.insert(current.arena_index())
        {
            continue;
        }
        let event = graph.storage_snapshots.get(current);
        match event.operation {
            FlowStorageOperation::Join { predecessors } => {
                pending.extend(graph.storage_predecessors.span_or_empty(predecessors));
            }
            _ => pending.push(event.previous),
        }
    }
    false
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
