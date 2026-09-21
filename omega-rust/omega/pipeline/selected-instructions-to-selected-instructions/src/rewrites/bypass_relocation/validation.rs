//! Independent validation of bypass relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the member's own
//! block's two-successor conditional branch, the branch targets that are
//! each either the join — the bypass edge — or a plain arm the branch's
//! edges alone feed ending in an unconditional `Jump` to the join, the
//! landing index the destination names in the join's body, the shared
//! crossed-window audit every crossed position and edge must pass, and
//! the boundary-settlement exclusion — then requires the proposal to
//! place exactly the member's instruction on the landing index in the
//! join block, and restoring the member to its head position must
//! reproduce the complete source by content. A producer admission error
//! therefore fails validation even when the proposal is exactly what
//! that producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockId, SelectedBlockOrigin, SelectedFunction, SelectedInstructionId,
    SelectedInstructionPlan, SelectedSuccessor, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{BypassRelocationError, BypassRelocationReceipt, ValidatedBypassRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the member's block — the bypassed triangle's branching
/// head — the join block one branch edge lands on directly, and the
/// landing index the destination names in the join's body. It shares no
/// state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The member's own block: the triangle's branching head.
    block_index: usize,
    /// The member's index inside that block's body.
    member_index: usize,
    /// The join block's index in `function.blocks`.
    target_index: usize,
    /// The body index the member lands at in the join: the named
    /// destination instruction's own index, or the body end when the
    /// destination names the join's terminator-carried instruction.
    landing_index: usize,
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn reject(rejection: RunRelocationRejection) -> BypassRelocationError {
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
fn arm_into(
    function: &SelectedFunction,
    branch_edges: &[&SelectedSuccessor],
    head: SelectedBlockId,
    arm_index: usize,
    join: SelectedBlockId,
) -> bool {
    let arm = &function.blocks[arm_index];
    if arm.id == head
        || arm.id == function.entry_block
        || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
    {
        return false;
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
        return false;
    }
    let SelectedTerminator::Jump {
        successor: arm_edge,
        ..
    } = &arm.terminator
    else {
        return false;
    };
    plain_edge(arm_edge) && arm_edge.block == join
}

/// Reconstruct the legality of relocating `member` through its block's
/// bypassed branch onto `destination` from the source records: locate
/// the member by identity, require its block's terminator to be a
/// two-successor conditional branch on plain edges, require each
/// distinct branch target to admit as the join — reached directly on a
/// bypass edge — only when every other distinct target is a plain arm
/// the branch alone feeds ending in a `Jump` to it and every edge into
/// it leaves the head or an arm, resolve the landing index in the join's
/// body, and re-derive the window's independence audit through the
/// shared `crossed_window`/`admit_run_relocation` contract — no register
/// or condition-state hazard between the member and any crossed
/// position, no interference with any crossed edge's register
/// transports, no roster-carrying member sharing the window with a
/// second memory-access actor, no barrier, call, hosted effect, or
/// call-roster entry inside the window, and no boundary settlement whose
/// observed executed prefix changes — then account the family's measured
/// steps against the budget. Nothing in this audit reads the producer's
/// admission decision, so a producer-side legality error fails here even
/// when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, BypassRelocationError> {
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
    let mut target_index: Option<usize> = None;
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
            if arm_into(function, &branch_edges, block.id, arm_index, join.id) {
                arm_indices.push(arm_index);
            } else {
                closes = false;
                break;
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
        if target_index.is_some() {
            return Err(BypassRelocationError::UnsupportedPair);
        }
        target_index = Some(join_index);
    }
    let target_index = target_index.ok_or(BypassRelocationError::UnsupportedPair)?;
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
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
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
    Ok(Reconstructed {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted member, join, and landing index from the source without
/// the producer's admission routine, the proposal must place exactly the
/// member's instruction on the landing index in the join block, and
/// moving it back must restore the complete source by content — every
/// crossed instruction, every other block and instruction, register,
/// roster row, call, settlement, and edge included.
pub fn validate_bypass_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedBypassRelocation, BypassRelocationError> {
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
        return Err(BypassRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let member_instruction = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .remove(reconstructed.landing_index);
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .insert(reconstructed.member_index, member_instruction);
    if restored != *source.selected_plan() {
        return Err(BypassRelocationError::ReplayMismatch);
    }
    Ok(ValidatedBypassRelocation {
        receipt: BypassRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
