//! Shared admission for in-block run relocation: locate the named `first`
//! and `last` members bounding one contiguous run inside a block's body,
//! locate the named destination instruction outside the run in the same
//! block, and prove the window they bound independent — no register or
//! condition-state hazard between any member and any crossed instruction,
//! no roster-carrying run sharing the window with a second memory-access
//! actor, no call, hosted-effect, or terminator barrier anywhere in the
//! window, and no boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::RunRelocationError;
use crate::ValidatedSelectedAnalysis;
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

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    first_member: SelectedInstructionId,
    last_member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, RunRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RunRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RunRelocationError::SourceMismatch)?;
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
        .ok_or(RunRelocationError::SourceMismatch)?;
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
        .ok_or(RunRelocationError::UnsupportedPair)?;
    // The destination sits outside the run in the same block; inside the
    // run it would name a member's own position, and outside the block it
    // names no in-block window.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| !(first_index..=last_index).contains(position))
        .ok_or(RunRelocationError::UnsupportedPair)?;
    let run = &block.instructions[first_index..=last_index];
    // Every member meets the schedulable bar itself; the run's memory
    // accounting is the union of its members' roster rows.
    let mut run_accounted = false;
    for member in run {
        run_accounted |=
            schedulable(function, member).ok_or(RunRelocationError::UnsupportedInstruction)?;
    }
    let (first, last) = (
        first_index.min(destination_index),
        last_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    for (offset, crossed) in window.iter().enumerate() {
        if (first_index..=last_index).contains(&(first + offset)) {
            continue;
        }
        // Every crossed instruction meets the same schedulable bar as the
        // members: no barrier kind, no call contract, and no unaccounted
        // memory reach. Its roster rows may keep their relative order only
        // while no member records any — a row-carrying run passing a
        // second accounted actor would reorder recorded accesses.
        let crossed_accounted =
            schedulable(function, crossed).ok_or(RunRelocationError::UnsupportedInstruction)?;
        if run_accounted && crossed_accounted {
            return Err(RunRelocationError::UnsupportedPair);
        }
        // Every member trades order with every crossed position, so each
        // direction of every hazard applies against each pair.
        for member in run {
            if coupled(member, crossed) {
                return Err(RunRelocationError::UnsupportedPair);
            }
        }
    }
    if interior_settlement(function, block.id, first + 1..=last) {
        return Err(RunRelocationError::UnsupportedPair);
    }
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every member-against-crossed operand and unit
    // surface plus the function's three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            run.iter().try_fold(total, |total, member| {
                window
                    .iter()
                    .enumerate()
                    .try_fold(total, |total, (offset, crossed)| {
                        if (first_index..=last_index).contains(&(first + offset)) {
                            return Some(total);
                        }
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
        .ok_or(RunRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RunRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RunRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        first_index,
        last_index,
        destination_index,
    })
}
