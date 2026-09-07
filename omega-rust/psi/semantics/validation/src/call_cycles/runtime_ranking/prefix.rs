//! Exact scalar entry-rank preservation through direct disjoint stores. The
//! shared write-frame owner closes aliases; an opaque frame is not evidence.

use crate::calls::{CallFrameResolver, frame_paths_overlap};
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;

use super::projection::{RankOrder, RankProjection};

pub(super) fn preserves_rank(
    program: &TypedTrees,
    machine: &Machine,
    rank: &RankProjection,
    statement: &StatementNode,
    frames: Option<&CallFrameResolver<'_>>,
) -> bool {
    if !matches!(rank.order, RankOrder::Natural(_)) {
        return false;
    }
    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };
    if !super::expression_is_inert(program, machine.symbol, assignment.target)
        || !super::expression_is_inert(program, machine.symbol, assignment.value)
    {
        return false;
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    // A ranged call consumes entry hypotheses and endpoints as well as its
    // rank. Protect every nonself input in that mode; translate exact owned
    // declarations to the caller-relative frame vocabulary only here.
    frames
        .and_then(|frames| {
            frames
                .assignment_write_frame(machine, statement)
                .into_complete_paths()
        })
        .is_some_and(|paths| {
            program
                .state_parameters(entry)
                .iter()
                .filter(|parameter| {
                    !parameter.is_self
                        && (rank.range.is_valid() || parameter.symbol == rank.parameter)
                })
                .all(|parameter| {
                    paths
                        .iter()
                        .all(|path| !frame_paths_overlap(path, parameter.name.as_str()))
                })
        })
}
