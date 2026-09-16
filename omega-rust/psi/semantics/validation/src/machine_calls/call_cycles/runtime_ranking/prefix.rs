//! Exact scalar entry-rank preservation through direct disjoint stores. The
//! shared write-frame owner closes aliases; an opaque frame is not evidence.

use crate::machine_calls::calls::{CallFrameResolver, frame_paths_overlap};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;

use super::projection::{RankOrder, RankProjection};

/// `premises` names the member's entry premise carriers (`None` when its
/// witness resolves none, which rejects every store); `entry_parameters` is
/// this state's discovered telescope, one entry role per non-self formal;
/// `conserved_endpoints` says the component mixes ranged and unranged
/// members, so every caller entry input may be read as a conserved endpoint.
pub(super) fn preserves_rank(
    program: &TypedTrees,
    machine: &Machine,
    state: &typed_trees::state::State,
    rank: &RankProjection,
    premises: Option<&[SymbolHandle]>,
    entry_parameters: &[SymbolHandle],
    conserved_endpoints: bool,
    statement: &StatementNode,
    frames: Option<&CallFrameResolver<'_>>,
) -> bool {
    if !matches!(
        rank.order,
        RankOrder::Natural(_)
            | RankOrder::IncreasingTo(_)
            | RankOrder::BoundedDistance(_)
            | RankOrder::SliceLength
            | RankOrder::DeclaredIdentity { .. }
    ) {
        return false;
    }
    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };
    if !super::expression_is_inert(program, machine, state, assignment.target)
        || !super::expression_is_inert(program, machine, state, assignment.value)
    {
        return false;
    }
    let Some(premises) = premises else {
        return false;
    };
    // Scalar calls consume the call-site state's hypotheses and any pinned
    // bounds as well as the subject. Every one of those facts names an entry
    // premise carrier, so protect the formals that carry such a role here:
    // the root's own premise formals, or a telescoped slot whose entry role
    // is one. Two further readers assume a slot's arrival value without it
    // being a premise. The site query binds one atom per entry role and
    // installs the equality of every slot sharing that role -- discovery
    // keeps a contested claim only for bare forwards, so the copies are equal
    // exactly while neither is stored to -- and a mixed-range component's
    // endpoint conservation compares each forwarded actual with the arrival
    // atom of every caller entry input. Slots in those positions stay
    // protected as well. Any other formal -- a mutable scratch input carrying
    // a unique non-premise role, or a computed payload with no entry role --
    // may be stored to, because no hypothesis reads its arrival value; the
    // actual it feeds into the call is read live. Translate exact owned
    // declarations to the caller-relative frame vocabulary only here.
    let protected = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .zip(entry_parameters)
        .filter(|(_, role)| {
            role.is_valid()
                && (conserved_endpoints
                    || premises.contains(role)
                    || entry_parameters
                        .iter()
                        .filter(|candidate| *candidate == *role)
                        .take(2)
                        .count()
                        == 2)
        })
        .map(|(parameter, _)| parameter.name.as_str())
        .collect::<Vec<_>>();
    frames
        .and_then(|frames| {
            frames
                .assignment_write_frame(machine, statement)
                .into_complete_paths()
        })
        .is_some_and(|paths| {
            protected
                .iter()
                .all(|input| paths.iter().all(|path| !frame_paths_overlap(path, input)))
        })
}
