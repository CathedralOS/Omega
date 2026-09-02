//! Independent direct-Boolean compare-zero entry replay.

use super::*;
use crate::selection::validation::blocks::instruction_projection;

pub(super) fn validate(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let entry = &function.blocks[0];
    if entry.instructions.len() != 1 {
        return mismatch(function_index);
    }
    instruction_projection::validate(
        function_index,
        &entry.instructions[0],
        SelectedInstructionId(0),
        SelectedInstructionKind::CompareI64Zero,
        keys.compare_i64_zero,
        &[VirtualRegisterId(0)],
        &SelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )?;
    let SelectedTerminator::ConditionalBranch {
        instruction,
        when_nonzero,
        when_zero,
    } = &entry.terminator
    else {
        return mismatch(function_index);
    };
    validate_branch(function_index, source, instruction, keys, catalog)?;
    let (expected_true, expected_false) = successors(source);
    if when_nonzero != &expected_true || when_zero != &expected_false {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    Ok(())
}

pub(super) fn validate_branch(
    function_index: usize,
    source: &SourceFunction,
    instruction: &SelectedInstruction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    instruction_projection::validate(
        function_index,
        instruction,
        SelectedInstructionId(1),
        SelectedInstructionKind::ConditionalBranchNonZero,
        keys.conditional_branch,
        &[],
        &SelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )
}

pub(super) fn mismatch(function: usize) -> Result<(), SelectedInstructionError> {
    Err(SelectedInstructionError::BlockProjectionMismatch { function, block: 0 })
}
