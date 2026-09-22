//! Independent replay of the confluence run relocation: the validator
//! reconstructs the run's block, the join the named destination sits in,
//! and the landing index from the source on its own audit — sharing no
//! state with the producer's `admission` record — then requires the
//! proposed program to place exactly the run's members on the landing
//! index in the join block in their original order, and restores the
//! complete source by content, every crossed instruction, every other
//! block and instruction, register, roster row, call, settlement, and
//! edge included. A producer admission error therefore fails validation
//! even when the proposal is exactly what that producer emitted.
use std::sync::Arc;

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedBlockOrigin, SelectedFunction, SelectedInstruction, SelectedInstructionId,
    SelectedInstructionPlan, SelectedTerminator,
};
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    ConfluenceRunRelocationError, ConfluenceRunRelocationReceipt, ValidatedConfluenceRunRelocation,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{
    all_edges, edge_surface, plain_edge, terminator_instruction, terminator_successors,
    transport_conflict,
};
use crate::rewrites::unexecuted::dead_path;
use crate::rewrites::window_hazards::{
    coupled, has_call_contract, register_writes, schedulable, speculatable, surface,
};

/// The validator's own reconstruction of the relocation the contract
/// permits: the run's block and contiguous span, the join the crossed
/// `Jump` edge reaches, and the landing index inside it. It shares no
/// state with the producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    /// The run's own block: one inflow of the join.
    block_index: usize,
    /// The run's first member index in the block body.
    first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    last_index: usize,
    /// The join's index in `function.blocks`.
    target_index: usize,
    /// The destination instruction's position in the join body: the run
    /// lands at this index. Naming the join's terminator-carried
    /// instruction lands the run at the body end, index
    /// `target.instructions.len()`.
    landing_index: usize,
}

/// Reconstruct the legality of sinking the run `first_member..=last_member`
/// into the join `destination` names from the source records: locate the
/// bounding members by identity inside one block's body, require that
/// block to end in an unconditional `Jump` on a plain semantic edge to a
/// source join at least one other predecessor's edge also reaches,
/// resolve the landing index in the join body, then re-derive the move's
/// soundness in both directions — every member pure register and
/// condition-state work, no member interference with the crossed edge's
/// register transports, no call contract or hazard against the `Jump`
/// instruction, every member's hazard directions against the positions
/// behind the run and before the landing index, no boundary settlement
/// whose observed executed prefix changes, and every member-written
/// location dead from the landing index forward — then account the
/// family's measured steps against the budget. Nothing in this audit
/// reads the producer's admission decision, so a producer-side legality
/// error fails here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, ConfluenceRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(ConfluenceRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(ConfluenceRunRelocationError::SourceMismatch)?;
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
        .ok_or(ConfluenceRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction confluence relocation the sibling family
    // already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(ConfluenceRunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Only an unconditional semantic jump edge keeps the run's execution
    // count on this inflow's path: every traversal of the run's block
    // leaves through it and reaches the join exactly once. A conditional
    // terminator keeps a second exit the run would still execute on —
    // the fork, diamond, and triangle families' shapes — and a
    // terminator naming no successors never reaches a join.
    let SelectedTerminator::Jump {
        instruction: terminator,
        successor,
    } = &block.terminator
    else {
        return Err(ConfluenceRunRelocationError::UnsupportedPair);
    };
    // The crossed edge must be a plain semantic successor: case dispatch,
    // continuation, structural transfer, and per-edge fuel all carry
    // boundary effects this step does not cross. Structural bindings may
    // remain only while every transport is `Unused`, which moves nothing.
    if !plain_edge(successor) {
        return Err(ConfluenceRunRelocationError::UnsupportedPair);
    }
    let target_index = function
        .blocks
        .iter()
        .position(|candidate| candidate.id == successor.block)
        .ok_or(ConfluenceRunRelocationError::SourceMismatch)?;
    let target = &function.blocks[target_index];
    // A self-edge is the in-block family's case with a back-edge
    // transport reading, not a cross-edge window. The entry block is
    // reached with no predecessor at all, and an implementation block's
    // origin carries edge or case work the bounded audit does not cross.
    if target.id == block.id
        || target.id == function.entry_block
        || !matches!(target.origin, SelectedBlockOrigin::Source(_))
    {
        return Err(ConfluenceRunRelocationError::UnsupportedPair);
    }
    // At least one edge into the join must leave another block: the
    // single-predecessor shape is the edge family's case, where no
    // traversal speculates. Every other inflow — whatever it carries —
    // runs before the landing index on its own arrivals and never crosses
    // the run's window; the dead-path audit reads its register surface
    // where a loop reaches it.
    if !all_edges(function)
        .any(|(source_block, edge)| edge.block == target.id && source_block != block.id)
    {
        return Err(ConfluenceRunRelocationError::UnsupportedPair);
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
        .ok_or(ConfluenceRunRelocationError::UnsupportedPair)?;
    // The run's execution newly runs on every arrival through the join's
    // other inflows, so only pure register and condition-state work may
    // sink — a roster-carrying or unaccounted memory access or a
    // potentially-faulting kind that ran only on this inflow would run on
    // every arrival after the move. Every member meets the bar itself;
    // the run then carries no roster rows at all, so it crosses any
    // accounted mix of positions without reordering a recorded access.
    for member in run {
        if schedulable(function, member) != Some(false) || !speculatable(member) {
            return Err(ConfluenceRunRelocationError::UnsupportedInstruction);
        }
    }
    // The crossed edge's register transports sit between the run's old
    // and new positions.
    for member in run {
        if transport_conflict(member, successor) {
            return Err(ConfluenceRunRelocationError::UnsupportedPair);
        }
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the call or hazard
    // audit. Its memory rows, and any rows the roster logs with the
    // edge's own origin, are accounted positions the row-less run crosses
    // without reordering a recorded access.
    if has_call_contract(function, terminator.id) {
        return Err(ConfluenceRunRelocationError::UnsupportedInstruction);
    }
    for member in run {
        if coupled(member, terminator) {
            return Err(ConfluenceRunRelocationError::UnsupportedPair);
        }
    }
    // Every member trades order with the positions behind the run in its
    // own body and the positions before the landing index in the join
    // body. Every other position keeps the run on the side it always had.
    // A roster-carrying crossed position is an accounted access the
    // row-less run cannot reorder, so only the schedulability and hazard
    // gates apply.
    for crossed in block.instructions[last_index + 1..]
        .iter()
        .chain(target.instructions[..landing_index].iter())
    {
        schedulable(function, crossed)
            .ok_or(ConfluenceRunRelocationError::UnsupportedInstruction)?;
        for member in run {
            if coupled(member, crossed) {
                return Err(ConfluenceRunRelocationError::UnsupportedPair);
            }
        }
    }
    // A settlement positioned past the run's first index observed a
    // member inside the source block's executed prefix; a settlement
    // positioned past the landing index observes the run inside the
    // join's — on the other inflows' arrivals included, where the run
    // never ran before. Both refuse; positions at or before either
    // boundary keep the executed set they always had. The other inflow
    // blocks are unaffected: the run never enters their streams.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > first_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(ConfluenceRunRelocationError::UnsupportedPair);
    }
    // The dead-path audit: every location any member writes must be dead
    // — unread until rewritten — from the landing index forward, on the
    // shared continuations every inflow reaches. The run publishes the
    // union of its members' locations.
    let members: Vec<&SelectedInstruction> = run.iter().collect();
    if !dead_path::dead(
        function,
        dead_path::Relocation {
            members: &members,
            vacated_block: block_index,
            vacated_first: first_index,
            vacated_last: last_index,
            landing_block: target_index,
            landing_index,
            landing: dead_path::Landing::Speculated,
            // The vacated span stays silent: every position behind it in
            // the run's own block is a crossed window position the hazard
            // audit already owns, and no surviving position sits inside
            // the span.
            vacated: dead_path::Vacated::Silent,
        },
        dead_path::Start::Block(target_index),
    ) {
        return Err(ConfluenceRunRelocationError::UnsupportedPair);
    }
    // The validator's own audit walks the same surfaces the family
    // publishes: every block body and terminator instruction once to
    // locate the run's first member, and again with successor edges to
    // find the join's other inflow; every member's surface against each
    // crossed position's; the dead-path audit's rescan of a block's
    // stream and edge surfaces only while its entry set grows — at most
    // once per member location per block.
    let run_locations: usize = run
        .iter()
        .map(|member| {
            register_writes(member).count() + member.implicit_defs.len() + member.clobbers.len()
        })
        .sum();
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
        .and_then(|total| {
            run.iter().try_fold(total, |total, member| {
                block.instructions[last_index + 1..]
                    .iter()
                    .chain(target.instructions[..landing_index].iter())
                    .chain(std::iter::once(terminator))
                    .try_fold(total, |total, crossed| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(crossed))
                    })
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .and_then(|total| total.checked_add(successor.bindings.len().saturating_mul(run.len())))
        .and_then(|total| {
            total.checked_add(block_scan.saturating_mul(run_locations.saturating_add(1)))
        })
        .ok_or(ConfluenceRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| ConfluenceRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(ConfluenceRunRelocationError::WorkBudgetExceeded);
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

/// Independently consume the proposed program: the validator's own
/// reconstruction re-derives the admitted run, join, and landing index
/// from the source without the producer's admission routine, the
/// proposal must place exactly the run's members on the landing index in
/// the join block in their original order, and moving the run back must
/// restore the complete source by content — every crossed instruction,
/// every other block and instruction, register, roster row, call,
/// settlement, and edge included.
pub fn validate_confluence_run_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConfluenceRunRelocation, ConfluenceRunRelocationError> {
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
        return Err(ConfluenceRunRelocationError::ReplayMismatch);
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
        return Err(ConfluenceRunRelocationError::ReplayMismatch);
    }
    Ok(ValidatedConfluenceRunRelocation {
        receipt: ConfluenceRunRelocationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
