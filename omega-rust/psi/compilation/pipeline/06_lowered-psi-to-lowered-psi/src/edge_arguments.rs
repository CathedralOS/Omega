use semantic_vocabulary::{BlockId, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{TerminalMachine, Terminator};

/// Drop the listed old positions from one edge's scalar arguments.
pub(crate) fn drop_removed_positions(arguments: &mut Vec<ValueId>, removed: Option<&Vec<usize>>) {
    let Some(removed) = removed else { return };
    let mut position = 0;
    arguments.retain(|_| {
        let retained = !removed.contains(&position);
        position += 1;
        retained
    });
}

/// Drop every listed old position from the argument list of each edge that
/// targets the block whose positions were removed: a `Jump`, or either arm of
/// a `Conditional`. The rewrites that remove block parameters share this walk,
/// so an edge kind that carries scalar arguments states them in one place.
pub(crate) fn drop_removed_positions_across_edges(
    machine: &mut TerminalMachine,
    removed_positions: &BTreeMap<BlockId, Vec<usize>>,
) {
    for block in &mut machine.blocks {
        match &mut block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } => drop_removed_positions(arguments, removed_positions.get(target)),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    drop_removed_positions(
                        &mut edge.arguments,
                        removed_positions.get(&edge.target),
                    );
                }
            }
            _ => {}
        }
    }
}
