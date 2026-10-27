//! Inspection of one conditional branch form before a relaxation sweep.

use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::SelectedBlockId;

use machine_code::{ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout};

use super::super::{X86BranchRelaxationAttemptOutcome, error::OptimizedX86BranchRelaxationError};
use super::lane;
use super::work::checked_delta;

pub(super) fn inspect_production_branch(
    function: &ResolvedSelectedFunctionLayout,
    block_index: usize,
    instruction_index: usize,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(X86BranchRelaxationAttemptOutcome, Option<i64>), OptimizedX86BranchRelaxationError> {
    let row = &function.blocks[block_index].instructions[instruction_index];
    let branch = row
        .branch
        .as_deref()
        .and_then(machine_code::ResolvedBranchEvidence::as_conditional)
        .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
            row.instruction,
        ))?;
    let near = lane::near_width(row.bytes.len()).ok_or(
        OptimizedX86BranchRelaxationError::MalformedBranch(row.instruction),
    )?;
    lane::validate(
        branch.predicate,
        near,
        physical,
        row.alternative,
        branch.byte_displacement,
        &row.bytes,
    )?;
    if !near {
        return Ok((X86BranchRelaxationAttemptOutcome::AlreadyShort, None));
    }
    let displacement = prospective_short_displacement(function, row, branch.when_taken_block)?;
    if i8::try_from(displacement).is_ok() {
        Ok((
            X86BranchRelaxationAttemptOutcome::SelectedForRelaxation,
            Some(displacement),
        ))
    } else {
        Ok((
            X86BranchRelaxationAttemptOutcome::NearDisplacementOutsideI8,
            None,
        ))
    }
}

fn prospective_short_displacement(
    function: &ResolvedSelectedFunctionLayout,
    row: &ResolvedSelectedFormRow,
    target: SelectedBlockId,
) -> Result<i64, OptimizedX86BranchRelaxationError> {
    let target_offset = function
        .blocks
        .iter()
        .find(|block| block.block == target)
        .map(|block| block.offset)
        .ok_or(OptimizedX86BranchRelaxationError::MissingTargetBlock(
            target,
        ))?;
    let shifted_target = if target_offset > row.offset {
        target_offset
            .checked_sub(4)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?
    } else {
        target_offset
    };
    checked_delta(
        shifted_target,
        row.offset
            .checked_add(2)
            .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?,
    )
}
