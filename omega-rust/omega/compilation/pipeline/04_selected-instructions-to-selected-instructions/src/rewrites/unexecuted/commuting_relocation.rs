//! In-block relocation of a run under proven memory commutation on the
//! selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! module proves is the bounded move with more than one accounted memory
//! actor: the named `first_member` and `last_member` bound one contiguous
//! run in their block, the named `destination` instruction sits outside the
//! run, and the run takes the destination's position as one body — the
//! crossed positions shifting one run-width toward the run's vacated span in
//! their original order. Naming one member as both bounds is the run of one,
//! so the single-member move is this admission's own granularity rather than
//! a family of its own.
//!
//! `super::relocation` proves the same in-block geometry with the narrower
//! memory rule, where every position a row-carrying run trades order with
//! must be row-less. This module widens exactly that rule: accounted actors
//! may face each other when every pair of roster rows that newly trades
//! order commutes, the audit shared through `rewrites/commuting_accesses`
//! with `rewrites/interchange`. The two stay disjoint because a window whose
//! trading pairs carry at most one rowed side refuses here, so the narrower
//! rule keeps its own cases. Retiring this module means giving `relocation`
//! the commutation audit in place of the row-less refusal, which would widen
//! its cross-block window audit at the same time; the commutation premise
//! must survive that, since falling back on the common memory-ordering
//! refusal disables the valid cases this module exists for.
//!
//!
//! Two recorded accesses commute when neither can observe the other's
//! effect on the bytes it reaches — two non-writing rows never conflict,
//! rows reaching provably distinct storage never conflict, and rows on
//! shared storage must be disjoint fixed extents. The run's members are
//! the only instructions whose position relative to the crossed run
//! changes, so the only row pairs that newly trade order are each member's
//! against each crossed position's; members keep their relative order
//! inside the run — internally coupled members move together through a
//! window neither could cross alone — crossed positions keep their
//! relative order with each other and with every position outside the
//! window, and positions before the window are untouched. Because every
//! traded row pair commutes, the relative order of conflicting accesses is
//! unchanged: none of the traded pairs conflict. The roster itself then
//! follows the new execution order — the rows naming the window's
//! instructions are permuted to match, each instruction's own rows keeping
//! their relative order — so the recorded accesses still appear in the
//! order the program performs them.
//!
//! Register and condition-state hazards, barriers, and settlements keep
//! the run relocation's audit unchanged: a register or unit a member
//! writes may not be read or written by a crossed position in either
//! direction, and one a member reads may not be written there — calls,
//! hosted effects, terminator kinds, and call-roster entries never sit in
//! the window, and a boundary settlement inside the window's span would
//! observe a different executed prefix. Since no location is written by
//! both a member and a crossed position, the last writer before every
//! later observer — the block's terminator and edge transports included —
//! is the same instruction it was before the move.
//!
//! No instruction, register, constraint key, call, settlement, or edge
//! record changes: every member keeps its own id, kind, operands,
//! provenance, and implicit surface while only the run's position in the
//! block vector moves, and each roster row keeps its own identity while
//! only its position in the access vector follows the move.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block's
//! window and the roster's window rows to equal the independently computed
//! rotation, and restores the complete source by content.

use crate::rewrites::block_edges::member_position;
mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget};
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{
    SelectedFunction, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity,
};
use semantic_vocabulary::FuelScheduleIdentity;

use crate::ValidatedSelectedAnalysis;
use crate::rewrites::unexecuted::commuting_accesses as accesses;
use crate::rewrites::unexecuted::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

pub(crate) use validation::validate_commuting_relocation;

/// An accepted commuting run relocation with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedCommutingRelocation {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: CommutingRelocationReceipt,
}

validated_transformed_receipt!(ValidatedCommutingRelocation, CommutingRelocationReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommutingRelocationReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(CommutingRelocationReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommutingRelocationError {
    SourceMismatch,
    /// A window position can never trade order: a barrier kind, a
    /// call-roster entry, or a memory-capable kind the roster does not
    /// account for — as a run member or as a crossed position.
    UnsupportedInstruction,
    /// The named instructions do not bound an admissible commuting
    /// window: absent or misordered in the run's block, a run of one
    /// member — the commuting relocation's granularity — a destination
    /// inside the run or outside the block, a member coupled by a
    /// register or condition-state hazard with a crossed position, a
    /// roster row that newly trades order not commuting with a crossed
    /// position's row, no rowed trading pair — the run relocation's own
    /// accounting case — or a boundary settlement inside the window's
    /// span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for CommutingRelocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid commuting run relocation: {self:?}")
    }
}

impl std::error::Error for CommutingRelocationError {}

/// The validator's own reconstruction of the relocation the contract
/// permits: the admitted run's and destination's coordinates inside
/// their block. It shares no state with the producer's `admission`
/// record.
struct Reconstructed<'source> {
    function: &'source SelectedFunction,
    block_index: usize,
    /// The run's first member index in the block body.
    first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    last_index: usize,
    /// The index whose instruction the run displaces: the window the
    /// relocation crosses is `first_index..=destination_index` in either
    /// order, with the destination always outside the run.
    destination_index: usize,
}

/// Reconstruct the legality of relocating the run `first_member` and
/// `last_member` bound to `destination` from first principles: locate
/// the run and the destination in one block in the named order, require
/// every window position schedulable, refuse any register or
/// condition-state coupling between a member and a crossed position and
/// any roster row that does not commute with a crossed position's row,
/// require at least one rowed trading pair — the accounting case that
/// keeps this family disjoint — and refuse a boundary settlement inside
/// the window's span. Nothing in this audit reads the producer's
/// admission decision.
fn reconstruct<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Reconstructed<'source>, CommutingRelocationError> {
    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            CommutingRelocationError::SourceMismatch
        })?;
    let (block_index, first_index) =
        member_position(function, first_member).ok_or(CommutingRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction relocation the sibling family already proves.
    let last_index = block
        .instruction_index(last_member)
        .filter(|position| *position >= first_index)
        .ok_or(CommutingRelocationError::UnsupportedPair)?;
    // The destination sits outside the run in the same block; inside the
    // run it would name a member's own position, and outside the block it
    // names no in-block window.
    let destination_index = block
        .instruction_index(destination)
        .filter(|position| !(first_index..=last_index).contains(position))
        .ok_or(CommutingRelocationError::UnsupportedPair)?;
    let (first, last) = (
        first_index.min(destination_index),
        last_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    let run_len = last_index - first_index + 1;
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the run
    // relocation enforces: no barrier kind, no call contract, and no
    // unaccounted memory reach. What changes here is only the accounting
    // rule — a roster-carrying member may trade order with a crossed
    // position whose own rows commute with it.
    for position in window {
        schedulable(function, position).ok_or(CommutingRelocationError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member trades order with every crossed position, so each
    // direction of every register and condition-state hazard applies
    // against each pair, and every row the member carries must commute
    // with every row the crossed position carries. Members of the run keep
    // their relative order and never face this audit against each other,
    // and crossed positions keep their relative order with each other. A
    // window in which no trading pair is rowed on both sides is the run
    // relocation's own accounting case and stays with it.
    let mut rowed_trade = false;
    for (member_index, member) in window.iter().enumerate() {
        if !(first_index - first..first_index - first + run_len).contains(&member_index) {
            continue;
        }
        for (crossed_index, crossed) in window.iter().enumerate() {
            if (first_index - first..first_index - first + run_len).contains(&crossed_index) {
                continue;
            }
            if coupled(member, crossed) {
                return Err(CommutingRelocationError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingRelocationError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingRelocationError::UnsupportedPair);
    }
    if interior_settlement(function, block.id, first + 1..=last) {
        return Err(CommutingRelocationError::UnsupportedPair);
    }
    // The audit walks the function's body and terminator instructions once;
    // the window audit touches every member-against-crossed operand,
    // unit, and roster-row surface plus the function's three rosters.
    let run_range = first_index - first..first_index - first + run_len;
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            window
                .iter()
                .enumerate()
                .try_fold(total, |total, (member_index, member)| {
                    if !run_range.contains(&member_index) {
                        return Some(total);
                    }
                    window
                        .iter()
                        .enumerate()
                        .try_fold(total, |total, (crossed_index, crossed)| {
                            if run_range.contains(&crossed_index) {
                                return Some(total);
                            }
                            total
                                .checked_add(surface(member))?
                                .checked_add(surface(crossed))?
                                .checked_add(
                                    rows[member_index]
                                        .len()
                                        .checked_mul(rows[crossed_index].len())?,
                                )
                        })
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(CommutingRelocationError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || CommutingRelocationError::IdentityOverflow,
        || CommutingRelocationError::WorkBudgetExceeded,
    )?;
    Ok(Reconstructed {
        function,
        block_index,
        first_index,
        last_index,
        destination_index,
    })
}
