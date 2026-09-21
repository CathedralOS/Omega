//! Shared admission for the in-block commuting run relocation: locate the
//! named `first_member` and `last_member` bounding one contiguous run
//! inside a block's body, locate the named destination instruction outside
//! the run in the same block, and prove the window they bound independent —
//! no register or condition-state hazard between any member and any crossed
//! position, every roster row that newly trades order commuting with every
//! row of the position it crosses, no call, hosted-effect, or terminator
//! barrier anywhere in the window, and no boundary settlement inside its
//! span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::CommutingRunRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::commuting_accesses as accesses;
use crate::rewrites::place_storage::structural_place_declarations;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable, surface};

pub(super) struct Admission {
    pub block_index: usize,
    /// The run's first member index in the block body.
    pub first_index: usize,
    /// The run's last member index; the run is the contiguous span
    /// `first_index..=last_index` of at least two members.
    pub last_index: usize,
    /// The index whose instruction the run displaces: the window the
    /// relocation crosses is `first_index..=destination_index` in either
    /// order, with the destination always outside the run.
    pub destination_index: usize,
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, CommutingRunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(CommutingRunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(CommutingRunRelocationError::SourceMismatch)?;
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
        .ok_or(CommutingRunRelocationError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The run is the contiguous span the two named members bound in this
    // block, in the named order; a repeated id or a last member that does
    // not follow the first names no multi-member run. A single member is
    // the one-instruction relocation the sibling family already proves.
    let last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == last_member)
        .filter(|position| *position > first_index)
        .ok_or(CommutingRunRelocationError::UnsupportedPair)?;
    // The destination sits outside the run in the same block; inside the
    // run it would name a member's own position, and outside the block it
    // names no in-block window.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| !(first_index..=last_index).contains(position))
        .ok_or(CommutingRunRelocationError::UnsupportedPair)?;
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
        schedulable(function, position)
            .ok_or(CommutingRunRelocationError::UnsupportedInstruction)?;
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
                return Err(CommutingRunRelocationError::UnsupportedPair);
            }
            rowed_trade |= !rows[member_index].is_empty() && !rows[crossed_index].is_empty();
            for member_row in &rows[member_index] {
                for crossed_row in &rows[crossed_index] {
                    if !accesses::commutes(member_row, crossed_row, structural_places) {
                        return Err(CommutingRunRelocationError::UnsupportedPair);
                    }
                }
            }
        }
    }
    if !rowed_trade {
        return Err(CommutingRunRelocationError::UnsupportedPair);
    }
    if interior_settlement(function, block.id, first + 1..=last) {
        return Err(CommutingRunRelocationError::UnsupportedPair);
    }
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every member-against-crossed operand, unit,
    // and roster-row surface plus the function's three rosters.
    let run_range = first_index - first..first_index - first + run_len;
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
        .ok_or(CommutingRunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| CommutingRunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(CommutingRunRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        first_index,
        last_index,
        destination_index,
    })
}
