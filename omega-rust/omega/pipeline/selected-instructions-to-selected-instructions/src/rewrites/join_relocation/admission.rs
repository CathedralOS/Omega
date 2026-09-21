//! Shared admission for join relocation: locate the named `member` in one
//! block's body, require that block to be a converging join — every edge
//! into it leaving an arm whose terminator is a plain unconditional
//! `Jump` back — whose arms are reached by the edges of one common fork
//! head alone, require that head's two-successor conditional terminator
//! to name only arms, and hand the crossed window to the shared run
//! audit — `crossed_window` derives the positions and edges every acyclic
//! path between the head and the join crosses (the two branch edges and
//! each arm's `Jump` edge under the gates below) and
//! `admit_run_relocation` proves the window independent once — no
//! register or condition-state hazard between the member and any crossed
//! position, no interference with any crossed edge's register transports,
//! no roster-carrying member sharing the window with a second
//! memory-access actor, no barrier, call, hosted effect, or call-roster
//! entry inside the window, and no boundary settlement whose observed
//! executed prefix changes.
//!
//! Where the sinking families prove a member's written locations dead on
//! the paths it stops traversing, this direction proves supply instead:
//! every edge into the join leaves an arm the head alone feeds, and every
//! head edge reaches an arm, so each traversal into the join passes the
//! member's new position and each traversal of the head reaches the join.
//! The member keeps its execution count of one and needs no dead-path
//! audit — the predecessor and successor bookkeeping is the totality
//! proof.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedInstructionId, SelectedSuccessor, SelectedTerminator,
};

use super::JoinRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

pub(super) struct Admission {
    /// The member's own block: the diamond's converging join.
    pub block_index: usize,
    /// The member's index inside that block's body.
    pub member_index: usize,
    /// The fork head's index in `function.blocks`.
    pub target_index: usize,
    /// The destination instruction's position in the head body: the
    /// member lands at this index. Naming the head's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    pub landing_index: usize,
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, JoinRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(JoinRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(JoinRelocationError::SourceMismatch)?;
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
        .ok_or(JoinRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // The member's block must be a converging join: every edge into it
    // leaves an arm — a plain source block, distinct from the join and the
    // entry, whose terminator is an unconditional `Jump` to the join on a
    // plain semantic edge. A predecessor shaped any other way hands the
    // join a path the member's new position cannot supply, and a join
    // with no predecessors is unreachable: hoisting the member would
    // start its execution.
    if block.id == function.entry_block || !matches!(block.origin, SelectedBlockOrigin::Source(_)) {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    let mut arm_indices: Vec<usize> = Vec::new();
    for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == block.id) {
        let arm_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == source_block)
            .ok_or(JoinRelocationError::SourceMismatch)?;
        let arm = &function.blocks[arm_index];
        if arm.id == block.id
            || arm.id == function.entry_block
            || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
        {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        // The predecessor's edge into the join is its `Jump` successor
        // itself: a conditional or non-plain edge into the join is a
        // converging path this step does not cross.
        let SelectedTerminator::Jump {
            successor: arm_edge,
            ..
        } = &arm.terminator
        else {
            return Err(JoinRelocationError::UnsupportedPair);
        };
        if !plain_edge(arm_edge) {
            return Err(JoinRelocationError::UnsupportedPair);
        }
        arm_indices.push(arm_index);
    }
    if arm_indices.is_empty() {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    // The arms must descend from one fork head alone: every edge into
    // every arm leaves the same block. A second predecessor into an arm
    // would hand the join a path that never crossed the member's new
    // position — the partial-supply failure the totality rule refuses.
    let mut target_index: Option<usize> = None;
    for &arm_index in &arm_indices {
        for (source_block, _) in
            all_edges(function).filter(|(_, edge)| edge.block == function.blocks[arm_index].id)
        {
            let head_index = function
                .blocks
                .iter()
                .position(|candidate| candidate.id == source_block)
                .ok_or(JoinRelocationError::SourceMismatch)?;
            match target_index {
                Some(existing) if existing != head_index => {
                    return Err(JoinRelocationError::UnsupportedPair);
                }
                Some(_) => {}
                None => target_index = Some(head_index),
            }
        }
    }
    let target_index = target_index.ok_or(JoinRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The head must not collapse back into the region: landing inside the
    // join or inside an arm is an in-block or self-loop move this step
    // does not cross, and an implementation block's origin carries edge or
    // case work the bounded audit does not cross.
    if target.id == block.id
        || arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == target.id)
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(JoinRelocationError::UnsupportedPair);
    }
    // Only a two-successor conditional terminator gives the landing block
    // the fork this step rises through; every other terminator shape is
    // the single-edge family's case or no relocation at all.
    let branch_edges: Vec<&SelectedSuccessor> = match &target.terminator {
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
            return Err(JoinRelocationError::UnsupportedPair);
        }
    };
    // Every edge the head names must reach an arm: an edge leaving the
    // region would run the member on a traversal the join never saw.
    for edge in &branch_edges {
        if !plain_edge(edge)
            || !arm_indices
                .iter()
                .any(|&arm| function.blocks[arm].id == edge.block)
        {
            return Err(JoinRelocationError::UnsupportedPair);
        }
    }
    let terminator = terminator_instruction(&target.terminator);
    // The destination names a position in the head body — the member lands
    // at its index — or the head's terminator-carried instruction,
    // landing the member at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| (terminator.id == destination).then_some(target.instructions.len()))
        .ok_or(JoinRelocationError::UnsupportedPair)?;
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the member is the one-member run, and the
    // gates above leave only head-to-arm-to-join acyclic paths. The walk
    // pushes each branch edge once and each arm's `Jump` edge once per
    // branch edge feeding it — two pushes per branch edge even when both
    // name the same arm — so twice the branch's own out-edge count bounds
    // it. The shared audit applies the hazard, memory-roster, transport,
    // and settlement checks once: a boundary settlement positioned past
    // the member's index in the join or past the landing index in the
    // head observed a changed executed prefix and refuses, and so does a
    // settlement inside a crossed arm — the member runs before that
    // arm's point after the move where it ran after it before.
    let edge_limit = branch_edges.len() * 2;
    let crossing = crossed_window(
        function,
        block_index,
        member_index,
        member_index,
        target_index,
        landing_index,
        CrossingDirection::Backward,
        edge_limit,
    )
    .ok_or(JoinRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(rejection)?;
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to gather the
    // join's and the arms' predecessors; the path walk touches each edge
    // once; the window audit walks the member's surface against each
    // crossed position's and each crossed edge's own surface, plus the
    // function's three rosters.
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
        .ok_or(JoinRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| JoinRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(JoinRelocationError::WorkBudgetExceeded);
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
fn rejection(rejection: RunRelocationRejection) -> JoinRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => JoinRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => JoinRelocationError::UnsupportedPair,
    }
}
