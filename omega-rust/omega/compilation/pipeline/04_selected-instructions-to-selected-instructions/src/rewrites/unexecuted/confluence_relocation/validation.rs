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

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};

use super::{
    ConfluenceRelocationError, ConfluenceRelocationReceipt, ValidatedConfluenceRelocation,
};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::transport_conflict;
use crate::rewrites::sealed;
use crate::rewrites::window_hazards::{coupled, has_call_contract, schedulable, surface};

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
) -> Result<Reconstructed<'source>, ConfluenceRelocationError> {
    let derived = super::derive_confluence_span(
        source,
        function_index,
        first_member,
        last_member,
        destination,
        environment,
    )?;
    let plan = derived.plan;
    let function = derived.function;
    let block_index = derived.block_index;
    let first_index = derived.first_index;
    let last_index = derived.last_index;
    let run = derived.run;
    let target_index = derived.target_index;
    let landing_index = derived.landing_index;
    let successor = derived.successor;
    let terminator = derived.terminator;
    let block = &function.blocks[block_index];
    let target = &function.blocks[target_index];
    for member in run {
        if transport_conflict(member, successor) {
            return Err(ConfluenceRelocationError::UnsupportedPair);
        }
    }
    // The `Jump` instruction itself is the crossed edge's position: it is
    // exempt from the barrier-kind rule but not from the call or hazard
    // audit. Its memory rows, and any rows the roster logs with the
    // edge's own origin, are accounted positions the row-less run crosses
    // without reordering a recorded access.
    if has_call_contract(function, terminator.id) {
        return Err(ConfluenceRelocationError::UnsupportedInstruction);
    }
    for member in run {
        if coupled(member, terminator) {
            return Err(ConfluenceRelocationError::UnsupportedPair);
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
        schedulable(function, crossed).ok_or(ConfluenceRelocationError::UnsupportedInstruction)?;
        for member in run {
            if coupled(member, crossed) {
                return Err(ConfluenceRelocationError::UnsupportedPair);
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
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    // The dead-path audit: every location any member writes must be dead
    // — unread until rewritten — from the landing index forward, on the
    // shared continuations every inflow reaches. The run publishes the
    // union of its members' locations.
    if !super::run_writes_dead(function, &derived) {
        return Err(ConfluenceRelocationError::UnsupportedPair);
    }
    // The scan walks every block body and terminator instruction once to
    // locate the run's first member, and again with successor edges to
    // find the join's other inflow; the audit walks every member's
    // surface against each crossed position's; the dead-path audit
    // rescans a block's stream and edge surfaces only while its entry
    // set grows — at most once per member location per block.
    let (scan, run_locations, block_scan) = super::confluence_scan_base(plan, function, run)
        .ok_or(ConfluenceRelocationError::IdentityOverflow)?;
    let total = Some(scan)
        .and_then(|total| total.checked_add(successor.bindings.len().saturating_mul(run.len())))
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
        .ok_or(ConfluenceRelocationError::IdentityOverflow)?;
    super::confluence_settle_steps(total, function, block_scan, run_locations, budget)?;
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
pub(crate) fn validate_confluence_relocation(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedConfluenceRelocation, ConfluenceRelocationError> {
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
        return Err(ConfluenceRelocationError::ReplayMismatch);
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
        return Err(ConfluenceRelocationError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedConfluenceRelocation,
        ConfluenceRelocationReceipt,
        source,
        proposed
    ))
}
