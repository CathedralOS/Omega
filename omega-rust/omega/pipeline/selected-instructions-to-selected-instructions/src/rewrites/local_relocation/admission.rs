//! Shared admission for in-block relocation: locate the named member
//! inside one block's body, locate the named destination instruction at a
//! different position in the same block, and prove the window they bound
//! independent — no register or condition-state hazard between the member
//! and any crossed instruction, no roster-carrying member sharing the
//! window with a second memory-access actor, no call, hosted-effect, or
//! terminator barrier anywhere in the window, and no boundary settlement
//! inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::{SelectedFunction, SelectedInstructionId};

use super::LocalRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::window_hazards::{coupled, interior_settlement, schedulable};

pub(super) struct Admission<'source> {
    pub function: &'source SelectedFunction,
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
) -> Result<Admission<'source>, LocalRelocationError> {
    let plan = source.selected_plan();
    if plan.target != environment.target() {
        return Err(LocalRelocationError::SourceMismatch);
    }
    let function = plan
        .functions
        .get(function_index)
        .ok_or(LocalRelocationError::SourceMismatch)?;
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
        .ok_or(LocalRelocationError::SourceMismatch)?;
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
        .ok_or(LocalRelocationError::UnsupportedPair)?;
    let member_instruction = &block.instructions[member_index];
    let member_accounted = schedulable(function, member_instruction)
        .ok_or(LocalRelocationError::UnsupportedInstruction)?;
    let (first, last) = (
        member_index.min(destination_index),
        member_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
    for (offset, crossed) in window.iter().enumerate() {
        if first + offset == member_index {
            continue;
        }
        // Every crossed instruction meets the same schedulable bar as the
        // member: no barrier kind, no call contract, and no unaccounted
        // memory reach. Its roster rows may keep their relative order only
        // while the member records none — a row-carrying member passing a
        // second accounted actor would reorder recorded accesses.
        let crossed_accounted =
            schedulable(function, crossed).ok_or(LocalRelocationError::UnsupportedInstruction)?;
        if member_accounted && crossed_accounted {
            return Err(LocalRelocationError::UnsupportedPair);
        }
        // The member trades order with every crossed position, so each
        // direction of every hazard applies against each.
        if coupled(member_instruction, crossed) {
            return Err(LocalRelocationError::UnsupportedPair);
        }
    }
    if interior_settlement(function, block.id, first + 1..=last) {
        return Err(LocalRelocationError::UnsupportedPair);
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
        .ok_or(LocalRelocationError::IdentityOverflow)?;
    if u64::try_from(steps).map_err(|_| LocalRelocationError::IdentityOverflow)?
        > budget.validation_steps()
    {
        return Err(LocalRelocationError::WorkBudgetExceeded);
    }
    Ok(Admission {
        function,
        block_index,
        member_index,
        destination_index,
    })
}
