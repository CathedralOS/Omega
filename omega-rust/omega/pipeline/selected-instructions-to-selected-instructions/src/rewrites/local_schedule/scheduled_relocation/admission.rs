//! The path-derived relocation window, now through the shared derivation
//! and audit: `crossed_window` enumerates every acyclic path between the
//! run's block and the destination's — in the direction this family's
//! own dominance bounds established — and returns the crossed positions
//! and crossed edges; `admit_run_relocation` proves that window
//! independent once. What stays local is what makes this family the
//! general admission: the sink/hoist once-per-traversal bounds
//! (dominance, re-entry, continuations), the member's pure-work bound,
//! and the skipped-edge dead-path audit a sink owes.
use std::collections::{BTreeSet, VecDeque};

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlock, SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstruction,
    SelectedInstructionId, SelectedSuccessor,
};
use semantic_vocabulary::EdgeId;

use super::ScheduledRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, RelocationCrossing, block_instructions, crossed_window, edge_surface,
    terminator_instruction, terminator_successors,
};
use crate::rewrites::dead_path;
use crate::rewrites::window_hazards::{
    RunRelocationRejection, admit_run_relocation, register_writes, schedulable, surface,
};

/// The admitted run relocation: where the run leaves and where it lands.
pub(super) struct Admission {
    /// The block the contiguous run vacates.
    pub(super) block_index: usize,
    /// The first body index the run occupies.
    pub(super) member_first: usize,
    /// The last body index the run occupies — `member_first` for a
    /// one-member run.
    pub(super) member_last: usize,
    /// The block the run lands in.
    pub(super) target_index: usize,
    /// The body index the run's first member takes; naming the
    /// destination's terminator-carried instruction lands the run at the
    /// body end.
    pub(super) landing_index: usize,
}

/// Whether the member's effect is pure register and condition-state work
/// that cannot observe or abandon the execution it leaves behind.
/// `schedulable` already cleared barrier kinds, call contracts, and
/// unaccounted memory-capable kinds, but a row-less load or private-slot
/// `Store64` still performs a memory access: sinking it would remove the
/// access — and any fault or slot write it carried — from every path the
/// move abandons. The same holds for kinds whose target encoding may
/// architecturally fault: their proof obligations establish definedness
/// for the source operation, but this audit runs at the selected level
/// where the encoded trap behavior is the honest bound — an execution
/// that could fault must still run on every path that ran it before.
fn sinkable(instruction: &SelectedInstruction) -> bool {
    use selected_instructions::SelectedInstructionKind::*;
    !matches!(
        instruction.kind,
        CopyBytes
            | LoadPacked { .. }
            | StorePacked { .. }
            | Store { .. }
            | Load8Indexed
            | Load64 { .. }
            | Load8 { .. }
            | Load16 { .. }
            | Load32 { .. }
            | Store64 { .. }
            | ExactDivideU64 { .. }
            | SaturatingDivide { .. }
            | SaturatingRemainder { .. }
    )
}

/// Whether every execution leaving `from` reaches `to` before it can exit
/// or revisit a block — the hoist's once-per-traversal bound. A block
/// reachable from `from` without passing `to` that terminates, or sits on
/// a cycle, would let a traversal pass `from` yet abandon or repeat the
/// run's execution. The returned count is the depth-first walk's
/// expansions for the work-budget fold.
fn continuations_reach(
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

/// The position `destination` names inside `block`: a body instruction's
/// own index, or the body end when it names the terminator-carried
/// instruction.
fn landing_position(block: &SelectedBlock, destination: SelectedInstructionId) -> Option<usize> {
    block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&block.terminator).id == destination)
                .then_some(block.instructions.len())
        })
}

fn block_index_of(function: &SelectedFunction, id: SelectedBlockId) -> Option<usize> {
    function.blocks.iter().position(|block| block.id == id)
}

/// Whether `target` is reachable from `from` while `avoid` is never
/// expanded — the dominator question asked concretely: `avoid` dominates
/// `target` exactly when no such route exists. `None` expands every block.
fn reaches_avoiding(
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

/// The acyclic-path walk bound: this family's dominance bounds admit
/// multi-path windows, so the derivation carries the cap the general
/// `relocation` admission uses — a window whose path enumeration takes
/// more edge traversals than this abandons as over budget rather than
/// reporting a truncated window.
const PATH_EDGE_LIMIT: usize = 64;

/// The region a `RelocationCrossing` derived: every block a crossed path
/// touches — the run's block, each intermediate's, and the destination's.
/// `crossed_window` records whole-body ordinals for intermediates and the
/// endpoint spans for the run and destination blocks, so its position
/// keys are exactly the region the family's audits named.
fn region(crossing: &RelocationCrossing<'_>) -> BTreeSet<usize> {
    crossing
        .positions
        .iter()
        .map(|(block_index, _)| *block_index)
        .collect()
}

/// Every edge leaving a region block that no crossed path uses — the
/// traversals a sink no longer executes the run on. The destination
/// itself is never a skipped-edge source: its successors continue the
/// execution the relocated run joined.
fn skipped_edges<'function>(
    function: &'function SelectedFunction,
    crossing: &RelocationCrossing<'function>,
) -> Vec<&'function SelectedSuccessor> {
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

/// Maps the shared audit's rejection onto this family's typed errors: an
/// unschedulable member or crossed position is the instruction-level
/// refusal and every window-level refusal — unreachable destination,
/// coupling, memory ordering, transport conflict, non-plain edge,
/// settlement — is the pair kind.
fn rejection(rejection: RunRelocationRejection) -> ScheduledRelocationError {
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

/// Admit `members` — a contiguous run of body instructions named in their
/// own order — to leave their block and land on `destination`'s position
/// in a block their block dominates (a sink), or land on a position in a
/// block dominating theirs whose continuations all reach theirs (a
/// hoist), when the shared audits clear the derived window: every path
/// between them simple and through plain `Source` blocks, every crossed
/// edge plain and free of member interference, every crossed position
/// schedulable and uncoupled, no boundary settlement observing a changed
/// prefix, and — for a sink, which abandons traversals — every location
/// the run writes dead on every traversal the run no longer executes on.
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, ScheduledRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ScheduledRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ScheduledRelocationError::SourceMismatch)?;
    let [first_member, ..] = members else {
        return Err(ScheduledRelocationError::UnsupportedPair);
    };
    // The run's first member bounds the block and span; the remaining
    // members must occupy the consecutive body positions in order.
    let (block_index, member_first) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == *first_member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(ScheduledRelocationError::SourceMismatch)?;
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
    // The destination names exactly one position: a body instruction's
    // index in its block, or the body end for a terminator-carried
    // instruction.
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
    // An in-block move is the adjacent-interchange family's case; this
    // admission bounds only moves across a block boundary.
    if target_index == block_index {
        return Err(ScheduledRelocationError::UnsupportedPair);
    }
    let entry_index = block_index_of(function, function.entry_block)
        .ok_or(ScheduledRelocationError::SourceMismatch)?;
    // The direction binds the once-per-traversal argument. A sink keeps
    // the run once-per-traversal because every destination traversal ran
    // the run's block first; a hoist mirrors that — the destination's
    // block must dominate the run's, so no traversal reaches the run
    // without having executed it at the landing. Any other topology moves
    // the run across an inflow or a branch where some traversal would
    // gain or lose it.
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
            // The run lands in the destination's stream and runs on every
            // traversal of it; a destination re-enterable from its own
            // successors would run it again where the source ran it once.
            for edge in terminator_successors(&function.blocks[target_index].terminator) {
                let next = block_index_of(function, edge.block)
                    .ok_or(ScheduledRelocationError::SourceMismatch)?;
                if next == target_index || reaches_avoiding(function, next, None, target_index)? {
                    return Err(ScheduledRelocationError::UnsupportedPair);
                }
            }
        }
        Direction::Hoist => {
            // A run block re-enterable from its own successors without
            // crossing the destination again would execute the run where
            // the relocated program already executed it once at the
            // landing.
            for edge in terminator_successors(&function.blocks[block_index].terminator) {
                let next = block_index_of(function, edge.block)
                    .ok_or(ScheduledRelocationError::SourceMismatch)?;
                if next == block_index
                    || reaches_avoiding(function, next, Some(target_index), block_index)?
                {
                    return Err(ScheduledRelocationError::UnsupportedPair);
                }
            }
            // Every traversal leaving the destination must reach the
            // run's block before it exits or revisits a block: an escape
            // would execute the relocated run on a traversal that never
            // ran it, and a cycle would run it again before the source
            // position arrived once. Since the search certifies that
            // continuation acyclic, every execution between them is a
            // simple path — so the derived window's edge audit covers
            // every edge a real traversal can cross.
            let (reaches, expansions) = continuations_reach(function, target_index, block_index)?;
            search_steps = expansions;
            if !reaches {
                return Err(ScheduledRelocationError::UnsupportedPair);
            }
        }
    }
    // The crossed window comes from the shared derivation in the
    // direction execution traverses it — the destination trails a sink
    // (paths join the run's block to it), leads a hoist (paths join it to
    // the run's block). The topology bounds above keep every real
    // traversal inside the acyclic paths the walk enumerates, and the
    // walk abandons as over budget past PATH_EDGE_LIMIT edge traversals
    // rather than reporting a truncated window. No path at all means the
    // run would only be deleted, not relocated, which the shared audit
    // refuses as an unreachable destination.
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
    // Every block a path crosses is plain `Source` control flow: an
    // implementation block's origin carries edge or case work the bounded
    // audit does not cross.
    for index in region(&crossing) {
        if !matches!(
            function.blocks[index].origin,
            SelectedBlockOrigin::Source(_)
        ) {
            return Err(ScheduledRelocationError::UnsupportedPair);
        }
    }
    // The run's own effect bound: the move makes its execution conditional
    // on reaching the destination, so only pure register and
    // condition-state work may relocate.
    for member in &run {
        if schedulable(function, member) != Some(false) || !sinkable(member) {
            return Err(ScheduledRelocationError::UnsupportedInstruction);
        }
    }
    // The shared audit applies the window independence proof once: every
    // member schedulable, no member coupled with a crossed position or a
    // crossed edge's terminator instruction, every crossed edge a plain
    // semantic successor the run's transports do not interfere with, and
    // no boundary settlement observing a changed executed prefix in the
    // run's block, a crossed intermediate, or the destination past the
    // landing index.
    admit_run_relocation(function, &run, &crossing).map_err(rejection)?;
    // A sink abandons traversals: the skipped edges — every edge leaving
    // a region block, other than the destination block, that no crossed
    // path uses — begin the traversals the run no longer executes on. The
    // dead-path audit requires every location the run writes to die
    // there: the vacated span stays silent because every position behind
    // it that could observe the missing writes sits inside the crossed
    // window, and the landing is Executed because the dominance bound
    // makes every destination traversal run the run. A hoist abandons
    // nothing: its dominance and continuation bounds keep every traversal
    // that ran the run running it.
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
    // The scan walks every block body, terminator, and edge once to
    // locate the run and the destination; the shared derivation walks at
    // most PATH_EDGE_LIMIT edge traversals; the window audit walks each
    // member's surface against each crossed position's and each crossed
    // edge's; the dead-path audit rescans a block's stream and edge surfaces only
    // while its entry set grows — at most once per member location per
    // block.
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
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
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
    if u64::try_from(steps).map_err(|_| ScheduledRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ScheduledRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        member_first,
        member_last,
        target_index,
        landing_index,
    })
}
