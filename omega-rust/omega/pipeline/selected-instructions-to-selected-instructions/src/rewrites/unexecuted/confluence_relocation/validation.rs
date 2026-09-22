//! Independent validation of confluence relocation.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! relocation's legality from the source records — the member's
//! coordinates inside its block, the unconditional `Jump` whose plain
//! semantic edge reaches a join at least one other predecessor's edge
//! also feeds, the join's own gates, the landing index the destination
//! names, the pure-work bound the speculating member must satisfy, the
//! shared crossed-window audit every crossed position and edge must
//! pass, and the forward dead-path audit proving every member-written
//! location dead from the landing index on the shared continuations —
//! then rebuilds the function the contract demands and requires the
//! proposal to equal it. Restoring the member to its source position
//! must reproduce the complete source by content. A producer admission
//! error therefore fails validation even when the proposal is exactly
//! what that producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionPlan, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    ConfluenceRelocationError, ConfluenceRelocationReceipt, ValidatedConfluenceRelocation,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    CrossingDirection, all_edges, crossed_window, edge_surface, plain_edge, terminator_instruction,
    terminator_successors,
};
use crate::rewrites::unexecuted::dead_path;
use crate::rewrites::window_hazards::{
    RunRelocationRejection, admit_run_relocation, register_writes, schedulable, surface,
};

/// The validator's own reconstruction of the relocation the contract
/// permits: the admitted member's coordinates, the join its lone `Jump`
/// edge reaches, and the landing index the destination names. It shares
/// no state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The member's own block: one inflow of the join.
    block_index: usize,
    /// The member's index inside that block's body.
    member_index: usize,
    /// The join's index in `function.blocks`.
    target_index: usize,
    /// The destination instruction's position in the join body: the
    /// member lands at this index. Naming the join's terminator-carried
    /// instruction lands the member at the body end, index
    /// `target.instructions.len()`.
    landing_index: usize,
}

/// Keeps the family's typed rejection vocabulary over the shared audit's
/// rejection kinds: an unschedulable member or crossed position is the
/// instruction-level refusal and every window-level refusal is the pair
/// kind.
fn reject(rejection: RunRelocationRejection) -> ConfluenceRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => ConfluenceRelocationError::UnsupportedInstruction,
        RunRelocationRejection::UnreachableDestination
        | RunRelocationRejection::Coupled
        | RunRelocationRejection::MemoryOrdering
        | RunRelocationRejection::TransportConflict
        | RunRelocationRejection::NonPlainEdge
        | RunRelocationRejection::Settlement => ConfluenceRelocationError::UnsupportedPair,
    }
}

/// Whether the member's effect is pure register and condition-state work
/// that adds no observable execution on the arrivals it never ran on.
/// `schedulable` already cleared barrier kinds, call contracts, and
/// unaccounted memory-capable kinds, but a row-less load or private-slot
/// `Store64` still performs a memory access: sinking it would add the
/// access — and any fault or slot write it carried — to every traversal
/// entering the join through the other inflows. The same holds for kinds
/// whose target encoding may architecturally fault: their proof
/// obligations establish definedness for the source operation, but this
/// audit runs at the selected level where the encoded trap behavior is
/// the honest bound — an execution that could fault must still run only
/// on the paths that ran it before.
fn speculatable(instruction: &SelectedInstruction) -> bool {
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
            | ExactDivideI64 { .. }
            | ExactRemainderI64 { .. }
            | SaturatingDivide { .. }
            | SaturatingRemainder { .. }
    )
}

/// Reconstruct the legality of relocating `member` into the confluence
/// join `destination` sits in, from first principles: locate the member,
/// require its block to end in an unconditional `Jump` through a plain
/// semantic edge to a join at least one other predecessor's edge also
/// reaches, require the member to be schedulable pure register and
/// condition-state work that can never fault on the arrivals it newly
/// runs on, resolve the landing index the destination names, then run
/// the audits the gates leave — the shared `crossed_window` derivation
/// over the lone `Jump` edge and `admit_run_relocation`'s hazard,
/// memory-roster, transport, and settlement checks, followed by the
/// forward dead-path audit that holds every member-written location
/// unread-until-rewritten on the shared continuations — and account the
/// family's measured steps against the budget. Nothing in this audit
/// reads the producer's admission decision, so a producer-side legality
/// error fails here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, ConfluenceRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConfluenceRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConfluenceRelocationError::SourceMismatch)?;
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
        .ok_or(ConfluenceRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    let member_instruction = &block.instructions[member_index];
    // Only an unconditional semantic jump edge keeps the member's
    // execution count on this inflow's path: every traversal of the
    // member's block leaves through it and reaches the join exactly once.
    // A conditional terminator keeps a second exit the member would still
    // execute on — the fork, diamond, and triangle families' shapes — and
    // a terminator naming no successors never reaches a join.
    let SelectedTerminator::Jump { successor, .. } = &block.terminator else {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if !plain_edge(successor) {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    let target_index = function
        .blocks
        .iter()
        .position(|candidate| candidate.id == successor.block)
        .ok_or(ConfluenceRelocationError::SourceMismatch)?;
    let target = &function.blocks[target_index];
    // A self-edge is the in-block family's case with a back-edge
    // transport reading, not a cross-edge window. The entry block is
    // reached with no predecessor at all, and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    // At least one edge into the join must leave another block: the
    // single-predecessor shape is the edge family's case, where no
    // traversal speculates. Every other inflow — whatever it carries —
    // runs before the landing index on its own arrivals and never crosses
    // the member's window; the dead-path audit reads its register surface
    // where a loop reaches it.
    if !all_edges(function)
        .any(|(source_block, edge)| edge.block == target.id && source_block != block.id)
    {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
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
        .ok_or(ConfluenceRelocationError::UnsupportedPair)?;
    // The member's execution newly runs on every arrival through the
    // join's other inflows, so only pure register and condition-state
    // work may sink — a roster-carrying or unaccounted memory access or a
    // potentially-faulting kind that ran only on this inflow would run on
    // every arrival after the move.
    if schedulable(function, member_instruction) != Some(false) || !speculatable(member_instruction)
    {
        return Err(ConfluenceRelocationError::UnsupportedInstruction);
    }
    // The crossed window is the shared derivation rather than this
    // family's own enumeration: the member is the one-member run, and the
    // gates above leave the lone `Jump` edge as the only acyclic path
    // from the member's block to the join, so the terminator's own
    // successor count bounds the walk. The shared audit applies the
    // schedulable, hazard, memory-roster, transport, and settlement
    // checks once — the `Jump` terminator instruction is the crossed
    // edge's own position, exempt from the barrier-kind rule but not the
    // call or hazard audit — and a boundary settlement positioned past
    // the member's index in its own block or past the landing index in
    // the join observed a changed executed prefix and refuses.
    let edge_limit = terminator_successors(&block.terminator).len();
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
    .ok_or(ConfluenceRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(reject)?;
    // The dead-path audit: every location the member writes must be dead
    // — unread until rewritten — from the landing index forward, on the
    // shared continuations every inflow reaches.
    if !dead_path::dead(
        function,
        dead_path::Relocation {
            members: &[member_instruction],
            vacated_block: block_index,
            vacated_first: member_index,
            vacated_last: member_index,
            landing_block: target_index,
            landing_index,
            landing: dead_path::Landing::Speculated,
            // The vacated index stays silent: every position behind it in
            // the member's own block is a crossed window position the
            // hazard audit already owns.
            vacated: dead_path::Vacated::Silent,
        },
        dead_path::Start::Block(target_index),
    ) {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the member, and again to locate the join and its other
    // inflow; the path walk pushes the lone `Jump` edge once; the window
    // audit walks the member's surface against each crossed position's
    // and each crossed edge's own surface, plus the function's three
    // rosters; the dead-path audit rescans a block's stream and edge
    // surfaces only while its entry set grows — at most once per member
    // location per block.
    let member_locations = register_writes(member_instruction).count()
        + member_instruction.implicit_defs.len()
        + member_instruction.clobbers.len();
    let block_scan: usize = function
        .blocks
        .iter()
        .map(|block| {
            block
                .instructions
                .iter()
                .chain(std::iter::once(terminator_instruction(&block.terminator)))
                .map(surface)
                .sum::<usize>()
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
        .and_then(|total| {
            total.checked_add(block_scan.saturating_mul(member_locations.saturating_add(1)))
        })
        .ok_or(ConfluenceRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ConfluenceRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ConfluenceRelocationError::WorkBudgetExceeded);
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
/// roster row, call, settlement, and edge included. The producer's
/// `admission::admit` is never consulted, so a wrong legality decision
/// fails here even when the proposal matches the edit the producer
/// emitted.
pub fn validate_confluence_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConfluenceRelocation, ConfluenceRelocationError> {
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
        return Err(ConfluenceRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let member_instruction = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .remove(reconstructed.landing_index);
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .insert(reconstructed.member_index, member_instruction);
    if restored != *source.selected_plan() {
        return Err(ConfluenceRelocationError::ReplayMismatch);
    }
    Ok(ValidatedConfluenceRelocation {
        receipt: ConfluenceRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
