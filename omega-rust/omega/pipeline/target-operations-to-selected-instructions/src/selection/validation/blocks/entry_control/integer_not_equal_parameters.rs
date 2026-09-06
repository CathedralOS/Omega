//! Independent ordered U64 inequality entry replay.

use super::direct_parameter::mismatch;
use super::*;
use crate::selection::validation::blocks::instruction_projection;

pub(super) fn validate(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: &SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let LegalizedCondition::IntegerNotEqualParametersV1 {
        equality_operation,
        equality_result,
        equality_fuel,
        boolean_not_operation,
        boolean_not_result,
        boolean_not_fuel,
        left,
        right,
        ..
    } = &source.condition
    else {
        return mismatch(function_index);
    };
    let entry = &function.blocks[0];
    if entry.instructions.len() != 1 {
        return mismatch(function_index);
    }
    instruction_projection::validate(
        function_index,
        &entry.instructions[0],
        SelectedInstructionId(0),
        SelectedInstructionKind::CompareI64,
        keys.compare_i64,
        &[VirtualRegisterId(0), VirtualRegisterId(1)],
        &SelectedInstructionProvenance {
            operations: vec![*equality_operation],
            values: vec![left.source_value, right.source_value, *equality_result],
            fuel: equality_fuel.clone(),
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
    instruction_projection::validate(
        function_index,
        instruction,
        SelectedInstructionId(1),
        SelectedInstructionKind::ConditionalBranchNonZero,
        keys.conditional_branch,
        &[],
        &SelectedInstructionProvenance {
            operations: vec![*boolean_not_operation],
            values: vec![*equality_result, *boolean_not_result],
            fuel: boolean_not_fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    let (expected_true, expected_false) = successors(source);
    if when_nonzero != &expected_true || when_zero != &expected_false {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    Ok(())
}
