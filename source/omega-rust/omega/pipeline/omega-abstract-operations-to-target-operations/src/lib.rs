#![forbid(unsafe_code)]

//! Optimizer module role: crate map. Abstract-operation lowering into target-specific operation plans.
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
pub use model::{AdmittedBoundaryExecution, AdmittedBoundarySettlement, LoweringError};
pub use validation::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationFamily,
    AbstractToTargetTranslationFamilyError, AbstractToTargetTranslationValidationError,
    AbstractToTargetTranslationValidationReceipt,
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanEqualParametersTranslationReceipt,
    StraightLineBooleanImmediateTranslationError, StraightLineBooleanImmediateTranslationReceipt,
    StraightLineBooleanNotParameterTranslationError,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationError, StraightLineBooleanParameterTranslationReceipt,
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineExactIntegerAddParametersTranslationReceipt,
    StraightLineIntegerBitwiseAndParametersTranslationError,
    StraightLineIntegerBitwiseAndParametersTranslationReceipt,
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerBitwiseNotParameterTranslationReceipt,
    StraightLineIntegerBitwiseOrParametersTranslationError,
    StraightLineIntegerBitwiseOrParametersTranslationReceipt,
    StraightLineIntegerBitwiseXorParametersTranslationError,
    StraightLineIntegerBitwiseXorParametersTranslationReceipt,
    StraightLineIntegerEqualParametersTranslationError,
    StraightLineIntegerEqualParametersTranslationReceipt,
    StraightLineIntegerExactCastParameterTranslationError,
    StraightLineIntegerExactCastParameterTranslationReceipt,
    StraightLineIntegerImmediateTranslationError, StraightLineIntegerImmediateTranslationReceipt,
    StraightLineIntegerLessOrEqualParametersTranslationError,
    StraightLineIntegerLessOrEqualParametersTranslationReceipt,
    StraightLineIntegerLessThanParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationReceipt,
    StraightLineIntegerParameterTranslationError, StraightLineIntegerParameterTranslationReceipt,
    StraightLineIntegerWidenParameterTranslationError,
    StraightLineIntegerWidenParameterTranslationReceipt,
    StraightLineSaturatingIntegerAddParametersTranslationError,
    StraightLineSaturatingIntegerAddParametersTranslationReceipt,
    StraightLineScalarCrashTranslationError, StraightLineScalarCrashTranslationReceipt,
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationReceipt,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
    StraightLineWrappingIntegerMultiplyParametersTranslationReceipt,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationReceipt,
    validate_abstract_to_target_translation,
};

#[cfg(test)]
mod tests;
