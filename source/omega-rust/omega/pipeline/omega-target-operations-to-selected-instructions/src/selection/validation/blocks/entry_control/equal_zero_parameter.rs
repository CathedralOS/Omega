//! Independent U64 parameter-equals-zero entry replay.

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
    let LegalizedCondition::U64EqualZeroParameterV1 {
        operation,
        fuel,
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
            operations: vec![zero.constant_operation, *operation],
            values: vec![
                parameter.source_value,
                zero.source_value,
                source.condition_source,
            ],
            fuel: zero.fuel.iter().chain(fuel).copied().collect(),
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
    direct_parameter::validate_branch(function_index, source, instruction, keys, catalog)?;
    let (expected_true, expected_false) = successors(source);
    if when_nonzero != &expected_false || when_zero != &expected_true {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    Ok(())
}
