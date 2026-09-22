//! Independent validation of triangle relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the converging join
//! the member's block must be, the one fork head the region descends
//! from, the arms the head's edges alone feed, the landing index the
//! destination names in the head's body, the shared crossed-window audit
//! every crossed position and edge must pass, and the
//! boundary-settlement exclusion — then requires the proposal to place
//! exactly the member's instruction on the landing index in the head
//! block, and restoring the member to its join position must reproduce
//! the complete source by content. A producer admission error therefore
//! fails validation even when the proposal is exactly what that producer
//! emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
    SelectedSuccessor, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{TriangleRelocationError, TriangleRelocationReceipt, ValidatedTriangleRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the member's block — the triangle's converging join — the
/// one fork head the region descends from, and the landing index the
/// destination names in the head's body. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The member's own block: the triangle's converging join.
    block_index: usize,
    /// The member's index inside that block's body.
    member_index: usize,
    /// The fork head's index in `function.blocks`.
    target_index: usize,
    /// The body index the member lands at in the head: the named
    /// destination instruction's own index, or the body end when the
    /// destination names the head's terminator-carried instruction.
    landing_index: usize,
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn reject(rejection: RunRelocationRejection) -> TriangleRelocationError {
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

/// Reconstruct the legality of relocating `member` out of its converging
/// join onto `destination` from the source records: locate the member by
/// identity, require its block to be a source-origin join whose every
/// inflow edge leaves the one fork head directly — the bypass — or
/// leaves an arm the head's edges alone feed, require the head's
/// two-successor conditional terminator to name only the join or an arm,
/// resolve the landing index in the head's body, and re-derive the
/// window's independence audit through the shared
/// `crossed_window`/`admit_run_relocation` contract — no register or
/// condition-state hazard between the member and any crossed position,
/// no interference with any crossed edge's register transports, no
/// roster-carrying member sharing the window with a second
/// memory-access actor, no barrier, call, hosted effect, or call-roster
/// entry inside the window, and no boundary settlement whose observed
/// executed prefix changes — then account the family's measured steps
/// against the budget. Nothing in this audit reads the producer's
/// admission decision, so a producer-side legality error fails here even
/// when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, TriangleRelocationError> {
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
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the member, and again with successor edges to gather the
    // join's and the arms' predecessors; each path edge once; the
    // member's surface against each crossed position's and each crossed
    // edge's own surface, plus the function's three rosters.
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
    Ok(Reconstructed {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted member, fork head, and landing index from the source
/// without the producer's admission routine, the proposal must place
/// exactly the member's instruction on the landing index in the head
/// block, and moving it back must restore the complete source by
/// content — every crossed instruction, every other block and
/// instruction, register, roster row, call, settlement, and edge
/// included.
pub fn validate_triangle_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedTriangleRelocation, TriangleRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        member,
        destination,
        environment,
        budget,
    )?;
    let source_member = &reconstructed.function.blocks[reconstructed.block_index].instructions
        [reconstructed.member_index];
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.target_index))
        .and_then(|block| block.instructions.get(reconstructed.landing_index))
        != Some(source_member)
    {
        return Err(TriangleRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let member_instruction = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .remove(reconstructed.landing_index);
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .insert(reconstructed.member_index, member_instruction);
    if restored != *source.selected_plan() {
        return Err(TriangleRelocationError::ReplayMismatch);
    }
    Ok(ValidatedTriangleRelocation {
        receipt: TriangleRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
