//! Independent I64 less-or-equal replay through reversed signed less-than.

use super::direct_parameter::mismatch;
use super::*;
use crate::selection::validation::blocks::instruction_projection;

pub(super) fn validate(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let LegalizedCondition::I64LessOrEqualParametersV1 {
        operation,
        fuel,
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
        &[VirtualRegisterId(1), VirtualRegisterId(0)],
        &SelectedInstructionProvenance {
            operations: vec![*operation],
            values: vec![
                left.source_value,
                right.source_value,
                source.condition_source,
            ],
            fuel: fuel.clone(),
            ..Default::default()
        },
        catalog,
    )?;
    let SelectedTerminator::ConditionalBranchI64LessThan {
        instruction,
        when_less,
        when_not_less,
    } = &entry.terminator
    else {
        return mismatch(function_index);
    };
    instruction_projection::validate(
        function_index,
        instruction,
        SelectedInstructionId(1),
        SelectedInstructionKind::ConditionalBranchI64LessThan,
        keys.conditional_branch,
        &[],
        &SelectedInstructionProvenance {
            values: vec![source.condition_source],
            ..Default::default()
        },
        catalog,
    )?;
    let (expected_true, expected_false) = successors(source);
    if when_less != &expected_false || when_not_less != &expected_true {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    Ok(())
}
