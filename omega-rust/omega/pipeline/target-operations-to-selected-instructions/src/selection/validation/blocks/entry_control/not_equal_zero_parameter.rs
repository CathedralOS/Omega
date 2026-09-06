//! Independent U64 parameter-not-equal-zero entry replay.

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
    let LegalizedCondition::U64NotEqualZeroParameterV1 {
        equality_operation,
        equality_result,
        equality_fuel,
        boolean_not_operation,
        boolean_not_result,
        boolean_not_fuel,
        parameter,
        zero,
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
        SelectedInstructionKind::CompareI64Zero,
        keys.compare_i64_zero,
        &[VirtualRegisterId(0)],
        &SelectedInstructionProvenance {
            operations: vec![zero.constant_operation, *equality_operation],
            values: vec![parameter.source_value, zero.source_value, *equality_result],
            fuel: zero.fuel.iter().chain(equality_fuel).copied().collect(),
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
