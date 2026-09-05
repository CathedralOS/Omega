//! Separate production and replay inspection of one conditional branch form.

use isa_x86_64::{
    validate_x86_64_selected_i64_less_than_branch_form,
    validate_x86_64_selected_nonzero_branch_form,
    validate_x86_64_selected_short_nonzero_branch_form,
    validate_x86_64_selected_u64_less_than_branch_form,
};
use register_model::ValidatedPhysicalRegisterModel;
use selected_instructions::SelectedBlockId;

use crate::{
    ResolvedConditionalBranchPredicate, ResolvedSelectedFormRow, ResolvedSelectedFunctionLayout,
};

use super::super::{
    error::OptimizedX86BranchRelaxationError, model::X86BranchRelaxationAttemptOutcome,
};
use super::work::checked_delta;

pub(super) fn inspect_production_branch(
    function: &ResolvedSelectedFunctionLayout,
    block_index: usize,
    instruction_index: usize,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(X86BranchRelaxationAttemptOutcome, Option<i64>), OptimizedX86BranchRelaxationError> {
    let row = &function.blocks[block_index].instructions[instruction_index];
    let branch =
        row.branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                row.instruction,
            ))?;
    let already_short = match (branch.predicate, row.bytes.len()) {
        (ResolvedConditionalBranchPredicate::NonZeroV1, 2) => {
            validate_x86_64_selected_short_nonzero_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            true
        }
        (ResolvedConditionalBranchPredicate::NonZeroV1, 6) => {
            validate_x86_64_selected_nonzero_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            false
        }
        (ResolvedConditionalBranchPredicate::U64LessThanV1, 2 | 6) => {
            validate_x86_64_selected_u64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            row.bytes.len() == 2
        }
        (ResolvedConditionalBranchPredicate::I64LessThanV1, 2 | 6) => {
            validate_x86_64_selected_i64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            row.bytes.len() == 2
        }
        _ => {
            return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                row.instruction,
            ));
        }
    };
    if already_short {
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

pub(super) fn replay_inspect_branch(
    function: &ResolvedSelectedFunctionLayout,
    block_index: usize,
    instruction_index: usize,
    physical: &ValidatedPhysicalRegisterModel,
) -> Result<(X86BranchRelaxationAttemptOutcome, Option<i64>), OptimizedX86BranchRelaxationError> {
    let row = function
        .blocks
        .get(block_index)
        .and_then(|block| block.instructions.get(instruction_index))
        .ok_or(OptimizedX86BranchRelaxationError::OffsetOverflow)?;
    let branch =
        row.branch
            .as_deref()
            .ok_or(OptimizedX86BranchRelaxationError::MalformedBranch(
                row.instruction,
            ))?;
    let already_short = match (branch.predicate, row.bytes.as_slice()) {
        (ResolvedConditionalBranchPredicate::NonZeroV1, [0x75, displacement]) => {
            let decoded = i64::from(*displacement as i8);
            if decoded != branch.byte_displacement {
                return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                    row.instruction,
                ));
            }
            validate_x86_64_selected_short_nonzero_branch_form(
                physical,
                row.alternative,
                decoded,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            true
        }
        (ResolvedConditionalBranchPredicate::NonZeroV1, [0x0f, 0x85, ..])
            if row.bytes.len() == 6 =>
        {
            validate_x86_64_selected_nonzero_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            false
        }
        (ResolvedConditionalBranchPredicate::U64LessThanV1, [0x72, displacement]) => {
            let decoded = i64::from(*displacement as i8);
            if decoded != branch.byte_displacement {
                return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                    row.instruction,
                ));
            }
            validate_x86_64_selected_u64_less_than_branch_form(
                physical,
                row.alternative,
                decoded,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            true
        }
        (ResolvedConditionalBranchPredicate::U64LessThanV1, [0x0f, 0x82, ..])
            if row.bytes.len() == 6 =>
        {
            validate_x86_64_selected_u64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            false
        }
        (ResolvedConditionalBranchPredicate::I64LessThanV1, [0x7c, displacement]) => {
            let decoded = i64::from(*displacement as i8);
            if decoded != branch.byte_displacement {
                return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                    row.instruction,
                ));
            }
            validate_x86_64_selected_i64_less_than_branch_form(
                physical,
                row.alternative,
                decoded,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            true
        }
        (ResolvedConditionalBranchPredicate::I64LessThanV1, [0x0f, 0x8c, ..])
            if row.bytes.len() == 6 =>
        {
            validate_x86_64_selected_i64_less_than_branch_form(
                physical,
                row.alternative,
                branch.byte_displacement,
                &row.bytes,
            )
            .map_err(OptimizedX86BranchRelaxationError::X86_64)?;
            false
        }
        _ => {
            return Err(OptimizedX86BranchRelaxationError::MalformedBranch(
                row.instruction,
            ));
        }
    };
    if already_short {
        return Ok((X86BranchRelaxationAttemptOutcome::AlreadyShort, None));
    }
    let displacement = prospective_short_displacement(function, row, branch.when_taken_block)?;
    if (-128..=127).contains(&displacement) {
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
