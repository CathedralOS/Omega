//! Optimizer module role: executable entrance. Independent abstract-to-target translation validation entrance.
//!
//! This module owns whole-plan root/roster custody, then descends into exact
//! semantic-family validators. Its receipt explicitly lists covered function
//! families and does not claim coverage for absent rows.

mod catalog;
mod model;
pub(crate) mod straight_line_boolean_immediate;
pub(crate) mod straight_line_integer_immediate;
pub(crate) mod straight_line_parameter;
pub(crate) mod straight_line_port_write_unit_return;
pub(crate) mod straight_line_scalar_crash;
pub(crate) mod straight_line_unit_call_return;
pub(crate) mod straight_line_unit_return;

use std::collections::BTreeMap;

use omega_abstract_operations::AbstractOperationPlan;
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperation, TargetOperationPlan};

pub use model::*;

pub fn validate_abstract_to_target_translation(
    source: &AbstractOperationPlan,
    expected_target: NativeTarget,
    target: &TargetOperationPlan,
) -> Result<AbstractToTargetTranslationValidationReceipt, AbstractToTargetTranslationValidationError>
{
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
        if matches!(
            &target_function.operation,
            TargetOperation::UnitBody(body)
                if body.structural_types != canonical_structural_types
        ) {
            return Err(
                AbstractToTargetTranslationValidationError::FunctionStructuralTypeRosterMismatch {
                    machine: source_function.machine,
                },
            );
        }
        let translation =
            catalog::validate_function(source_function, expected_target, target_function)?;
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
    ))
}
