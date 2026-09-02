//! Independent ordered U64 equality entry replay with inverted branch successors.

use super::direct_parameter::{mismatch, validate_branch};
use super::*;
use crate::selection::validation::blocks::instruction_projection;

pub(super) fn validate(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let LegalizedCondition::IntegerEqualParametersV1 {
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
        &[VirtualRegisterId(0), VirtualRegisterId(1)],
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
    if when_nonzero != &expected_false || when_zero != &expected_true {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    Ok(())
}
