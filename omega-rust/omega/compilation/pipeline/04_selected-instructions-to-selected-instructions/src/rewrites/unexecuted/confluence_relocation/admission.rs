//! Shared admission for confluence run relocation: locate the named
//! `first_member` and `last_member` bounding one contiguous run inside a
//! block's body, require that block to end in an unconditional `Jump`
//! whose plain semantic successor is a join at least one other
//! predecessor's edge also reaches — the destination's block — locate
//! the named `destination` there, and prove the move sound in both
//! directions — hand the crossed window to the shared run audit —
//! `crossed_window` derives the positions and edges every acyclic path
//! between the run's block and the join crosses (the lone `Jump` edge
//! under the gates below) and `admit_run_relocation` proves the window
//! independent once (no register or condition-state hazard between any
//! member and any crossed position, no member interference with the
//! crossed edge's register transports, no barrier, call, hosted effect,
//! or call-roster entry inside the window, and no boundary settlement
//! whose observed executed prefix changes) — and every member-written
//! location dead from the landing index forward, where the run's new
//! execution publishes them on arrivals that never ran it.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedInstruction, SelectedInstructionId};

use super::ConfluenceRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, crossed_window, edge_surface, terminator_successors,
};
use crate::rewrites::window_hazards::{admit_run_relocation, surface};

pub(super) struct Admission {
    /// The run's own block: one inflow of the join.
    pub block_index: usize,
    /// The run's first member index in the block body.
    pub first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    pub last_index: usize,
    /// The join's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the join body: the run
    /// lands at this index. Naming the join's terminator-carried
    /// instruction lands the run at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, ConfluenceRelocationError> {
    let derived = super::derive_confluence_span(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
    )?;
    let plan = derived.plan;
    let function = derived.function;
    let block_index = derived.block_index;
    let first_index = derived.first_index;
    let last_index = derived.last_index;
    let run = derived.run;
    let target_index = derived.target_index;
    let landing_index = derived.landing_index;
    let block = &function.blocks[block_index];
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the gates above leave the lone `Jump`
    // edge as the only acyclic path from the run's block to the join, so
    // the terminator's own successor count bounds the walk. The shared
    // audit applies the schedulable, hazard, memory-roster, transport,
    // and settlement checks once across the run's members — the `Jump`
    // terminator instruction is the crossed edge's own position, exempt
    // from the barrier-kind rule but not the call or hazard audit — and
    // a boundary settlement positioned past the run's first index in its
    // own block or past the landing index in the join observed a changed
    // executed prefix and refuses.
    let members: Vec<&SelectedInstruction> = run.iter().collect();
    let edge_limit = terminator_successors(&block.terminator).len();
    let crossing = crossed_window(
        function,
        block_index,
        first_index,
        last_index,
        target_index,
        landing_index,
        CrossingDirection::Forward,
        edge_limit,
    )
    .ok_or(ConfluenceRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &members, &crossing).map_err(super::rejection)?;
    // The dead-path audit: every location any member writes must be dead
    // — unread until rewritten — from the landing index forward, on the
    // shared continuations every inflow reaches. The run publishes the
    // union of its members' locations.
    if !super::run_writes_dead(function, &derived) {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the run's first member, and again with successor edges to
    // find the join's other inflow; the path walk pushes the lone `Jump`
    // edge once; the window audit walks every member's surface against
    // each crossed position's and each crossed edge's own surface, plus
    // the function's three rosters; the dead-path audit rescans a block's
    // stream and edge surfaces only while its entry set grows — at most
    // once per member location per block.
    let (scan, run_locations, block_scan) = super::confluence_scan_base(plan, function, run)
        .ok_or(ConfluenceRelocationError::IdentityOverflow)?;
    let total = Some(scan)
        .and_then(|total| total.checked_add(edge_limit))
        .and_then(|total| {
            run.iter().try_fold(total, |total, member| {
                crossing
                    .positions
                    .iter()
                    .try_fold(total, |total, (crossed_block, positions)| {
                        positions.iter().try_fold(total, |total, position| {
                            total.checked_add(surface(member))?.checked_add(surface(
                                &function.blocks[*crossed_block].instructions[*position],
                            ))
                        })
                    })
            })
        })
        .and_then(|total| {
            run.iter().try_fold(total, |total, member| {
                crossing.edges.iter().try_fold(total, |total, edge| {
                    total
                        .checked_add(surface(member))?
                        .checked_add(surface(edge.instruction))?
                        .checked_add(edge_surface(edge.successor))
                })
            })
        })
        .ok_or(ConfluenceRelocationError::IdentityOverflow)?;
    super::confluence_settle_steps(total, function, block_scan, run_locations, budget)?;
    Ok(Admission {
        block_index: derived.block_index,
        first_index: derived.first_index,
        last_index: derived.last_index,
        target_index: derived.target_index,
        landing_index: derived.landing_index,
    })
}
