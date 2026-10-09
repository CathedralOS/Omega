//! Independent replay of the inflow relocation: the validator
//! reconstructs the member's join block, the inflow predecessor the
//! destination names, and the landing index inside it from the source on
//! its own audit — sharing no state with the producer's `admission`
//! record — then requires the proposed program to place exactly the
//! member on the landing index and restores the complete source by
//! content, every crossed instruction, every other block and
//! instruction, register, roster row, call, settlement, and edge
//! included.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedFunction, SelectedInstructionId, SelectedInstructionPlan, terminator_instruction,
};

use super::{InflowRelocationError, InflowRelocationReceipt, ValidatedInflowRelocation};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{terminator_successors, transport_conflict};
use crate::rewrites::sealed;
use crate::rewrites::window_hazards::{
    block_scan_surface, coupled, has_call_contract, register_writes, schedulable, surface,
};

/// The validator's own reconstruction of the relocation the contract
/// permits: the member's join block and position, the inflow
/// predecessor, and the landing index. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    member_index: usize,
    target_index: usize,
    landing_index: usize,
}

/// Reconstruct the legality of moving `member` onto `destination` inside
/// the inflow predecessor from the source records: locate both
/// instructions by identity, re-derive the lone `Jump` edge, the join
/// requirement, the member's removable kind, the crossed-window
/// independence audit — the crossed edge's register transports, the
/// terminator's call roster, every crossed position's hazards, and no
/// boundary settlement whose observed executed prefix changes — and the
/// dead-path audit over the member's writes, then account the family's
/// measured steps against the budget. Nothing in this audit reads the
/// producer's admission decision, so a producer-side legality error
/// fails here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, InflowRelocationError> {
    let derived =
        super::derive_inflow_span(source, function_index, member, destination, environment)?;
    let plan = derived.plan;
    let function = derived.function;
    let block_index = derived.block_index;
    let member_index = derived.member_index;
    let member_instruction = derived.member_instruction;
    let target_index = derived.target_index;
    let landing_index = derived.landing_index;
    let successor = derived.successor;
    let terminator = derived.terminator;
    let block = &function.blocks[block_index];
    let target = &function.blocks[target_index];
    if transport_conflict(member_instruction, successor) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the call or hazard
    // audit. Its memory rows, and any rows the roster logs with the
    // edge's own origin, are accounted positions a row-less member
    // crosses without reordering a recorded access.
    if has_call_contract(function, terminator.id) {
        return Err(InflowRelocationError::UnsupportedInstruction);
    }
    if coupled(member_instruction, terminator) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The member trades order with the positions at and after the
    // landing index in the inflow body and the positions before its index
    // in its own body. Every other position keeps the member on the side
    // it always had. A roster-carrying crossed position is an accounted
    // access the row-less member cannot reorder, so only the
    // schedulability and hazard gates apply.
    for crossed in target.instructions[landing_index..]
        .iter()
        .chain(block.instructions[..member_index].iter())
    {
        schedulable(function, crossed).ok_or(InflowRelocationError::UnsupportedInstruction)?;
        if coupled(member_instruction, crossed) {
            return Err(InflowRelocationError::UnsupportedPair);
        }
    }
    // A settlement positioned past the member's index observed it inside
    // the source block's executed prefix; a settlement positioned past
    // the landing index observes it inside the inflow block's — on this
    // inflow's arrivals, where it has not run yet. Both refuse; positions
    // at or before either boundary keep the executed set they always had.
    // The other inflow blocks are unaffected: the member never enters
    // their streams.
    if function.boundary_settlements.iter().any(|settlement| {
        (settlement.block == block.id && settlement.instruction_index as usize > member_index)
            || (settlement.block == target.id
                && settlement.instruction_index as usize > landing_index)
    }) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The dead-path audit: every location the member writes must be dead
    // — unread until rewritten — on every arrival through the join's
    // other inflows.
    if !super::member_writes_dead(function, &derived) {
        return Err(InflowRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the member, and again with successor edges to locate the
    // destination and find the join's other inflows; the window audit
    // walks the member's surface against each crossed position's; the
    // dead-path audit rescans a block's stream and edge surfaces only
    // while its entry set grows — at most once per member location per
    // block.
    let member_locations = register_writes(member_instruction).count()
        + member_instruction.implicit_defs.len()
        + member_instruction.clobbers.len();
    let block_scan = block_scan_surface(function);
    let steps = crate::rewrites::rewrite_scan(plan, function)
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
                total.checked_add(terminator_successors(&candidate.terminator).len())
            })
        })
        .and_then(|total| {
            target.instructions[landing_index..]
                .iter()
                .chain(block.instructions[..member_index].iter())
                .chain(std::iter::once(terminator))
                .try_fold(total, |total, crossed| {
                    total
                        .checked_add(surface(member_instruction))?
                        .checked_add(surface(crossed))
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .and_then(|total| total.checked_add(successor.bindings.len()))
        .and_then(|total| {
            total.checked_add(block_scan.saturating_mul(member_locations.saturating_add(1)))
        })
        .ok_or(InflowRelocationError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || InflowRelocationError::IdentityOverflow,
        || InflowRelocationError::WorkBudgetExceeded,
    )?;
    Ok(Reconstructed {
        function,
        block_index,
        member_index,
        target_index,
        landing_index,
    })
}

/// Independently consume the proposed program: the validator's own
/// reconstruction re-derives the member's block, inflow block, and
/// landing index from the source, the proposal must place exactly the
/// member's instruction on the landing index in the inflow block, and
/// moving it back must restore the complete source by content — every
/// crossed instruction, every other block and instruction, register,
/// roster row, call, settlement, and edge included.
pub(crate) fn validate_inflow_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedInflowRelocation, InflowRelocationError> {
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
        return Err(InflowRelocationError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    let member_instruction = restored.functions[function_index].blocks[reconstructed.target_index]
        .instructions
        .remove(reconstructed.landing_index);
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .insert(reconstructed.member_index, member_instruction);
    if restored != *source.selected_plan() {
        return Err(InflowRelocationError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedInflowRelocation,
        InflowRelocationReceipt,
        source,
        proposed
    ))
}
