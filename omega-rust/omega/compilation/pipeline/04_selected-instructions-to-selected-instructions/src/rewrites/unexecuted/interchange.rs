//! In-block interchange of two runs on the selected CFG.
//!
//! Scheduling here means changing the order instructions execute inside one
//! block without changing what any of them observes. The primitive this
//! module proves is the bounded interchange: two disjoint runs of body
//! instructions — each the contiguous span its named first and last members
//! bound — exchange places while every instruction between them keeps its
//! relative order, shifted by the difference of the runs' lengths.
//!
//! Naming one member as both bounds is the run of one, so a member against a
//! run and two members against each other are granularities of this
//! admission rather than families of their own. The memory rule is the
//! commutation audit: every pair of roster rows that newly trades order must
//! commute, shared through `rewrites/commuting_accesses`. A window whose
//! trading pairs carry rows on at most one side satisfies that by having no
//! pair to check, which is the at-most-one-accounted-actor case expressed as
//! the absence of a conflict rather than as a separate rule. Those two facts
//! are why the six families this replaced — two members, a member against a
//! run, two runs, each in a plain and a commuting form — were one operation.
//!
//! Two recorded accesses commute when neither can observe the other's
//! effect on the bytes it reaches — two non-writing rows never conflict,
//! rows reaching provably distinct storage never conflict, and rows on
//! shared storage must be disjoint fixed extents. Because every traded row
//! pair commutes, the relative order of conflicting accesses is unchanged:
//! none of the traded pairs conflict. The roster itself then follows the
//! new execution order — the rows naming the window's instructions are
//! permuted to match, each instruction's own rows keeping their relative
//! order — so the recorded accesses still appear in the order the program
//! performs them. Rows inside one run never trade order, since the run's
//! members keep their relative order as one body.
//!
//! Register and condition-state hazards, barriers, and settlements keep
//! the run interchange's audit unchanged: no register or unit a member
//! writes may be read or written by a crossed position in either
//! direction — a member's crossed positions are the other run's members
//! and the whole interior — calls, hosted effects, and call-roster entries
//! never sit in the window, and a boundary settlement inside the window's
//! span would observe a different executed prefix. Since no location is
//! written by both a member and a crossed position, the last writer before
//! every later observer — the block's terminator and edge transports
//! included — is the same instruction it was before the interchange, and
//! positions before the window are untouched.
//!
//! No instruction, register, constraint key, call, settlement, or edge
//! record changes: each instruction keeps its own id, kind, operands,
//! provenance, and implicit surface while only its position in the block
//! vector moves, and each roster row keeps its own identity while only
//! its position in the access vector follows the move.
//!
//! Proposal and independent replay share only the admission predicates.
//! Validation consumes the proposed program, requires the touched block's
//! window and the roster's window rows to equal the independently
//! computed interchange, and restores the complete source by content.

mod admission;
mod rewrite;
mod validation;

use std::sync::Arc;

use crate::rewrites::block_edges::member_position;
use optimization_core::OptimizationUnitIdentity;
use selected_instructions::{SelectedInstructionPlan, SelectedInstructionPlanIdentity};
use semantic_vocabulary::FuelScheduleIdentity;

pub(crate) use validation::validate_selected_interchange;

/// An accepted commuting run interchange with its replay receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ValidatedInterchange {
    transformed: Arc<SelectedInstructionPlan>,
    receipt: InterchangeReceipt,
}

validated_transformed_receipt!(ValidatedInterchange, InterchangeReceipt);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterchangeReceipt {
    source_selected: SelectedInstructionPlanIdentity,
    transformed_selected: SelectedInstructionPlanIdentity,
    optimization_unit: OptimizationUnitIdentity,
    fuel_schedule: FuelScheduleIdentity,
}

receipt_selected_identities!(InterchangeReceipt);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InterchangeError {
    SourceMismatch,
    /// A window position can never trade order: a barrier kind, a
    /// call-roster entry, or a memory-capable kind the roster does not
    /// account for — as a run member or as a crossed interior position.
    UnsupportedInstruction,
    /// The named instructions do not bound an admissible commuting
    /// window: absent or misordered in the earlier run's block, a run of
    /// one member — the commuting pair's and the member-against-run
    /// interchange's granularity — coupled by a register or
    /// condition-state hazard with a crossed position, trading a roster
    /// row that does not commute with a crossed position's row, carrying
    /// no rowed-trading pair — the run interchange's own accounting case —
    /// or separated by a boundary settlement inside the window's span.
    UnsupportedPair,
    WorkBudgetExceeded,
    IdentityOverflow,
    ReplayMismatch,
}

impl std::fmt::Display for InterchangeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid commuting run interchange: {self:?}")
    }
}

impl std::error::Error for InterchangeError {}

/// The run positions one admissible commuting interchange derives: the
/// function carrying the touched block, the block, and the four members
/// bounding the earlier run, the interior, and the later run. Admission
/// and validation both prove the same derivation — the validator replays
/// the family's audit over the proposed program rather than trusting the
/// producer's record — so the family module owns it once.
pub(super) struct InterchangeDerivation<'source> {
    pub function: &'source selected_instructions::SelectedFunction,
    pub block_index: usize,
    /// The earlier run's first member index in the block body.
    pub earlier_first_index: usize,
    /// The earlier run's last member index; the earlier run is the
    /// contiguous span `earlier_first_index..=earlier_last_index` of at
    /// least two members.
    pub earlier_last_index: usize,
    /// The later run's first member index, strictly past the earlier run;
    /// the positions between the runs form the interior, possibly empty.
    pub later_first_index: usize,
    /// The later run's last member index; the window the interchange
    /// crosses is `earlier_first_index..=later_last_index`.
    pub later_last_index: usize,
}

/// Re-derive the legality of trading two disjoint runs under proven memory
/// commutation from the source records: locate each named member by
/// identity inside one block's body, then re-derive the window's
/// independence audit — every window position schedulable, every register
/// and condition-state hazard direction between each member and the
/// positions outside its own run, every roster row that newly trades
/// order commuting with every row of the position it crosses, at least
/// one trading pair rowed on both sides — the accounting case that keeps
/// this family disjoint from the plain run interchange — and no boundary
/// settlement inside the window's span — and account the family's
/// measured steps against the budget.
pub(super) fn derive_interchange<'source>(
    source: &'source impl crate::ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: selected_instructions::SelectedInstructionId,
    earlier_last: selected_instructions::SelectedInstructionId,
    later_first: selected_instructions::SelectedInstructionId,
    later_last: selected_instructions::SelectedInstructionId,
    environment: &'source register_environment::ValidatedTargetRegisterEnvironment,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<InterchangeDerivation<'source>, InterchangeError> {
    use crate::rewrites::unexecuted::commuting_accesses as accesses;
    use crate::rewrites::unexecuted::place_storage::structural_place_declarations;
    use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

    let (plan, function) =
        crate::rewrites::plan_function(source, function_index, environment, || {
            InterchangeError::SourceMismatch
        })?;
    let (block_index, earlier_first_index) =
        member_position(function, earlier_first).ok_or(InterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // Each run is the contiguous span its two named members bound in this
    // block, in the named order. Naming one member twice is the run of one,
    // which is the member granularity; an absent or reversed id names no run.
    let earlier_last_index = block
        .instruction_index(earlier_last)
        .filter(|position| *position >= earlier_first_index)
        .ok_or(InterchangeError::UnsupportedPair)?;
    // The later run follows the earlier run disjointly; the positions
    // between them form the interior, which keeps its relative order while
    // the runs trade places.
    let later_first_index = block
        .instruction_index(later_first)
        .filter(|position| *position > earlier_last_index)
        .ok_or(InterchangeError::UnsupportedPair)?;
    let later_last_index = block
        .instruction_index(later_last)
        .filter(|position| *position >= later_first_index)
        .ok_or(InterchangeError::UnsupportedPair)?;
    let window = &block.instructions[earlier_first_index..=later_last_index];
    let earlier_len = earlier_last_index - earlier_first_index + 1;
    let later_len = later_last_index - later_first_index + 1;
    let interior_len = window.len() - earlier_len - later_len;
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the run
    // interchange enforces: no barrier kind, no call contract, and no
    // unaccounted memory reach. What changes here is only the accounting
    // rule — a roster-carrying member may trade order with a crossed
    // position whose own rows commute with it.
    for position in window {
        schedulable(function, position).ok_or(InterchangeError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member trades order with every window position outside its own
    // run — the other run's members and the whole interior — so each
    // direction of every register and condition-state hazard applies
    // against each pair, and every row the member carries must commute
    // with every row the crossed position carries. Members of one run keep
    // their relative order and never face this audit against each other,
    // and interior positions keep their relative order with each other. A
    // window in which no trading pair carries rows on both sides satisfies
    // this by having no pair to check, which is the at-most-one-actor case
    // stated as the absence of a conflict rather than as its own rule.
    for (member_index, member) in window.iter().enumerate() {
        let crossed_range = if member_index < earlier_len {
            // An earlier-run member crosses the interior and the later
            // run.
            earlier_len..window.len()
        } else if member_index < earlier_len + interior_len {
            // An interior position crosses nothing alone: its trades are
            // already audited as the crossed side of both runs' members.
            continue;
        } else {
            // A later-run member crosses the earlier run and the
            // interior.
            0..earlier_len + interior_len
        };
        for crossed_index in crossed_range {
            let crossed = &window[crossed_index];
            if coupled(member, crossed) {
                return Err(InterchangeError::UnsupportedPair);
            }
            for member_row in &rows[member_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(InterchangeError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if interior_settlement(
        function,
        block.id,
        earlier_first_index + 1..=later_last_index,
    ) {
        return Err(InterchangeError::UnsupportedPair);
    }
    // The search scans the function's body and terminator instructions once;
    // the window audit walks every member-against-crossed operand, unit,
    // and roster-row surface plus the function's three rosters.
    let steps = crate::rewrites::rewrite_scan(plan, function)
        .and_then(|total| {
            window
                .iter()
                .enumerate()
                .try_fold(total, |total, (member_index, member)| {
                    let mut crossed_range = if member_index < earlier_len {
                        earlier_len..window.len()
                    } else if member_index < earlier_len + interior_len {
                        return Some(total);
                    } else {
                        0..earlier_len + interior_len
                    };
                    crossed_range.try_fold(total, |total, crossed_index| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(&window[crossed_index]))?
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
        .ok_or(InterchangeError::IdentityOverflow)?;
    crate::rewrites::charge_steps(
        steps,
        budget,
        || InterchangeError::IdentityOverflow,
        || InterchangeError::WorkBudgetExceeded,
    )?;
    Ok(InterchangeDerivation {
        function,
        block_index,
        earlier_first_index,
        earlier_last_index,
        later_first_index,
        later_last_index,
    })
}
