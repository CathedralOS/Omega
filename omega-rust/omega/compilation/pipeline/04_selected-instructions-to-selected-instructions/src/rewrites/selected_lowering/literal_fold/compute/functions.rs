//! Ordered per-function proposal derivation.

use selected_instructions::{SelectedFunction, SelectedInstructionPlan};

use crate::{
    FunctionLiteralFold, LiteralFoldError, ValidatedRecoveryClassifications,
    ValidatedSelectedAnalysis,
};

use super::actions::derive_actions;
use super::constraints::AdmittedPairs;
use super::function_rewrite::apply_actions;
use crate::rewrites::block_edges::{block_instruction_ids, virtual_register_ids_are_dense};

pub(super) fn derive_function_folds(
    selected: &impl ValidatedSelectedAnalysis,
    recovery: &ValidatedRecoveryClassifications,
    rows: &AdmittedPairs<'_>,
) -> Result<(Vec<FunctionLiteralFold>, SelectedInstructionPlan), LiteralFoldError> {
    let source_plan = selected.selected_plan();
    let mut transformed = source_plan.clone();
    let mut functions = Vec::with_capacity(source_plan.functions.len());

    for (function_index, source) in source_plan.functions.iter().enumerate() {
        let recovery_function = recovery.plan().functions.get(function_index).ok_or(
            LiteralFoldError::FunctionMismatch {
                function: function_index,
            },
        )?;
        if source.machine != recovery_function.machine {
            return Err(LiteralFoldError::FunctionMismatch {
                function: function_index,
            });
        }
        validate_dense_identifiers(function_index, source)?;

        // Nomination scans the function's own instruction stream rather
        // than consulting the classification: every `MaterializeI64` whose
        // record carries the producer custody and whose register's sole
        // other occurrence is one `Use` operand later in the same block is
        // a candidate, whichever pressure-recovery role — if any — the
        // register played. A record the enabled pairs cannot serve
        // declines so the stream stands untouched; the root binding above
        // still ties this fold to the exact recovery artifact it ran
        // alongside.
        let actions = derive_actions(source, rows);
        if !actions.is_empty() {
            apply_actions(
                function_index,
                &mut transformed.functions[function_index],
                &actions,
                rows,
            )?;
        }
        functions.push(FunctionLiteralFold {
            machine: source.machine,
            actions,
        });
    }

    Ok((functions, transformed))
}

fn validate_dense_identifiers(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), LiteralFoldError> {
    if !virtual_register_ids_are_dense(function) {
        return Err(LiteralFoldError::FunctionMismatch {
            function: function_index,
        });
    }
    let mut ids = function
        .blocks
        .iter()
        .flat_map(block_instruction_ids)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    let count = u32::try_from(ids.len()).map_err(|_| LiteralFoldError::WorkOverflow)?;
    if ids != (0..count).collect::<Vec<_>>() {
        return Err(LiteralFoldError::FunctionMismatch {
            function: function_index,
        });
    }
    Ok(())
}
