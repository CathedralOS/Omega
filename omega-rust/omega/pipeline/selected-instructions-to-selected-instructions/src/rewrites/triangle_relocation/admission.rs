//! Shared admission for triangle relocation: locate the named `member` in
//! one block's body, require that block to be a converging join whose
//! every inflow edge leaves the one fork head directly — the bypass — or
//! leaves an arm the head's edges alone feed, where an arm is a plain
//! source block ending in a plain unconditional `Jump` to the join, and
//! require the head's two-successor conditional terminator to name only
//! the join or an arm, then hand the crossed window to the shared run
//! audit — `crossed_window` derives the positions and edges every acyclic
//! path between the head and the join crosses (the bypass edge, each
//! arm-ward branch edge, and each arm's `Jump` edge under the gates
//! below) and `admit_run_relocation` proves the window independent once —
//! no register or condition-state hazard between the member and any
//! crossed position, no interference with any crossed edge's register
//! transports, no roster-carrying member sharing the window with a second
//! memory-access actor, no barrier, call, hosted effect, or call-roster
//! entry inside the window, and no boundary settlement whose observed
//! executed prefix changes.
//!
//! Where the sinking families prove a member's written locations dead on
//! the paths it stops traversing, this direction proves supply instead:
//! every edge into the join leaves the head or an arm the head alone
//! feeds, and every head edge lands on the join or an arm, so each
//! traversal into the join passed the head exactly once and each
//! traversal of the head reaches the join exactly once — bypassed arm or
//! not. The member keeps its execution count of one and needs no
//! dead-path audit — the predecessor and successor bookkeeping is the
//! totality proof. Requiring at least one arm alongside the bypass edge
//! keeps this window disjoint from the arm family's all-direct shape;
//! requiring the direct edge keeps it disjoint from the diamond family's
//! all-arms shape.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedSuccessor,
    SelectedTerminator,
};

use super::TriangleRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

pub(super) struct Admission {
    /// The member's own block: the triangle's converging join.
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

/// Whether the inflow source `arm` is a plain arm of this bypassed
/// triangle: a source block — never the join itself, which would make the
/// move self-referential, and never the entry block, which is reached
/// with no predecessor at all — whose lone `Jump` successor is the join
/// on a plain semantic edge. An implementation block's origin carries
/// edge or case work the bounded audit does not cross.
fn arm_edge_into(
    function: &SelectedFunction,
    arm: &selected_instructions::SelectedBlock,
    join: selected_instructions::SelectedBlockId,
) -> bool {
    if arm.id == join
        || arm.id == function.entry_block
        || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
    {
        return false;
    }
    let SelectedTerminator::Jump {
        successor: arm_edge,
        ..
    } = &arm.terminator
    else {
        return false;
    };
    plain_edge(arm_edge)
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, TriangleRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(TriangleRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(TriangleRelocationError::SourceMismatch)?;
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
        .ok_or(TriangleRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // The member's block must be a converging join: a source block — not
    // the entry block, which is reached with no predecessor at all —
    // whose every inflow edge leaves the one fork head directly or an arm
    // ending in a plain `Jump` to it. Classify each inflow source by its
    // terminator: a lone `Jump` makes it an arm candidate and a
    // two-successor conditional makes it a head candidate; every other
    // terminator names no successor and so never appears here. An arm
    // candidate whose `Jump` is absent or impure hands the join a path
    // the member's new position cannot supply.
    if block.id == function.entry_block || !matches!(block.origin, SelectedBlockOrigin::Source(_)) {
        return Err(TriangleRelocationError::UnsupportedPair);
    }
    let mut arm_indices: Vec<usize> = Vec::new();
    let mut head_index: Option<usize> = None;
    for (source_block, inflow_edge) in
        all_edges(function).filter(|(_, edge)| edge.block == block.id)
    {
        let source_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == source_block)
            .ok_or(TriangleRelocationError::SourceMismatch)?;
        let predecessor = &function.blocks[source_index];
        match &predecessor.terminator {
            SelectedTerminator::Jump { .. } => {
                if !arm_edge_into(function, predecessor, block.id) {
                    return Err(TriangleRelocationError::UnsupportedPair);
                }
                if !arm_indices.contains(&source_index) {
                    arm_indices.push(source_index);
                }
            }
            SelectedTerminator::ConditionalBranch { .. }
            | SelectedTerminator::ConditionalBranchU64LessThan { .. }
            | SelectedTerminator::ConditionalBranchI64LessThan { .. } => {
                if !plain_edge(inflow_edge) {
                    return Err(TriangleRelocationError::UnsupportedPair);
                }
                match head_index {
                    Some(existing) if existing != source_index => {
                        return Err(TriangleRelocationError::UnsupportedPair);
                    }
                    Some(_) => {}
                    None => head_index = Some(source_index),
                }
            }
            SelectedTerminator::Crash { .. }
            | SelectedTerminator::HostedExitProcess { .. }
            | SelectedTerminator::Return { .. } => {
                return Err(TriangleRelocationError::SourceMismatch);
            }
        }
    }
    // The bypass edge is what parts this window from the diamond family's:
    // at least one edge into the join must leave the head directly. A join
    // fed by the head on every edge — no arm at all — is the arm family's
    // shape instead.
    let target_index = head_index.ok_or(TriangleRelocationError::UnsupportedPair)?;
    if arm_indices.is_empty() {
        return Err(TriangleRelocationError::UnsupportedPair);
    }
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
        return Err(TriangleRelocationError::UnsupportedPair);
    }
    // The arms must descend from the head alone: every edge into every arm
    // leaves the fork head. A second predecessor into an arm would hand the
    // join a path that never crossed the member's new position — the
    // partial-supply failure the totality rule refuses.
    for &arm_index in &arm_indices {
        for (source_block, _) in
            all_edges(function).filter(|(_, edge)| edge.block == function.blocks[arm_index].id)
        {
            if source_block != target.id {
                return Err(TriangleRelocationError::UnsupportedPair);
            }
        }
    }
    // The head's terminator is a two-successor conditional branch by the
    // inflow classification above; extract its edges to bound the window.
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
            return Err(TriangleRelocationError::UnsupportedPair);
        }
    };
    // Every edge the head names must land on the join or an arm: an edge
    // leaving the region would run the member on a traversal the join
    // never saw.
    for edge in &branch_edges {
        if !plain_edge(edge)
            || (edge.block != block.id
                && !arm_indices
                    .iter()
                    .any(|&arm| function.blocks[arm].id == edge.block))
        {
            return Err(TriangleRelocationError::UnsupportedPair);
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
        .ok_or(TriangleRelocationError::UnsupportedPair)?;
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the member is the one-member run, and the
    // gates above leave only head-to-join and head-to-arm-to-join acyclic
    // paths. The walk pushes the bypass edge once and each arm path's two
    // edges once per branch edge feeding it — at most two pushes per
    // branch edge — so twice the branch's own out-edge count bounds it.
    // The shared audit applies the hazard, memory-roster, transport, and
    // settlement checks once: a boundary settlement positioned past the
    // member's index in the join or past the landing index in the head
    // observed a changed executed prefix and refuses, and so does a
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
    .ok_or(TriangleRelocationError::WorkBudgetExceeded)?;
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
        .ok_or(TriangleRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| TriangleRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(TriangleRelocationError::WorkBudgetExceeded);
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
fn rejection(rejection: RunRelocationRejection) -> TriangleRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => TriangleRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => TriangleRelocationError::UnsupportedPair,
    }
}
