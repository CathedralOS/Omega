//! Shared admission for the in-block run interchange: locate the four named
//! members bounding two disjoint runs inside one block's body — the earlier
//! run's first and last and the later run's first and last — and prove the
//! window they bound independent: no register or condition-state hazard
//! between any member and any instruction outside its own run inside the
//! window, no roster-carrying run trading order with a second
//! memory-access actor, no call, hosted-effect, or terminator barrier
//! anywhere in the window, and no boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId};

use super::RunInterchangeError;
use crate::ValidatedSelectedAnalysis;
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
) -> Result<Admission<'source>, RunInterchangeError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(RunInterchangeError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(RunInterchangeError::SourceMismatch)?;
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
        .ok_or(RunInterchangeError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // Each run is the contiguous span its two named members bound in this
    // block, in the named order; a run of one member is the pair
    // interchange's granularity and a repeated or absent id names no run.
    let earlier_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == earlier_last)
        .filter(|position| *position > earlier_first_index)
        .ok_or(RunInterchangeError::UnsupportedPair)?;
    // The later run follows the earlier run disjointly; the positions
    // between them form the interior, which keeps its relative order while
    // the runs trade places.
    let later_first_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later_first)
        .filter(|position| *position > earlier_last_index)
        .ok_or(RunInterchangeError::UnsupportedPair)?;
    let later_last_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later_last)
        .filter(|position| *position > later_first_index)
        .ok_or(RunInterchangeError::UnsupportedPair)?;
    let earlier_run = &block.instructions[earlier_first_index..=earlier_last_index];
    let interior = &block.instructions[earlier_last_index + 1..later_first_index];
    let later_run = &block.instructions[later_first_index..=later_last_index];
    // Every member meets the schedulable bar itself: no barrier kind, no
    // call contract, and no unaccounted memory reach. A run's memory
    // accounting is the union of its members' roster rows — roster-carrying
    // members of one run keep their relative order, so a run may carry
    // several rows, but a row-carrying run may only cross row-less
    // positions: a second accounted actor in the other run or the interior
    // would reorder recorded accesses.
    let mut earlier_accounted = false;
    for member in earlier_run {
        earlier_accounted |=
            schedulable(function, member).ok_or(RunInterchangeError::UnsupportedInstruction)?;
    }
    let mut later_accounted = false;
    for member in later_run {
        later_accounted |=
            schedulable(function, member).ok_or(RunInterchangeError::UnsupportedInstruction)?;
    }
    if earlier_accounted && later_accounted {
        return Err(RunInterchangeError::UnsupportedPair);
    }
    let member_accounted = earlier_accounted || later_accounted;
    for crossed in interior {
        // Every interior position meets the same schedulable bar as the
        // members, since every member trades order with it. Its roster
        // rows may stay only while no member of either run carries any —
        // the interior's recorded accesses then keep their relative order
        // while two memory-inert runs trade places around them.
        let interior_accounted =
            schedulable(function, crossed).ok_or(RunInterchangeError::UnsupportedInstruction)?;
        if interior_accounted && member_accounted {
            return Err(RunInterchangeError::UnsupportedPair);
        }
    }
    // Every member trades order with every window position outside its own
    // run — the other run's members and the whole interior — so each
    // direction of every hazard applies against each pair. Members of one
    // run keep their relative order and never face this audit against each
    // other.
    for member in earlier_run {
        for crossed in interior.iter().chain(later_run.iter()) {
            if coupled(member, crossed) {
                return Err(RunInterchangeError::UnsupportedPair);
            }
        }
    }
    for member in later_run {
        for crossed in interior.iter().chain(earlier_run.iter()) {
            if coupled(member, crossed) {
                return Err(RunInterchangeError::UnsupportedPair);
            }
        }
    }
    if interior_settlement(
        function,
        block.id,
        earlier_first_index + 1..=later_last_index,
    ) {
        return Err(RunInterchangeError::UnsupportedPair);
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
            earlier_run.iter().try_fold(total, |total, member| {
                interior
                    .iter()
                    .chain(later_run.iter())
                    .try_fold(total, |total, crossed| {
                        total
                            .checked_add(surface(member))?
                            .checked_add(surface(crossed))
                    })
            })
        })
        .and_then(|total| {
            later_run.iter().try_fold(total, |total, member| {
                interior
                    .iter()
                    .chain(earlier_run.iter())
                    .try_fold(total, |total, crossed| {
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
        .ok_or(RunInterchangeError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| RunInterchangeError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(RunInterchangeError::WorkBudgetExceeded);
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
