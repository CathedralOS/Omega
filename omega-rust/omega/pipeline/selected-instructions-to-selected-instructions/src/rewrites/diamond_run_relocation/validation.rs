//! Independent validation of cross-diamond run relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the contiguous run
//! the named members bound inside the branching head, the two-successor
//! conditional whose plain semantic edges alone feed each arm, the
//! unconditional `Jump` every arm ends in on one common join reached by
//! arm edges alone, the landing index the destination names in the
//! join's body, the shared crossed-window audit every member and crossed
//! position and edge must pass, and the boundary-settlement exclusion —
//! then requires the proposal to place exactly the run's members on the
//! landing index in the join block in their original order, and moving
//! the run back must restore the complete source by content — every
//! crossed instruction, every other block and instruction, register,
//! roster row, call, settlement, and edge included. A producer admission
//! error therefore fails validation even when the proposal is exactly
//! what that producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
    SelectedSuccessor, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    DiamondRunRelocationError, DiamondRunRelocationReceipt, ValidatedDiamondRunRelocation,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation, surface};

/// The validator's own reconstruction of the relocation the contract
/// permits: the run's own block — the diamond's branching head — the
/// run's contiguous span in its body, the join the arms alone feed, and
/// the landing index the destination names there. It shares no state
/// with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The run's own block: the diamond's branching head.
    block_index: usize,
    /// The run's first member index in the block body.
    first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    last_index: usize,
    /// The join block's index in `function.blocks`.
    target_index: usize,
    /// The destination instruction's position in the join body: the run
    /// lands at this index. Naming the join's terminator-carried
    /// instruction lands the run at the body end, index
    /// `target.instructions.len()`.
    landing_index: usize,
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn reject(rejection: RunRelocationRejection) -> DiamondRunRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => DiamondRunRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => DiamondRunRelocationError::UnsupportedPair,
    }
}

/// Reconstruct the legality of relocating the run `first_member` and
/// `last_member` bound onto `destination` from the source records:
/// locate the run's bounding members by identity inside one block's
/// body, require that block to end in a two-successor conditional
/// branch whose distinct targets — the arms — are each reached by that
/// branch's edges alone and each end in an unconditional `Jump` to one
/// common join reached by arm edges alone, resolve the landing index in
/// the join's body, and re-derive the window's independence audit
/// through the shared `crossed_window`/`admit_run_relocation` contract —
/// no register or condition-state hazard between any member and any
/// crossed position, no member interference with any crossed edge's
/// register transports, no roster-carrying run sharing the window with a
/// second memory-access actor, no barrier, call, hosted effect, or
/// call-roster entry inside the window, and no boundary settlement whose
/// observed executed prefix changes — then account the family's
/// measured steps against the budget. Nothing in this audit reads the
/// producer's admission decision, so a producer-side legality error
/// fails here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, DiamondRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(DiamondRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(DiamondRunRelocationError::SourceMismatch)?;
    let (block_index, first_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == first_member)
                .map(|first_index| (block_index, first_index))
        })
        .ok_or(DiamondRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction cross-diamond relocation the sibling family
    // already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(DiamondRunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Only a two-successor conditional terminator gives the run's block
    // the branch this step crosses; every other terminator shape is the
    // single-edge family's case or no relocation at all.
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
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
    };
    for edge in &branch_edges {
        if !plain_edge(edge) {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
    }
    // The arms are the branch's distinct targets: both edges naming one
    // block is a degenerate diamond with a single arm. Each arm must be a
    // plain source block the branch alone reaches — a second predecessor
    // would give the join a path the run never executed on, the run's own
    // block would make the move self-referential, the entry block is
    // reached with no predecessor at all, and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    let mut arm_indices: Vec<usize> = Vec::new();
    for edge in &branch_edges {
        let arm_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == edge.block)
            .ok_or(DiamondRunRelocationError::SourceMismatch)?;
        if !arm_indices.contains(&arm_index) {
            arm_indices.push(arm_index);
        }
    }
    for &arm_index in &arm_indices {
        let arm = &function.blocks[arm_index];
        if arm.id == block.id
            || arm.id == function.entry_block
            || !matches!(arm.origin, SelectedBlockOrigin::Source(_))
        {
            return Err(DiamondRunRelocationError::UnsupportedPair);
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
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
    }
    // Each arm must leave through an unconditional `Jump` to one common
    // join on a plain semantic edge — the converging side of the diamond.
    let mut target_index: Option<usize> = None;
    for &arm_index in &arm_indices {
        let arm = &function.blocks[arm_index];
        let SelectedTerminator::Jump {
            successor: arm_edge,
            ..
        } = &arm.terminator
        else {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        };
        if !plain_edge(arm_edge) {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
        let join_index = function
            .blocks
            .iter()
            .position(|candidate| candidate.id == arm_edge.block)
            .ok_or(DiamondRunRelocationError::SourceMismatch)?;
        match target_index {
            Some(existing) if existing != join_index => {
                return Err(DiamondRunRelocationError::UnsupportedPair);
            }
            Some(_) => {}
            None => target_index = Some(join_index),
        }
    }
    let target_index = target_index.ok_or(DiamondRunRelocationError::UnsupportedPair)?;
    let target = &function.blocks[target_index];
    // The join must not collapse back into the diamond: landing inside the
    // run's own block or inside an arm is an in-block or self-loop move
    // this step does not cross, the entry block is reached with no
    // predecessor at all, and an implementation block's origin carries
    // edge or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == target.id)
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(DiamondRunRelocationError::UnsupportedPair);
    }
    // Every edge into the join must leave an arm: a second predecessor
    // gives the join a path the run would newly execute on.
    let mut join_incoming = 0usize;
    for (source_block, _) in all_edges(function).filter(|(_, edge)| edge.block == target.id) {
        if !arm_indices
            .iter()
            .any(|&arm| function.blocks[arm].id == source_block)
        {
            return Err(DiamondRunRelocationError::UnsupportedPair);
        }
        join_incoming += 1;
    }
    if join_incoming != arm_indices.len() {
        return Err(DiamondRunRelocationError::UnsupportedPair);
    }
    // The destination names a position in the join body — the run lands
    // at its index — or the join's terminator-carried instruction,
    // landing the run at the body end.
    let landing_index = target
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .or_else(|| {
            (terminator_instruction(&target.terminator).id == destination)
                .then_some(target.instructions.len())
        })
        .ok_or(DiamondRunRelocationError::UnsupportedPair)?;
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the gates above leave only
    // head-to-arm-to-join acyclic paths, and the walk pushes each branch
    // edge once and each arm's `Jump` edge once per branch edge feeding
    // it — two pushes per branch edge even when both name the same arm —
    // so twice the branch's own out-edge count bounds it. The shared
    // audit applies the hazard, memory-roster, transport, and settlement
    // checks once across the run's members: a boundary settlement
    // positioned past the run's first index in the head or past the
    // landing index in the join observed a changed executed prefix and
    // refuses, and so does a settlement inside a crossed arm — every
    // member executes after that arm's point after the move where it
    // executed before it before.
    let edge_limit = branch_edges.len() * 2;
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
    .ok_or(DiamondRunRelocationError::WorkBudgetExceeded)?;
    let members: Vec<_> = run.iter().collect();
    admit_run_relocation(function, &members, &crossing).map_err(reject)?;
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the run's first member, and again with successor edges to
    // count the arms' and the join's predecessor edges; the path walk is
    // bounded by two pushes per branch edge; the window audit walks
    // every member-against-crossed operand and unit surface plus each
    // crossed edge's own surface, and the function's three rosters.
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
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(DiamondRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| DiamondRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(DiamondRunRelocationError::WorkBudgetExceeded);
    }
    Ok(Reconstructed {
        function,
        block_index,
        first_index,
        last_index,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted run, diamond, and landing index from the source without
/// the producer's admission routine, the proposal must place exactly the
/// run's members on the landing index in the join block in their
/// original order, and moving the run back must restore the complete
/// source by content — every crossed instruction, every other block and
/// instruction, register, roster row, call, settlement, and edge
/// included.
pub fn validate_diamond_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedDiamondRunRelocation, DiamondRunRelocationError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
        budget,
    )?;
    let run_len = reconstructed.last_index - reconstructed.first_index + 1;
    if proposed
        .functions
        .get(function_index)
        .and_then(|function| function.blocks.get(reconstructed.target_index))
        .and_then(|block| {
            block
                .instructions
                .get(reconstructed.landing_index..reconstructed.landing_index + run_len)
        })
        != Some(
            &reconstructed.function.blocks[reconstructed.block_index].instructions
                [reconstructed.first_index..=reconstructed.last_index],
        )
    {
        return Err(DiamondRunRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let run: Vec<_> = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .drain(reconstructed.landing_index..reconstructed.landing_index + run_len)
        .collect();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(reconstructed.first_index..reconstructed.first_index, run);
    if restored != *source.selected_plan() {
        return Err(DiamondRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedDiamondRunRelocation {
        receipt: DiamondRunRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
