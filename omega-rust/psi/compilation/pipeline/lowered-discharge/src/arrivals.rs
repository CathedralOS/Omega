/*
Block invariants and their arrivals.

A call cycle is lowered as a loop: one head block per machine of the cycle,
whose parameters are that machine's formals, and a jump to the head for each
call (`structure-elaboration`, `lower`). The machine's `requires`, stated
over the head's parameters, is the head's invariant, one row of the module's
`scalar_block_invariants`, and it is the only thing known about those
parameters inside the loop.

`wiki/spec/terminal-psi/control_flow.md`: "Every arrival proves the predicate
under simultaneous destination-parameter to actual-argument substitution,
before destination hypotheses become available." and "At a cyclic scheduling
cut, only checked invariant predicates enter as induction hypotheses; prefix
facts are discarded." Knowledge on this route names values, and a head
parameter is a value of its own on every arrival, so nothing known about an
argument is known about the parameter: the predicate's row is all there is.

The predicate is the producer's, never inferred here. This module admits a
row (its scope, and that no edge into the head escapes it); the row then
becomes the head's `BlockInvariant` knowledge, the same row an inferred join
predicate is (`joins`), and `joins::arrival_goal` states each arrival's goal
from it. The one prover decides that goal like any other, and a declared
predicate is never weakened when an arrival fails it.
*/

use semantic_vocabulary::{BlockId, MachineId};
use terminal_check_tables::edges::Destination;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, Construct, MachineTables, RefusalReason};
use terminal_psi::{ScalarBlockInvariant, TerminalMachine};

use crate::refusal;

fn block(lowered: &TerminalMachine, id: BlockId) -> Option<&terminal_psi::Block> {
    usize::try_from(id.get() - 1)
        .ok()
        .and_then(|position| lowered.blocks.get(position))
        .filter(|block| block.id == id)
}

/// Admits one invariant row before its predicate is known anywhere.
///
/// Its predicate may name only the machine's formals and the head's own
/// parameters (control_flow.md: "Predicate scope contains immutable scalar
/// machine formals, destination scalar parameters"), and every edge into the
/// head must be one of its arrivals with an `ArrivalInvariant` obligation
/// (control_flow.md: "Missing or duplicate arrivals, wrong scope or carriers,
/// and stale edge coordinates reject."). A row that fails either would make
/// the predicate a hypothesis nobody proved, so the machine is refused.
pub(crate) fn admit(
    machine: MachineId,
    lowered: &TerminalMachine,
    tables: &MachineTables,
    invariant: &ScalarBlockInvariant,
) -> Checked<()> {
    let header = invariant.header;
    let refuse = |detail: &str| {
        refusal(
            RefusalReason::ForeignIdentity,
            machine,
            Construct::Block(header),
            detail,
        )
    };
    if block(lowered, header).is_none() || header == lowered.entry {
        return Err(refuse("a block invariant names no block it may stand on"));
    }
    let mut scoped = true;
    invariant.predicate.visit_value_ids(|value| {
        scoped &= match tables.values.get(value).map(|row| row.origin) {
            Some(ValueOrigin::MachineParameter { .. }) => true,
            Some(ValueOrigin::BlockParameter { block, .. }) => block == header,
            _ => false,
        };
    });
    if !scoped {
        return Err(refuse("a block invariant names a value outside its scope"));
    }
    for (edge, row) in tables.edges.iter() {
        if row.destination != Destination::Block(header) {
            continue;
        }
        let owed = invariant
            .arrivals
            .iter()
            .filter(|arrival| arrival.edge == edge)
            .count();
        if owed != 1 {
            return Err(refuse(
                "an edge into a block owes its invariant not exactly once",
            ));
        }
    }
    for arrival in &invariant.arrivals {
        let kind = tables
            .obligations
            .get(arrival.obligation)
            .map(|row| row.kind);
        let enters = tables.edges.get(arrival.edge).map(|row| row.destination);
        if kind
            != Some(
                terminal_check_tables::obligations::ObligationKind::ArrivalInvariant {
                    edge: arrival.edge,
                },
            )
            || enters != Some(Destination::Block(header))
        {
            return Err(refuse(
                "a block invariant arrival names another edge or obligation",
            ));
        }
    }
    Ok(())
}
