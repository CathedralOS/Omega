//! Dominance-based relocation of a member run whose written locations die on
//! every abandoned path, on the selected CFG.
//!
//! This is the second general member-run mechanism in this area, and the two
//! divide by what they prove rather than by CFG shape. `super::relocation`
//! proves that the move changes no traversal, in either direction, for any
//! run, including the in-block case; it refuses a move whose gained or lost
//! traversal set is nonempty. This admission takes the moves it refuses that
//! are still sound: a sink into a block its own block dominates, where some
//! traversal no longer executes the run at all and the `dead_path` audit
//! proves nothing observes the locations the run wrote, and the mirrored
//! hoist. The price is a narrower subject — pure register and condition-state
//! work, cross-block only — so neither mechanism subsumes the other, and
//! merging them means one module with two admissions rather than one
//! admission.
//!
//! The sink into a fork arm is this region's shallow case, not a shape of
//! its own: the fork head dominates an arm no second edge reaches, and the
//! other edge is the skipped path the dead-path audit clears.
//! `member_sinks_into_the_immediate_fork_arm` lands in the arm itself,
//! `member_sinks_beneath_a_fork_arm` lands in a block deeper than any
//! per-shape family could name, and
//! `a_degenerate_single_arm_fork_sinks_with_no_skipped_path` is the boundary
//! where both edges reach the arm and the audit has nothing to clear.
//!
//! This module lived under `local_schedule`, which is the in-block pair
//! interchange, and nothing here ever read that parent. It sits beside
//! `relocation` because that is the operation it performs.
//!
//! Given a contiguous run of members in one block's body and a destination
//! position in a different block, it derives
//! every simple control-flow path between them, and from those paths the
//! crossed positions (the tail or head of the run's own block, each
//! traversed block's whole stream, and the destination block's share on
//! the far side of the landing index), the crossed edges, the skipped
//! edges that begin the traversals a sink no longer executes on, and the
//! traversals that would gain or lose the run — refused outright by the
//! once-per-traversal bound in either direction: sinking requires the
//! member's block to dominate the destination's, hoisting mirrors it —
//! the destination's block must dominate the member's, the member's
//! block must not be re-enterable without crossing the destination
//! again, and every continuation off the destination must reach the
//! member's block before it can exit or cycle. The hazard, dead-path,
//! and settlement audits then apply once to that derived window,
//! independent of which shape realized it.
//!
//! A window any per-shape family locates — the single `Jump` edge, the
//! fork's landing arm, the diamond's join — is a special case of the
//! derived region; windows the families cannot name, such as a run sinking
//! through a chain of sole-successor blocks or through one arm of a fork
//! into a deeper dominated block, or a run hoisting out of a join back
//! through its predecessors, are the shapes this admission exists to
//! cover. Moves where neither block dominates the other — across an
//! inflow or a branch the run would gain or lose on some traversal — and
//! in-block interchanges remain with the existing families and
//! `super::admission`.
//!
//! The member run must be pure register and condition-state work whose
//! written locations die on every path a sink abandons, the window's
//! crossed positions must be schedulable and uncoupled from every member,
//! every crossed edge must be a plain semantic successor the run does not
//! interfere with, no boundary settlement may observe a changed executed
//! prefix, and every block on a crossing path must be a plain `Source`
//! block. Validation consumes the proposed program, requires the run to
//! sit contiguous at the landing index in the destination block, and
//! restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::collections::{BTreeSet, VecDeque};
use std::sync::Arc;

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, RelocationCrossing, block_instructions, crossed_window, edge_surface,
    member_position, terminator_successors,
};
use crate::rewrites::unexecuted::dead_path;
use crate::rewrites::window_hazards::{
    RunRelocationRejection, admit_run_relocation, register_writes, schedulable, speculatable,
    surface,
};

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    SelectedSuccessor, terminator_instruction,
};
use semantic_vocabulary::{EdgeId, FuelScheduleIdentity};

pub(crate) use validation::validate_scheduled_relocation;

/// An accepted scheduled relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedScheduledRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: ScheduledRelocationReceipt,
}

validated_transformed_receipt!(ValidatedScheduledRelocation, ScheduledRelocationReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScheduledRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(ScheduledRelocationReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScheduledRelocationError {
    /// The named member run or destination does not resolve inside the
    /// named function, or the plan's target does not match the register
    /// environment's.
    SourceMismatch,
    /// The named run can never move past a block boundary: a barrier kind,
    /// a call-roster entry, a roster-carrying or unaccounted memory-capable
    /// kind — an access or fault that ran on every traversal would run only
    /// on the landing path — or a barrier kind, call, or unaccounted
    /// memory-capable instruction at a position the move would cross.
    UnsupportedInstruction,
    /// The named run and destination do not bound an admissible window:
    /// the run is empty, non-contiguous, or shares the destination's
    /// block; the destination names no position, or neither block
    /// dominates the other; the destination (sinking) is reachable from
    /// its own successors, or the run's block (hoisting) is re-enterable
    /// without crossing the destination again, or a continuation off the
    /// destination can exit or cycle before reaching the run's block; a
    /// block on a crossing path is not a plain `Source` block; a crossed
    /// edge is not a plain semantic successor or carries case, fuel, or
    /// structural transfers; a register or condition-state hazard couples
    /// a member with a crossed position or a crossed edge's transports; a
    /// boundary settlement observes a changed executed prefix; or the
    /// dead-path audit finds a location a member writes still live at a
    /// reader on a path the run no longer executes on.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for ScheduledRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid scheduled relocation: {self:?}")
    }
}

impl std::error::Error for ScheduledRelocationError {}

pub(super) fn continuations_reach(
    function: &SelectedFunction,
    from: usize,
    to: usize,
) -> Result<(bool, usize), ScheduledRelocationError> {
    const UNVISITED: u8 = 0;
    const IN_STACK: u8 = 1;
    const DONE: u8 = 2;
    let mut marks = vec![UNVISITED; function.blocks.len()];
    let mut expansions = 0usize;
    let mut stack: Vec<(usize, usize)> = vec![(from, 0)];
    marks[from] = IN_STACK;
    while let Some((block, edge_index)) = stack.last_mut() {
        let successors = terminator_successors(&function.blocks[*block].terminator);
        if successors.is_empty() {
            return Ok((false, expansions));
        }
        if *edge_index == successors.len() {
            marks[*block] = DONE;
            stack.pop();
            continue;
        }
        let edge = successors[*edge_index];
        *edge_index += 1;
        let next =
            block_index_of(function, edge.block).ok_or(ScheduledRelocationError::SourceMismatch)?;
        if next == to {
            continue;
        }
        match marks[next] {
            IN_STACK => return Ok((false, expansions)),
            DONE => continue,
            _ => {
                marks[next] = IN_STACK;
                expansions += 1;
                stack.push((next, 0));
            }
        }
    }
    Ok((true, expansions))
}

pub(super) fn reaches_avoiding(
    function: &SelectedFunction,
    from: usize,
    avoid: Option<usize>,
    target: usize,
) -> Result<bool, ScheduledRelocationError> {
    let mut seen = vec![false; function.blocks.len()];
    let mut pending = VecDeque::from([from]);
    seen[from] = true;
    while let Some(current) = pending.pop_front() {
        if current == target {
            return Ok(true);
        }
        if Some(current) == avoid {
            continue;
        }
        for edge in terminator_successors(&function.blocks[current].terminator) {
            let next = block_index_of(function, edge.block)
                .ok_or(ScheduledRelocationError::SourceMismatch)?;
            if !seen[next] {
                seen[next] = true;
                pending.push_back(next);
            }
        }
    }
    Ok(false)
}

pub(super) fn rejection(rejection: RunRelocationRejection) -> ScheduledRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => ScheduledRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => ScheduledRelocationError::UnsupportedPair,
    }
}

pub(super) fn landing_position(
    block: &SelectedBlock,
    destination: SelectedInstructionId,
) -> Option<usize> {
    block.instruction_index(destination).or_else(|| {
        (terminator_instruction(&block.terminator).id == destination)
            .then_some(block.instructions.len())
    })
}

pub(super) fn block_index_of(function: &SelectedFunction, id: SelectedBlockId) -> Option<usize> {
    function.block_index(id)
}

/// The validator's own reconstruction of the relocation the contract
/// permits: the block the contiguous run vacates, the body positions it
/// occupies, the block it lands in, and the landing index. It shares no
/// state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    member_first: usize,
    member_last: usize,
    target_index: usize,
    landing_index: usize,
}

/// The validator's own continuation search: every simple continuation
/// leaving `from` reaches `to` before it exits or revisits a block.
/// The acyclic-path walk bound, kept symmetric with the producer's so
/// neither side of the contract derives a window the other refuses to
/// measure — the cap the general `relocation` admission uses.
const PATH_EDGE_LIMIT: usize = 64;

/// Every block a crossed path enters — the region the family's
/// `Source`-origin bound applies to.
fn region(crossing: &RelocationCrossing<'_>) -> BTreeSet<usize> {
    crossing
        .positions
        .iter()
        .map(|(block_index, _)| *block_index)
        .collect()
}

/// Every edge leaving a region block that no crossed path uses: the
/// starts of the traversals a sink abandons. The destination block's own
/// successors stay out of the set — the run executes there.
fn skipped_edges<'a>(
    function: &'a SelectedFunction,
    crossing: &RelocationCrossing<'_>,
) -> Vec<&'a SelectedSuccessor> {
    let used: BTreeSet<(SelectedBlockId, EdgeId)> = crossing
        .edges
        .iter()
        .map(|edge| (edge.block, edge.successor.psi_edge))
        .collect();
    let mut skipped = Vec::new();
    for index in region(crossing) {
        if index == crossing.destination_block {
            continue;
        }
        for edge in terminator_successors(&function.blocks[index].terminator) {
            if !used.contains(&(function.blocks[index].id, edge.psi_edge)) {
                skipped.push(edge);
            }
        }
    }
    skipped
}

/// Reconstruct the legality of moving the run `members` onto
/// `destination` from the source records: locate the run and the
/// destination position by identity, bind the sink/hoist direction from
/// the topology's own dominance and continuation bounds, re-derive the
/// path-derived window — every block on a path plain `Source`, every
/// crossed edge plain and free of member interference, every run member
/// schedulable and sinkable, every crossed body position and crossed
/// terminator uncoupled, no boundary settlement observing a changed
/// prefix, and every location the run writes dead on the traversals a
/// sink abandons — and account the family's measured steps against the
/// budget. Nothing in this audit reads the producer's admission
/// decision, so a producer-side legality error fails here even when the
/// proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, ScheduledRelocationError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            ScheduledRelocationError::SourceMismatch
        })?;
    let [first_member, ..] = members else {
        return Err(ScheduledRelocationError::UnsupportedPair);
    };
    let (block_index, member_first) =
        member_position(function, *first_member).ok_or(ScheduledRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_last = member_first + members.len() - 1;
    if members.iter().enumerate().any(|(offset, member)| {
        block
            .instructions
            .get(member_first + offset)
            .map(|instruction| instruction.id)
            != Some(*member)
    }) {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    let run: Vec<&SelectedInstruction> = block.instructions[member_first..=member_last]
        .iter()
        .collect();
    let mut destination_position = None;
    for (target_index, target) in function.blocks.iter().enumerate() {
        if let Some(index) = landing_position(target, destination) {
            if destination_position.is_some() {
                return Err(ScheduledRelocationError::SourceMismatch);
            }
            destination_position = Some((target_index, index));
        }
    }
    let Some((target_index, landing_index)) = destination_position else {
        return Err(ScheduledRelocationError::UnsupportedPair);
    };
    if target_index == block_index {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    let entry_index = block_index_of(function, function.entry_block)
        .ok_or(ScheduledRelocationError::SourceMismatch)?;
    enum Direction {
        Sink,
        Hoist,
    }
    let direction = if !reaches_avoiding(function, entry_index, Some(block_index), target_index)? {
        Direction::Sink
    } else if !reaches_avoiding(function, entry_index, Some(target_index), block_index)? {
        Direction::Hoist
    } else {
        return Err(ScheduledRelocationError::UnsupportedPair);
    };
    let mut search_steps = 0usize;
    match direction {
        Direction::Sink => {
            for edge in terminator_successors(&function.blocks[target_index].terminator) {
                let next = block_index_of(function, edge.block)
                    .ok_or(ScheduledRelocationError::SourceMismatch)?;
                if next == target_index || reaches_avoiding(function, next, None, target_index)? {
                    return Err(ScheduledRelocationError::UnsupportedPair);
                }
            }
        }
        Direction::Hoist => {
            for edge in terminator_successors(&function.blocks[block_index].terminator) {
                let next = block_index_of(function, edge.block)
                    .ok_or(ScheduledRelocationError::SourceMismatch)?;
                if next == block_index
                    || reaches_avoiding(function, next, Some(target_index), block_index)?
                {
                    return Err(ScheduledRelocationError::UnsupportedPair);
                }
            }
            let (reaches, expansions) = continuations_reach(function, target_index, block_index)?;
            search_steps = expansions;
            if !reaches {
                return Err(ScheduledRelocationError::UnsupportedPair);
            }
        }
    }
    // The validator derives the crossed window itself through the shared
    // enumeration, in the direction execution traverses it — the
    // destination trails a sink, leads a hoist — under the same edge
    // traversal bound the producer measures against. No path at all
    // means the run would only be deleted, not relocated, which the
    // shared audit refuses as an unreachable destination.
    let crossing = crossed_window(
        function,
        block_index,
        member_first,
        member_last,
        target_index,
        landing_index,
        match direction {
            Direction::Sink => CrossingDirection::Forward,
            Direction::Hoist => CrossingDirection::Backward,
        },
        PATH_EDGE_LIMIT,
    )
    .ok_or(ScheduledRelocationError::WorkBudgetExceeded)?;
    for index in region(&crossing) {
        if !matches!(
            function.blocks[index].origin,
            SelectedBlockOrigin::Source(_)
        ) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    for member in &run {
        if schedulable(function, member) != Some(false) || !speculatable(member) {
            return Err(ScheduledRelocationError::UnsupportedInstruction);
        }
    }
    // The shared audit independently re-applies the window independence
    // proof: every member schedulable, no member coupled with a crossed
    // position or a crossed edge's terminator instruction, every crossed
    // edge a plain semantic successor the run's transports do not
    // interfere with, and no boundary settlement observing a changed
    // executed prefix.
    admit_run_relocation(function, &run, &crossing).map_err(rejection)?;
    if matches!(direction, Direction::Sink) {
        let skipped = skipped_edges(function, &crossing);
        if !skipped.is_empty()
            && !dead_path::dead(
                function,
                dead_path::Relocation {
                    members: &run,
                    vacated_block: block_index,
                    vacated_first: member_first,
                    vacated_last: member_last,
                    landing_block: target_index,
                    landing_index,
                    landing: dead_path::Landing::Executed,
                    vacated: dead_path::Vacated::Silent,
                },
                dead_path::Start::Edges(&skipped),
            )
        {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    let member_locations = run
        .iter()
        .map(|member| {
            register_writes(member).count() + member.implicit_defs.len() + member.clobbers.len()
        })
        .sum::<usize>();
    let block_scan: usize = function
        .blocks
        .iter()
        .map(|block| {
            block_instructions(block).map(surface).sum::<usize>()
                + terminator_successors(&block.terminator)
                    .iter()
                    .map(|edge| edge_surface(edge))
                    .sum::<usize>()
        })
        .sum();
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block_instructions(block).count())
            })
        })
        .and_then(|total| total.checked_add(PATH_EDGE_LIMIT))
        .and_then(|total| total.checked_add(search_steps))
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
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .and_then(|total| {
            total.checked_add(block_scan.saturating_mul(member_locations.saturating_add(1)))
        })
        .ok_or(ScheduledRelocationError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || ScheduledRelocationError::IdentityOverflow,
        || ScheduledRelocationError::WorkBudgetExceeded,
    )?;
    Ok(Reconstructed {
        function,
        block_index,
        member_first,
        member_last,
        target_index,
        landing_index,
    })
}
