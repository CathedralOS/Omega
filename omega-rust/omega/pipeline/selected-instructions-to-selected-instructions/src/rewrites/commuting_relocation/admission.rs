//! Shared admission for the in-block commuting relocation: locate the
//! named member inside one block's body, locate the named destination
//! instruction at a different position in the same block, and prove the
//! window they bound independent — no register or condition-state hazard
//! between the member and any crossed position, every roster row that
//! newly trades order commuting with every row of the position it crosses,
//! no call, hosted-effect, or terminator barrier anywhere in the window,
//! and no boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::CommutingRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;
use crate::rewrites::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

pub(super) struct Admission {
    pub block_index: usize,
    /// The member's index in the block body.
    pub member_index: usize,
    /// The index whose instruction the member displaces: the window the
    /// relocation crosses is `member_index..=destination_index` in either
    /// order, a single adjacent step at distance one.
    pub destination_index: usize,
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, CommutingRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingRelocationError::SourceMismatch)?;
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
        .ok_or(CommutingRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The window is the span between the member and the destination in
    // this block; every other instruction inside it belongs to the crossed
    // run the member passes. The destination at the member's own position
    // names no move.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| *position != member_index)
        .ok_or(CommutingRelocationError::UnsupportedPair)?;
    let (first, last) = (
        member_index.min(destination_index),
        member_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    let member_window_index = member_index - first;
    let member_instruction = &window[member_window_index];
    let structural_places = structural_place_declarations(function);
    // Every window position meets the same schedulable bar the local
    // relocation enforces: no barrier kind, no call contract, and no
    // unaccounted memory reach. What changes here is only the accounting
    // rule — the row-carrying member may trade order with a crossed
    // position whose own rows commute with it.
    for position in window {
        schedulable(function, position).ok_or(CommutingRelocationError::UnsupportedInstruction)?;
    }
    let rows = accesses::window_rows(function, window);
    // The member trades order with every crossed position — the
    // destination and the whole interior — so each direction of every
    // register and condition-state hazard applies against each, and every
    // row the member carries must commute with every row the crossed
    // position carries. Crossed positions keep their relative order with
    // each other and are never audited against one another. A window in
    // which no trading pair is rowed on both sides is the local
    // relocation's own accounting case and stays with it.
    let mut rowed_trade = false;
    for (crossed_index, crossed) in window.iter().enumerate() {
        if crossed_index == member_window_index {
            continue;
        }
        if coupled(member_instruction, crossed) {
            return Err(CommutingRelocationError::UnsupportedPair);
        }
        rowed_trade |= !rows[member_window_index].is_empty() && !rows[crossed_index].is_empty();
        for member_row in &rows[member_window_index] {
            for crossed_row in &rows[crossed_index] {
                if !accesses::commutes(member_row, crossed_row, structural_places) {
                    return Err(CommutingRelocationError::UnsupportedPair);
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
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks the member's surface against each crossed
    // position's operand, unit, and roster-row surface plus the function's
    // three rosters.
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
                .try_fold(total, |total, (crossed_index, crossed)| {
                    if crossed_index == member_window_index {
                        return Some(total);
                    }
                    total
                        .checked_add(surface(member_instruction))?
                        .checked_add(surface(crossed))?
                        .checked_add(
                            rows[member_window_index]
                                .len()
                                .checked_mul(rows[crossed_index].len())?,
                        )
                })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(CommutingRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CommutingRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CommutingRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        member_index,
        destination_index,
    })
}
