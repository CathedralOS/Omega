//! Shared admission for the in-block commuting pair interchange: locate
//! the named earlier instruction inside one block's body, locate the named
//! later instruction after it in the same block, and prove the window they
//! bound independent — no register or condition-state hazard between either
//! member and a crossed position, every roster row that newly trades order
//! commuting with the position it crosses, no call, hosted-effect, or
//! terminator barrier anywhere in the window, and no boundary settlement
//! inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId};

use super::CommutingInterchangeError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;
use crate::rewrites::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
    pub block_index: usize,
    /// The earlier member's index in the block body.
    pub earlier_index: usize,
    /// The later member's index: the window the interchange crosses is
    /// `earlier_index..=later_index`, a single position when the pair is
    /// adjacent.
    pub later_index: usize,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier: SelectedInstructionId,
    later: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission<'source>, CommutingInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingInterchangeError::SourceMismatch)?;
    let (block_index, earlier_index) = function
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| instruction.id == earlier)
                .map(|earlier_index| (block_index, earlier_index))
        })
        .ok_or(CommutingInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The pair is the two named instructions in the named order inside this
    // block; every instruction between them belongs to the window the
    // interchange crosses.
    let later_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later)
        .filter(|position| *position > earlier_index)
        .ok_or(CommutingInterchangeError::UnsupportedPair)?;
    let window = &block.instructions[earlier_index..=later_index];
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the pair
    // interchange enforces: no barrier kind, no call contract, and no
    // unaccounted memory reach. What changes here is only the accounting
    // rule — a roster-carrying position may trade order with a crossed
    // position whose own rows commute with it.
    for position in window {
        schedulable(function, position).ok_or(CommutingInterchangeError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // Every member trades order with every crossed position — the other
    // member and the whole interior — so each direction of every register
    // and condition-state hazard applies against each pair, and every row
    // the member carries must commute with every row the crossed position
    // carries. Interior positions keep their relative order with each
    // other and are never audited against one another. A window in which
    // no trading pair is rowed on both sides is the pair interchange's own
    // accounting case and stays with it.
    let mut rowed_trade = false;
    for (member_index, member) in [0usize, window.len() - 1]
        .into_iter()
        .map(|index| (index, &window[index]))
    {
        for (crossed_index, crossed) in window
            .iter()
            .enumerate()
            .filter(|(crossed_index, _)| *crossed_index != member_index)
        {
            if coupled(member, crossed) {
                return Err(CommutingInterchangeError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingInterchangeError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingInterchangeError::UnsupportedPair);
    }
    if interior_settlement(function, block.id, earlier_index + 1..=later_index) {
        return Err(CommutingInterchangeError::UnsupportedPair);
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
            [0usize, window.len() - 1]
                .into_iter()
                .try_fold(total, |total, member_index| {
                    window
                        .iter()
                        .enumerate()
                        .try_fold(total, |total, (crossed_index, crossed)| {
                            if crossed_index == member_index {
                                return Some(total);
                            }
                            total
                                .checked_add(surface(&window[member_index]))?
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
        .ok_or(CommutingInterchangeError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CommutingInterchangeError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CommutingInterchangeError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        earlier_index,
        later_index,
    })
}
