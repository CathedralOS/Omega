//! Shared admission for bypass relocation: locate the named `member` in
//! one block's body, require that block to end in a two-successor
//! conditional branch where at least one edge lands directly on the join
//! — the bypass — and every other distinct target is an arm reached by
//! that branch's edges alone and ending in an unconditional `Jump` to the
//! join, with every edge into the join leaving the head or an arm, then
//! hand the crossed window to the shared run audit — `crossed_window`
//! derives the positions and edges every acyclic path between the head
//! and the join crosses (the bypass edge, each arm-ward branch edge, and
//! each arm's `Jump` edge under the gates below) and
//! `admit_run_relocation` proves the window independent once — no
//! register or condition-state hazard between the member and any crossed
//! position, no interference with any crossed edge's register transports,
//! no roster-carrying member sharing the window with a second
//! memory-access actor, no barrier, call, hosted effect, or call-roster
//! entry inside the window, and no boundary settlement whose observed
//! executed prefix changes. A settlement inside a crossed arm refuses the
//! way the migrated families' intermediate blocks do: the member executes
//! after the arm's point where it executed before it before the move.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstructionId,
    SelectedSuccessor, SelectedTerminator,
};

use super::BypassRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

pub(super) struct Admission {
    /// The member's own block: the triangle's branching head.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The join block's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the join body: the
    /// member lands at this index. Naming the join's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

/// The index of the block an edge lands on.
fn block_index_of(function: &SelectedFunction, id: SelectedBlockId) -> Option<usize> {
    function
        .blocks
        .iter()
        .position(|candidate| candidate.id == id)
}

/// Whether `arm_index` is a plain arm of this bypassed triangle: a source
/// block — never the member's own block, which would make the move
/// self-referential, and never the entry block, which is reached with no
/// predecessor at all — that the branch's edges alone reach, ending in an
/// unconditional `Jump` to the join on a plain semantic edge. A second
/// predecessor into the arm would give the join a path the member never
/// executed on, and an implementation block's origin carries edge or
/// case work the bounded audit does not cross.
fn arm_into<'function>(
    function: &'function SelectedFunction,
    branch_edges: &[&SelectedSuccessor],
    head: SelectedBlockId,
    arm_index: usize,
    join: SelectedBlockId,
) -> Option<&'function SelectedSuccessor> {
    let arm = &function.blocks[arm_index];
    if arm.id == head
        || arm.id == function.entry_block
        || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
    {
        return None;
    }
    let branch_edges_into_arm = branch_edges
        .iter()
        .filter(|edge| edge.block == arm.id)
        .count();
    if all_edges(function)
        .filter(|(_, edge)| edge.block == arm.id)
        .count()
        != branch_edges_into_arm
    {
        return None;
    }
    let SelectedTerminator::Jump {
        successor: arm_edge,
        ..
    } = &arm.terminator
    else {
        return None;
    };
    if !plain_edge(arm_edge) || arm_edge.block != join {
        return None;
    }
    Some(arm_edge)
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, BypassRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(BypassRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(BypassRelocationError::SourceMismatch)?;
    let (block_index, member_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == member)
                .map(|member_index| (block_index, member_index))
        })
        .ok_or(BypassRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // Only a two-successor conditional terminator gives the member's
    // block the branch this step crosses; every other terminator shape
    // is the single-edge family's case or no relocation at all.
    let branch_edges: Vec<&SelectedSuccessor> = match &block.terminator {
        SelectedTerminator::ConditionalBranch {
            when_nonzero,
            when_zero,
            ..
        } => vec![when_nonzero, when_zero],
        SelectedTerminator::ConditionalBranchU64LessThan {
            when_less,
            when_not_less,
            ..
        }
        | SelectedTerminator::ConditionalBranchI64LessThan {
            when_less,
            when_not_less,
            ..
        } => vec![when_less, when_not_less],
        SelectedTerminator::Jump { .. }
        | SelectedTerminator::Crash { .. }
        | SelectedTerminator::HostedExitProcess { .. }
        | SelectedTerminator::Return { .. } => {
            return Err(BypassRelocationError::UnsupportedPair);
        }
    };
    for edge in &branch_edges {
        if !plain_edge(edge) {
            return Err(BypassRelocationError::UnsupportedPair);
        }
    }
    // The join is a branch target itself — the bypass edge lands on it
    // directly, which is exactly where this family parts from the
    // all-arms diamond. Each distinct edge target is tried in turn: a
    // candidate admits when every other distinct target is an arm ending
    // in a `Jump` to it and every edge into it leaves the head or an
    // arm. At most one candidate can admit — a second would make each an
    // arm of the other, and a block jumping to a candidate join is a
    // predecessor that candidate's arm check refuses — so the first and
    // only match stands; zero matches is the diamond family's shape or
    // no converging move at all.
    let mut candidates: Vec<usize> = Vec::new();
    for edge in &branch_edges {
        let index =
            block_index_of(function, edge.block).ok_or(BypassRelocationError::SourceMismatch)?;
        if !candidates.contains(&index) {
            candidates.push(index);
        }
    }
    let mut found: Option<(usize, Vec<usize>)> = None;
    for &join_index in &candidates {
        let join = &function.blocks[join_index];
        // The join must not collapse back into the triangle: landing
        // inside the member's own block is an in-block or self-loop move
        // this step does not cross, the entry block is reached with no
        // predecessor at all, and an implementation block's origin
        // carries edge or case work the bounded audit does not cross.
        if join.id == block.id
            || join.id == function.entry_block
            || !matches!(join.origin, SelectedBlockOrigin::Source(_))
        {
            continue;
        }
        let mut arm_indices: Vec<usize> = Vec::new();
        let mut closes = true;
        for &arm_index in &candidates {
            if arm_index == join_index || arm_indices.contains(&arm_index) {
                continue;
            }
            match arm_into(function, &branch_edges, block.id, arm_index, join.id) {
                Some(_) => {
                    arm_indices.push(arm_index);
                }
                None => {
                    closes = false;
                    break;
                }
            }
        }
        if !closes {
            continue;
        }
        // Every edge into the join must leave the head or an arm: a
        // predecessor anywhere else — including the join's own self-loop —
        // gives the join a path the member would newly execute on or
        // execute a second time. Head edges and arm edges into the join
        // are already enumerated, so the per-source check is complete.
        for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == join.id) {
            if source_block != block.id
                && !arm_indices
                    .iter()
                    .any(|&arm| function.blocks[arm].id == source_block)
            {
                closes = false;
                break;
            }
        }
        if !closes {
            continue;
        }
        if found.is_some() {
            return Err(BypassRelocationError::UnsupportedPair);
        }
        found = Some((join_index, arm_indices));
    }
    let (target_index, _) = found.ok_or(BypassRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The destination names a position in the join body — the member
    // lands at its index — or the join's terminator-carried instruction,
    // landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(BypassRelocationError::UnsupportedPair)?;
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the member is the one-member run, and the
    // gates above leave only head-to-join and head-to-arm-to-join acyclic
    // paths. Each branch edge contributes at most two pushes — the edge
    // into an arm plus the arm's `Jump` edge, or the lone bypass edge —
    // so twice the branch's out-edge count bounds the walk. The shared
    // audit applies the hazard, memory-roster, transport, and settlement
    // checks once: a boundary settlement positioned past the member's
    // index in the head or past the landing index in the join observes a
    // changed executed prefix and refuses, and so does a settlement
    // inside a crossed arm — the member runs after that arm's point
    // after the move where it ran before it before.
    let edge_limit = branch_edges.len() * 2;
    let crossing = crossed_window(
        function,
        block_index,
        member_index,
        member_index,
        target_index,
        landing_index,
        CrossingDirection::Forward,
        edge_limit,
    )
    .ok_or(BypassRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(rejection)?;
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to count
    // predecessor edges; the path walk touches each edge once; the window
    // audit walks the member's surface against each crossed position's
    // and each crossed edge's own surface, plus the function's three
    // rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, candidate| {
                candidate
                    .instructions
                    .iter()
                    .chain(std::iter::once(terminator_instruction(
                        &candidate.terminator,
                    )))
                    .try_fold(total, |total, _| total.checked_add(1))
            })
        })
        .and_then(|total| {
            function.blocks.iter().try_fold(total, |total, candidate| {
                terminator_successors(&candidate.terminator)
                    .iter()
                    .try_fold(total, |total, _| total.checked_add(1))
            })
        })
        .and_then(|total| total.checked_add(edge_limit))
        .and_then(|total| {
            crossing
                .positions
                .iter()
                .try_fold(total, |total, (crossed_block, positions)| {
                    positions.iter().try_fold(total, |total, position| {
                        total
                            .checked_add(surface(member_instruction))?
                            .checked_add(surface(
                                &function.blocks[*crossed_block].instructions[*position],
                            ))
                    })
                })
        })
        .and_then(|total| {
            crossing.edges.iter().try_fold(total, |total, edge| {
                total
                    .checked_add(surface(member_instruction))?
                    .checked_add(surface(edge.instruction))?
                    .checked_add(edge_surface(edge.successor))
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(BypassRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| BypassRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(BypassRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn rejection(rejection: RunRelocationRejection) -> BypassRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => BypassRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => BypassRelocationError::UnsupportedPair,
    }
}
