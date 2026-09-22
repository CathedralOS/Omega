//! Shared admission for in-block interchange: locate the named earlier
//! instruction inside one block's body, locate the named later instruction
//! after it in the same block, and prove the window they bound independent —
//! no register or condition-state hazard between either member and the
//! instructions between them, at most one roster-carrying memory actor among
//! the crossed positions, no call, hosted-effect, or terminator barrier
//! anywhere in the window, and no boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::LocalScheduleError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable};

pub(super) struct Admission {
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
) -> Result<Admission, LocalScheduleError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LocalScheduleError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LocalScheduleError::SourceMismatch)?;
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
        .ok_or(LocalScheduleError::SourceMismatch)?;
    let block = &function.blocks[block_index];
    // The pair is the two named instructions in the named order inside this
    // block; every instruction between them belongs to the window the
    // interchange crosses.
    let later_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == later)
        .filter(|position| *position > earlier_index)
        .ok_or(LocalScheduleError::UnsupportedPair)?;
    let earlier_instruction = &block.instructions[earlier_index];
    let later_instruction = &block.instructions[later_index];
    let earlier_accounted = schedulable(function, earlier_instruction)
        .ok_or(LocalScheduleError::UnsupportedInstruction)?;
    let later_accounted = schedulable(function, later_instruction)
        .ok_or(LocalScheduleError::UnsupportedInstruction)?;
    // A roster-carrying access may only cross instructions that cannot
    // observe memory: a second accounted actor anywhere in the window would
    // need a place-alias decision this step does not take.
    if earlier_accounted && later_accounted {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    let member_accounted = earlier_accounted || later_accounted;
    let window = &block.instructions[earlier_index..=later_index];
    for interior in &window[1..window.len() - 1] {
        // Every crossed instruction meets the same schedulable bar as the
        // named members: no barrier kind, no call contract, and no
        // unaccounted memory reach. Its roster rows may stay only while
        // neither member carries any — the interior's recorded accesses
        // then keep their position while two memory-inert instructions
        // trade places around them.
        let interior_accounted =
            schedulable(function, interior).ok_or(LocalScheduleError::UnsupportedInstruction)?;
        if interior_accounted && member_accounted {
            return Err(LocalScheduleError::UnsupportedPair);
        }
        // The interior keeps its position but trades order with both
        // members, so each direction of every hazard applies against each.
        if coupled(earlier_instruction, interior) || coupled(later_instruction, interior) {
            return Err(LocalScheduleError::UnsupportedPair);
        }
    }
    if interior_settlement(function, block.id, earlier_index + 1..=later_index) {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    if coupled(earlier_instruction, later_instruction) {
        return Err(LocalScheduleError::UnsupportedPair);
    }
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every crossed member's operand and unit lists
    // plus the function's three rosters.
    let steps = plan
        .functions
        .iter()
        .try_fold(0usize, |total, function| {
            function.blocks.iter().try_fold(total, |total, block| {
                total.checked_add(block.instructions.len())?.checked_add(1)
            })
        })
        .and_then(|total| {
            window.iter().try_fold(total, |total, instruction| {
                total
                    .checked_add(instruction.operands.len())?
                    .checked_add(instruction.implicit_uses.len())?
                    .checked_add(instruction.implicit_defs.len())?
                    .checked_add(instruction.clobbers.len())
            })
        })
        .and_then(|total| {
            total
                .checked_add(function.memory_accesses.len())?
                .checked_add(function.calls.len())?
                .checked_add(function.boundary_settlements.len())
        })
        .ok_or(LocalScheduleError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LocalScheduleError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LocalScheduleError::WorkBudgetExceeded);
    }
    Ok(Admission {
        block_index,
        earlier_index,
        later_index,
    })
}
