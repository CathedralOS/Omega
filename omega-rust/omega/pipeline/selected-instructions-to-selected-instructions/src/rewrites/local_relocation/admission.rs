//! Shared admission for in-block relocation: locate the named member
//! inside one block's body, locate the named destination instruction at a
//! different position in the same block, and hand the window they bound to
//! the shared derivation and run-relocation audit — every crossed position
//! schedulable, a roster-carrying member meeting no second accounted actor,
//! every hazard direction against each crossed position, and no boundary
//! settlement inside the window's span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::LocalRelocationError;
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::block_edges::{CrossingDirection, crossed_window};
use crate::rewrites::window_hazards::{RunRelocationRejection, admit_run_relocation};

pub(super) struct Admission {
    pub block_index: usize,
    /// The member's index in the block body.
    pub member_index: usize,
    /// The index whose instruction the member displaces: the window the
    /// relocation crosses is `member_index..=destination_index` in either
    /// order, a single adjacent step at distance one.
    pub destination_index: usize,
}

/// Map the shared audit's rejection onto this module's public error: an
/// unschedulable member or crossed position is `UnsupportedInstruction`;
/// every other refusal — unreachable destination, hazard coupling, memory
/// ordering, transport, or settlement — is `UnsupportedPair`. The
/// unreachable and edge kinds cannot arise on an in-block move, whose
/// crossing is reachable by construction and crosses no edge.
fn reject(rejection: RunRelocationRejection) -> LocalRelocationError {
    match rejection {
        RunRelocationRejection::Unschedulable => LocalRelocationError::UnsupportedInstruction,
        _ => LocalRelocationError::UnsupportedPair,
    }
}

pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    member: SelectedInstructionId,
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, LocalRelocationError> {
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
    // names no move, and the destination names a body position — a
    // terminator-carried instruction is another family's landing.
    let destination_index = block
        .instructions
        .iter()
        .position(|instruction| instruction.id == destination)
        .filter(|position| *position != member_index)
        .ok_or(LocalRelocationError::UnsupportedPair)?;
    let member_instruction = &block.instructions[member_index];
    // The single member is a one-instruction run: the shared derivation
    // returns the positions between it and the landing index, the
    // destination included, and the shared audit proves that window
    // independent once. An in-block move crosses no edge, so the path
    // bound is inert.
    let crossing = crossed_window(
        function,
        block_index,
        member_index,
        member_index,
        block_index,
        destination_index,
        CrossingDirection::Forward,
        0,
    )
    .ok_or(LocalRelocationError::WorkBudgetExceeded)?;
    admit_run_relocation(function, &[member_instruction], &crossing).map_err(reject)?;
    // The search scans the plan's body and terminator instructions once;
    // the window audit walks every crossed member's operand and unit lists
    // plus the function's three rosters.
    let (first, last) = (
        member_index.min(destination_index),
        member_index.max(destination_index),
    );
    let window = &block.instructions[first..=last];
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
        block_index,
        member_index,
        destination_index,
    })
}
