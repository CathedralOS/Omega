//! Independent abstract-to-target translation validation entrance.
//!
//! This module owns whole-plan root/roster custody, then descends into exact
//! semantic-family validators. Its receipt explicitly lists covered function
//! families and does not claim coverage for absent rows.

mod model;
pub(crate) mod straight_line_integer_immediate;

use omega_abstract_operations::AbstractOperationPlan;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

pub use model::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetTranslationValidationError,
    AbstractToTargetTranslationValidationReceipt, StraightLineIntegerImmediateTranslationError,
    StraightLineIntegerImmediateTranslationReceipt,
};

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

    let mut function_roster = Vec::with_capacity(source.functions.len());
    let mut straight_line_integer_immediates = Vec::new();
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
        function_roster.push(AbstractToTargetFunctionRosterReceipt::new(
            source_function.machine,
            source_function.attachment,
        ));
        if straight_line_integer_immediate::is_candidate(source_function) {
            straight_line_integer_immediates.push(
                straight_line_integer_immediate::validate(source_function, target_function)
                    .map_err(|error| {
                        AbstractToTargetTranslationValidationError::StraightLineIntegerImmediate {
                            machine: source_function.machine,
                            error,
                        }
                    })?,
            );
        }
    }

    Ok(AbstractToTargetTranslationValidationReceipt::new(
        source.psi,
        expected_target,
        source.entry,
        function_roster,
        straight_line_integer_immediates,
    ))
}
