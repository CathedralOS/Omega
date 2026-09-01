//! Optimizer module role: executable entrance. Translation error taxonomy and exact family-to-error join.

mod immediate;
mod parameter;
mod terminal;
mod validation;

pub use immediate::{
    StraightLineBooleanImmediateTranslationError, StraightLineIntegerImmediateTranslationError,
};
pub(in crate::validation) use parameter::StraightLineParameterReconstructionError;
pub use parameter::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanNotParameterTranslationError, StraightLineBooleanParameterTranslationError,
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineIntegerBitwiseAndParametersTranslationError,
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerBitwiseOrParametersTranslationError,
    StraightLineIntegerBitwiseXorParametersTranslationError,
    StraightLineIntegerEqualParametersTranslationError,
    StraightLineIntegerExactCastParameterTranslationError,
    StraightLineIntegerLessOrEqualParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationError,
    StraightLineIntegerParameterTranslationError,
    StraightLineIntegerWidenParameterTranslationError,
    StraightLineSaturatingIntegerAddParametersTranslationError,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};
pub use terminal::StraightLineScalarCrashTranslationError;
pub use validation::AbstractToTargetTranslationValidationError;

use super::AbstractToTargetTranslationFamily;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationFamilyError {
    StraightLineIntegerImmediate(StraightLineIntegerImmediateTranslationError),
    StraightLineBooleanImmediate(StraightLineBooleanImmediateTranslationError),
    StraightLineScalarCrash(StraightLineScalarCrashTranslationError),
    StraightLineIntegerParameter(StraightLineIntegerParameterTranslationError),
    StraightLineBooleanParameter(StraightLineBooleanParameterTranslationError),
    StraightLineBooleanNotParameter(StraightLineBooleanNotParameterTranslationError),
    StraightLineIntegerBitwiseNotParameter(StraightLineIntegerBitwiseNotParameterTranslationError),
    StraightLineBooleanEqualParameters(StraightLineBooleanEqualParametersTranslationError),
    StraightLineIntegerEqualParameters(StraightLineIntegerEqualParametersTranslationError),
    StraightLineIntegerLessThanParameters(StraightLineIntegerLessThanParametersTranslationError),
    StraightLineIntegerLessOrEqualParameters(
        StraightLineIntegerLessOrEqualParametersTranslationError,
    ),
    StraightLineIntegerWidenParameter(StraightLineIntegerWidenParameterTranslationError),
    StraightLineIntegerExactCastParameter(StraightLineIntegerExactCastParameterTranslationError),
    StraightLineIntegerBitwiseAndParameters(
        StraightLineIntegerBitwiseAndParametersTranslationError,
    ),
    StraightLineIntegerBitwiseOrParameters(StraightLineIntegerBitwiseOrParametersTranslationError),
    StraightLineIntegerBitwiseXorParameters(
        StraightLineIntegerBitwiseXorParametersTranslationError,
    ),
    StraightLineExactIntegerAddParameters(StraightLineExactIntegerAddParametersTranslationError),
    StraightLineSaturatingIntegerAddParameters(
        StraightLineSaturatingIntegerAddParametersTranslationError,
    ),
    StraightLineWrappingIntegerAddParameters(
        StraightLineWrappingIntegerAddParametersTranslationError,
    ),
    StraightLineSaturatingIntegerSubtractParameters(
        StraightLineSaturatingIntegerSubtractParametersTranslationError,
    ),
    StraightLineWrappingIntegerSubtractParameters(
        StraightLineWrappingIntegerSubtractParametersTranslationError,
    ),
    StraightLineWrappingIntegerMultiplyParameters(
        StraightLineWrappingIntegerMultiplyParametersTranslationError,
    ),
    StraightLineSaturatingIntegerMultiplyParameters(
        StraightLineSaturatingIntegerMultiplyParametersTranslationError,
    ),
}
