//! Whole-plan identity, evidence-roster, and function-roster replay.

use std::collections::{BTreeMap, BTreeSet};

use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use target::NativeTarget;
use target_operations::{TargetOperation, TargetOperationPlan};

use super::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetTranslationValidationError,
    AbstractToTargetTranslationValidationReceipt, catalog,
};

pub fn validate_abstract_to_target_translation(
    source: &AbstractOperationPlan,
    expected_target: NativeTarget,
    target: &TargetOperationPlan,
) -> Result<AbstractToTargetTranslationValidationReceipt, AbstractToTargetTranslationValidationError>
{
    validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
        source,
        expected_target,
        target,
        &[],
    )
}

pub fn validate_abstract_to_target_translation_with_ieee_float_fma_settlements(
    source: &AbstractOperationPlan,
    expected_target: NativeTarget,
    target: &TargetOperationPlan,
    ieee_float_fma: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<AbstractToTargetTranslationValidationReceipt, AbstractToTargetTranslationValidationError>
{
    validate_plan_identity(source, expected_target, target)?;
    validate_fma_settlement_roster(source, ieee_float_fma)?;
    let structural_call_return = catalog::validate_plan(source, target)?;

    let canonical_structural_types = source
        .structural_types
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>()
        .into_values()
        .cloned()
        .collect::<Vec<_>>();
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
        if matches!(&target_function.operation, TargetOperation::UnitBody(body)
            if body.structural_types != canonical_structural_types)
        {
            return Err(
                AbstractToTargetTranslationValidationError::FunctionStructuralTypeRosterMismatch {
                    machine: source_function.machine,
                },
            );
        }
        let translation = catalog::validate_function(
            source_function,
            expected_target,
            target_function,
            ieee_float_fma,
        )?;
        function_roster.push(AbstractToTargetFunctionRosterReceipt::new(
            source_function.machine,
            source_function.attachment,
            translation,
        ));
    }
    Ok(AbstractToTargetTranslationValidationReceipt::new(
        source.psi,
        expected_target,
        source.entry,
        function_roster,
        structural_call_return,
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

fn validate_fma_settlement_roster(
    source: &AbstractOperationPlan,
    settlements: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<(), AbstractToTargetTranslationValidationError> {
    let expected = source
        .functions
        .iter()
        .flat_map(|function| &function.operations)
        .filter_map(|operation| match operation {
            AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { psi_operation, .. } => {
                Some(*psi_operation)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut supplied = BTreeSet::new();
    for settlement in settlements {
        if !supplied.insert(settlement.terminal_operation) {
            return Err(
                AbstractToTargetTranslationValidationError::DuplicateIeeeFloatFmaSettlement(
                    settlement.terminal_operation,
                ),
            );
        }
        if !expected.contains(&settlement.terminal_operation) {
            return Err(
                AbstractToTargetTranslationValidationError::UnknownIeeeFloatFmaSettlement(
                    settlement.terminal_operation,
                ),
            );
        }
    }
    if let Some(missing) = expected.difference(&supplied).next() {
        return Err(
            AbstractToTargetTranslationValidationError::MissingIeeeFloatFmaSettlement(*missing),
        );
    }
    Ok(())
}
