//! Whole-plan identity, evidence-roster, and function-roster replay.

use std::collections::BTreeMap;

use crate::target_operations::{TargetOperationPlan, TargetUnitOperation};
use target::NativeTarget;
use terminal_psi_to_abstract_operations::abstract_operations::{
    AbstractOperation, AbstractOperationPlan,
};

use super::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetTranslationValidationError,
    AbstractToTargetTranslationValidationReceipt,
};

pub fn validate_abstract_to_target_translation(
    source: &AbstractOperationPlan,
    expected_target: NativeTarget,
    target: &TargetOperationPlan,
) -> Result<AbstractToTargetTranslationValidationReceipt, AbstractToTargetTranslationValidationError>
{
    validate_plan_identity(source, expected_target, target)?;
    validate_native_callback_roster(target)?;
    // Independent replay must reject raw FMA even when target rows omit it.
    if let Some(operation) = source
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. } => {
                Some(*psi_operation)
            }
            _ => None,
        })
        .min()
    {
        return Err(
            AbstractToTargetTranslationValidationError::UnsupportedIeeeFloatFmaRealization(
                operation,
            ),
        );
    }

    let canonical_structural_types = if source
        .structural_types
        .windows(2)
        .all(|pair| pair[0].id < pair[1].id)
    {
        source.structural_types.clone()
    } else {
        source
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<BTreeMap<_, _>>()
            .into_values()
            .cloned()
            .collect::<terminal_psi_to_abstract_operations::abstract_operations::StructuralTypeCatalog>()
    };
    let mut function_roster = Vec::with_capacity(source.functions.len());
    for (position, (source_function, target_function)) in
        source.functions.iter().zip(&target.functions).enumerate()
    {
        if source_function.machine != target_function.machine {
            return Err(
                AbstractToTargetTranslationValidationError::FunctionMachineMismatch { position },
            );
        }
        if source_function.attachment != target_function.attachment {
            return Err(
                AbstractToTargetTranslationValidationError::FunctionAttachmentMismatch {
                    machine: source_function.machine,
                },
            );
        }
        if target_function.graph.structural_types != canonical_structural_types {
            return Err(
                AbstractToTargetTranslationValidationError::FunctionStructuralTypeRosterMismatch {
                    machine: source_function.machine,
                },
            );
        }
        super::structural_signatures::validate(
            source_function,
            target_function,
            expected_target,
            &source.structural_types,
        )
        .ok_or(
            AbstractToTargetTranslationValidationError::StructuralSignatureMismatch {
                machine: source_function.machine,
            },
        )?;
        super::structural_call_arguments::validate(
            source_function,
            &source.functions,
            target_function,
            &target.functions,
            &source.structural_types,
            &source.boundary_machines,
            expected_target,
            &target.native_callback_arguments,
        )
        .map_err(|operation| {
            AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                machine: source_function.machine,
                operation,
            }
        })?;
        super::structural_argument_sources::validate(source_function, target_function).map_err(
            |operation| {
                AbstractToTargetTranslationValidationError::StructuralCallArgumentMismatch {
                    machine: source_function.machine,
                    operation,
                }
            },
        )?;
        function_roster.push(AbstractToTargetFunctionRosterReceipt::new(
            source_function.machine,
            source_function.attachment,
        ));
    }
    Ok(AbstractToTargetTranslationValidationReceipt::new(
        source.psi,
        expected_target,
        source.entry,
        function_roster,
    ))
}

fn validate_plan_identity(
    source: &AbstractOperationPlan,
    expected_target: NativeTarget,
    target: &TargetOperationPlan,
) -> Result<(), AbstractToTargetTranslationValidationError> {
    if source.psi != target.psi {
        return Err(AbstractToTargetTranslationValidationError::PsiMismatch);
    }
    if expected_target != target.target {
        return Err(AbstractToTargetTranslationValidationError::TargetMismatch);
    }
    if source.entry != target.entry {
        return Err(AbstractToTargetTranslationValidationError::EntryMismatch);
    }
    if source.functions.len() != target.functions.len() {
        return Err(AbstractToTargetTranslationValidationError::FunctionCountMismatch);
    }
    Ok(())
}

/// Every retained native-callback argument must name exactly one normalized
/// foreign call row that agrees it consumed that admission: the roster is a
/// plan-level custody carrier, so a dangling, duplicated, or substituted row
/// fails closed here rather than inside one function's replay.
fn validate_native_callback_roster(
    target: &TargetOperationPlan,
) -> Result<(), AbstractToTargetTranslationValidationError> {
    for callback in &target.native_callback_arguments {
        let mut matching = target
            .functions
            .iter()
            .flat_map(|function| function.graph.blocks.iter())
            .flat_map(|block| block.operations.iter())
            .filter_map(|operation| match operation {
                TargetUnitOperation::NormalizedForeignCall {
                    psi_operation,
                    binding,
                    ..
                } => Some((*psi_operation, binding)),
                _ => None,
            })
            .filter(|(psi_operation, binding)| {
                *psi_operation == callback.terminal_operation
                    && binding.boundary_entry_plan == callback.registrar_boundary_entry_plan
            });
        if matching.next().is_none() || matching.next().is_some() {
            return Err(
                AbstractToTargetTranslationValidationError::NativeCallbackRosterMismatch(
                    callback.terminal_operation,
                ),
            );
        }
    }
    Ok(())
}
