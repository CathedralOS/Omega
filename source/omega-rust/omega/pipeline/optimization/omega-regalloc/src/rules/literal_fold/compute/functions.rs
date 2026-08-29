//! Ordered per-function proposal derivation.

use omega_selected_instructions::{SelectedFunction, SelectedInstructionPlan, SelectedTerminator};

use crate::{
    FunctionLiteralFold, LiteralFoldError, ValidatedRecoveryClassifications,
    ValidatedSelectedAnalysis,
};

use super::actions::derive_action;
use super::constraints::ImmediateRows;
use super::function_rewrite::apply_action;

pub(super) fn derive_function_folds(
    selected: &impl ValidatedSelectedAnalysis,
    recovery: &ValidatedRecoveryClassifications,
    rows: &ImmediateRows<'_>,
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

        let action = recovery_function
            .classification
            .as_ref()
            .map(|classification| derive_action(function_index, source, classification, rows))
            .transpose()?;
        if let Some(action) = action {
            apply_action(
                function_index,
                &mut transformed.functions[function_index],
                action,
                rows,
            )?;
        }
        functions.push(FunctionLiteralFold {
            machine: source.machine,
            action,
        });
    }

    Ok((functions, transformed))
}

fn validate_dense_identifiers(
    function_index: usize,
    function: &SelectedFunction,
) -> Result<(), LiteralFoldError> {
    if function
        .virtual_registers
        .iter()
        .enumerate()
        .any(|(index, register)| usize::try_from(register.id.0) != Ok(index))
    {
        return Err(LiteralFoldError::FunctionMismatch {
            function: function_index,
        });
    }
    let mut ids = function
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .map(|instruction| instruction.id.0)
                .chain(std::iter::once(match &block.terminator {
                    SelectedTerminator::ConditionalBranch { instruction, .. }
                    | SelectedTerminator::Return { instruction, .. } => instruction.id.0,
                }))
        })
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
