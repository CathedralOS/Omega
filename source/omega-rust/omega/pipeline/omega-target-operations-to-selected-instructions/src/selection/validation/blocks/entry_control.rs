use crate::selection::shared::*;

use super::instruction_projection;

pub(super) fn validate(
    function_index: usize,
    source: &SourceFunction,
    function: &SelectedFunction,
    keys: SelectedConstraintKeys,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let entry = &function.blocks[0];
    if entry.instructions.len() != 1 {
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
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
        return Err(SelectedInstructionError::BlockProjectionMismatch {
            function: function_index,
            block: 0,
        });
    };
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
    )?;
    let expected_true = SelectedSuccessor {
        psi_edge: source.branch_true_edge,
        block: SelectedBlockId(1),
        source_target: source.true_block,
        bindings: source.branch_true_bindings.clone(),
        fuel: source.branch_true_fuel.clone(),
    };
    let expected_false = SelectedSuccessor {
        psi_edge: source.branch_false_edge,
        block: SelectedBlockId(2),
        source_target: source.false_block,
        bindings: source.branch_false_bindings.clone(),
        fuel: source.branch_false_fuel.clone(),
    };
    if when_nonzero != &expected_true || when_zero != &expected_false {
        return Err(SelectedInstructionError::SuccessorProjectionMismatch {
            function: function_index,
            block: 0,
        });
    }
    Ok(())
}
