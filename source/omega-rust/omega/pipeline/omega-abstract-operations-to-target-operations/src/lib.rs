#![forbid(unsafe_code)]

//! Abstract-operation lowering into target-specific operation plans.
//!
//! Enter `lowering/mod.rs` for the validated settlement-to-function lowering
//! join, then descend by result family and semantic responsibility.

mod lowering;
mod model;
mod validation;

pub use lowering::{
    lower_ranked_to_target_operations, lower_to_target_operations,
    lower_to_target_operations_with_provider_executions,
    lower_to_target_operations_with_provider_executions_and_installation,
};
pub use model::{AdmittedBoundarySettlement, LoweringError};
pub use validation::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamily,
    AbstractToTargetTranslationFamilyError, AbstractToTargetTranslationValidationError,
    AbstractToTargetTranslationValidationReceipt, StraightLineBooleanImmediateTranslationError,
    StraightLineBooleanImmediateTranslationReceipt,
    StraightLineBooleanNotParameterTranslationError,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationError, StraightLineBooleanParameterTranslationReceipt,
    StraightLineIntegerImmediateTranslationError, StraightLineIntegerImmediateTranslationReceipt,
    StraightLineIntegerParameterTranslationError, StraightLineIntegerParameterTranslationReceipt,
    StraightLineScalarCrashTranslationError, StraightLineScalarCrashTranslationReceipt,
    validate_abstract_to_target_translation,
};

#[cfg(test)]
mod tests;
