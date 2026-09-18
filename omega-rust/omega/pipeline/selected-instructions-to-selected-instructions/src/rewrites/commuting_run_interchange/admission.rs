//! Shared admission for the in-block commuting run interchange: locate the
//! four named members bounding two disjoint runs inside one block's body —
//! the earlier run's first and last and the later run's first and last —
//! and prove the window they bound independent: no register or
//! condition-state hazard between any member and any instruction outside
//! its own run inside the window, every roster row that newly trades order
//! commuting with every row of the position it crosses, no call,
//! hosted-effect, or terminator barrier anywhere in the window, and no
//! boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId};

use super::CommutingRunInterchangeError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;
use crate::rewrites::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
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

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, CommutingRunInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingRunInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingRunInterchangeError::SourceMismatch)?;
    let (block_index, earlier_first_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == earlier_first)
                .map(|earlier_first_index| (block_index, earlier_first_index))
        })
        .ok_or(CommutingRunInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // Each run is the contiguous span its two named members bound in this
    // block, in the named order; a run of one member is the pair
    // interchange's granularity and a repeated or absent id names no run.
    let earlier_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == earlier_last)
        .filter(|position| *position > earlier_first_index)
        .ok_or(CommutingRunInterchangeError::UnsupportedPair)?;
    // The later run follows the earlier run disjointly; the positions
    // between them form the interior, which keeps its relative order while
    // the runs trade places.
    let later_first_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later_first)
        .filter(|position| *position > earlier_last_index)
        .ok_or(CommutingRunInterchangeError::UnsupportedPair)?;
    let later_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later_last)
        .filter(|position| *position > later_first_index)
        .ok_or(CommutingRunInterchangeError::UnsupportedPair)?;
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
        schedulable(function, position)
            .ok_or(CommutingRunInterchangeError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member trades order with every window position outside its own
    // run — the other run's members and the whole interior — so each
    // direction of every register and condition-state hazard applies
    // against each pair, and every row the member carries must commute
    // with every row the crossed position carries. Members of one run keep
    // their relative order and never face this audit against each other,
    // and interior positions keep their relative order with each other. A
    // window in which no trading pair is rowed on both sides is the run
    // interchange's own accounting case and stays with it.
    let mut rowed_trade = false;
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
                return Err(CommutingRunInterchangeError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingRunInterchangeError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingRunInterchangeError::UnsupportedPair);
    }
    if interior_settlement(
        function,
        block.id,
        earlier_first_index + 1..=later_last_index,
    ) {
        return Err(CommutingRunInterchangeError::UnsupportedPair);
    }
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every member-against-crossed operand, unit,
    // and roster-row surface plus the function's three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
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
        .ok_or(CommutingRunInterchangeError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CommutingRunInterchangeError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CommutingRunInterchangeError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        earlier_first_index,
        earlier_last_index,
        later_first_index,
        later_last_index,
    })
}
