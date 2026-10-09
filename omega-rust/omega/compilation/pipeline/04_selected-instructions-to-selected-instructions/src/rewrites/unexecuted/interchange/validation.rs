//! Replay of the in-block commuting run interchange: re-derive the window
//! the two named runs bound from the source, require the proposed program
//! to place exactly the later run, the interior, and the earlier run in
//! that order inside the window with the roster equal to the source's own
//! rows permuted into the new execution order, then restore the source by
//! content — splicing the source window back and writing the source
//! order's rows back into the window's roster positions must reproduce the
//! complete selected program bit-for-bit.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId, SelectedInstructionPlan};

use super::{InterchangeError, InterchangeReceipt, ValidatedInterchange};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;
use crate::rewrites::unexecuted::commuting_accesses as accesses;

/// The validator's own reconstruction of the interchange the contract
/// permits: the touched block plus the four positions the two runs'
/// named members bound inside it. It shares no state with the
/// producer's `admission` record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    /// The earlier run's first member index in the block body.
    earlier_first_index: usize,
    /// The earlier run's last member index; the earlier run is the
    /// contiguous span `earlier_first_index..=earlier_last_index` of at
    /// least two members.
    earlier_last_index: usize,
    /// The later run's first member index, strictly past the earlier run;
    /// the positions between the runs form the interior, possibly empty.
    later_first_index: usize,
    /// The later run's last member index; the window the interchange
    /// crosses is `earlier_first_index..=later_last_index`.
    later_last_index: usize,
}

/// Reconstruct the legality of trading two disjoint runs under proven
/// memory commutation from the source records: locate each named member
/// by identity inside one block's body, then re-derive the window's
/// independence audit — every window position schedulable, every register
/// and condition-state hazard direction between each member and the
/// positions outside its own run, every roster row that newly trades
/// order commuting with every row of the position it crosses, at least
/// one trading pair rowed on both sides — the accounting case that keeps
/// this family disjoint from the plain run interchange — and no boundary
/// settlement inside the window's span — and account the family's
/// measured steps against the budget. Nothing in this audit reads the
/// producer's admission decision, so a producer-side legality error fails
/// here even when the proposal matches the emitted edit.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, InterchangeError> {
    let derived = super::derive_interchange(
        source,
        function_index,
        earlier_first,
        earlier_last,
        later_first,
        later_last,
        environment,
        budget,
    )?;
    Ok(Reconstructed {
        function: derived.function,
        block_index: derived.block_index,
        earlier_first_index: derived.earlier_first_index,
        earlier_last_index: derived.earlier_last_index,
        later_first_index: derived.later_first_index,
        later_last_index: derived.later_last_index,
    })
}

/// Independently consume the proposed program: the validator re-derives
/// the admitted runs and window from the source without the producer's
/// admission routine, the touched block's window must equal exactly the
/// later run, the interior, and the earlier run in that order, the roster
/// must equal the source's rows permuted into the new execution order,
/// and restoring the window's order and rows must recover the complete
/// source by content: every instruction before and after the window,
/// every other instruction, register, roster row, call, settlement, and
/// function included.
pub(crate) fn validate_selected_interchange(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedInterchange, InterchangeError> {
    let reconstructed = reconstruct(
        source,
        function_index,
        earlier_first,
        earlier_last,
        later_first,
        later_last,
        environment,
        budget,
    )?;
    let source_block = &reconstructed.function.blocks[reconstructed.block_index];
    let proposed_function = proposed
        .functions
        .get(function_index)
        .ok_or(InterchangeError::ReplayMismatch)?;
    let proposed_block = proposed_function
        .blocks
        .get(reconstructed.block_index)
        .ok_or(InterchangeError::ReplayMismatch)?;
    let first = reconstructed.earlier_first_index;
    let last = reconstructed.later_last_index;
    let source_window = source_block
        .instructions
        .get(first..=last)
        .ok_or(InterchangeError::ReplayMismatch)?;
    let earlier_len = reconstructed.earlier_last_index - first + 1;
    let later_len = last - reconstructed.later_first_index + 1;
    let interior_len = source_window.len() - earlier_len - later_len;
    // The proposal's window must be the source window's own instructions
    // rearranged to later-run ++ interior ++ earlier-run — no other member,
    // order, or content.
    let expected: Vec<_> = source_window[earlier_len + interior_len..]
        .iter()
        .chain(&source_window[earlier_len..earlier_len + interior_len])
        .chain(&source_window[..earlier_len])
        .collect();
    if proposed_block
        .instructions
        .get(first..=last)
        .map(|window| window.iter().collect::<Vec<_>>())
        != Some(expected)
    {
        return Err(InterchangeError::ReplayMismatch);
    }
    // The roster is the family's one content change beyond the reorder:
    // the source's window rows, permuted into the new execution order and
    // nothing else. Comparing against the permutation derived from the
    // source — never from the proposal's own grouping — keeps a stale,
    // reordered, or diluted roster from replaying.
    let window: Vec<SelectedInstructionId> = source_window
        .iter()
        .map(|instruction| instruction.id)
        .collect();
    let positions = accesses::window_row_positions(reconstructed.function, &window);
    let new_order: Vec<SelectedInstructionId> = window[earlier_len + interior_len..]
        .iter()
        .chain(&window[earlier_len..earlier_len + interior_len])
        .chain(&window[..earlier_len])
        .copied()
        .collect();
    let ordered = accesses::rows_in_order(reconstructed.function, &new_order);
    if positions.len() != ordered.len() {
        return Err(InterchangeError::ReplayMismatch);
    }
    let mut expected_roster = reconstructed.function.memory_accesses.clone();
    for (position, access) in positions.iter().zip(ordered) {
        expected_roster[*position] = access;
    }
    if proposed_function.memory_accesses != expected_roster {
        return Err(InterchangeError::ReplayMismatch);
    }
    let mut restored = proposed.clone();
    restored.functions[function_index].blocks[reconstructed.block_index]
        .instructions
        .splice(first..=last, source_window.iter().cloned());
    let restored_function = &mut restored.functions[function_index];
    let source_order = accesses::rows_in_order(reconstructed.function, &window);
    for (position, access) in positions.iter().zip(source_order) {
        restored_function.memory_accesses[*position] = access;
    }
    if restored != *source.selected_plan() {
        return Err(InterchangeError::ReplayMismatch);
    }
    Ok(sealed!(
        ValidatedInterchange,
        InterchangeReceipt,
        source,
        proposed
    ))
}
