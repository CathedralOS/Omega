//! Closed tagged error carrier for one independently replayed function family.

use super::immediate::{
    StraightLineBooleanImmediateTranslationError, StraightLineIntegerImmediateTranslationError,
};
use super::parameter::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanNotParameterTranslationError, StraightLineBooleanParameterTranslationError,
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineExactIntegerDivideParametersTranslationError,
    StraightLineExactIntegerMultiplyParametersTranslationError,
    StraightLineExactIntegerRemainderParametersTranslationError,
    StraightLineExactIntegerSubtractParametersTranslationError,
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
use super::terminal::StraightLineScalarCrashTranslationError;

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
    StraightLineExactIntegerSubtractParameters(
        StraightLineExactIntegerSubtractParametersTranslationError,
    ),
    StraightLineExactIntegerMultiplyParameters(
        StraightLineExactIntegerMultiplyParametersTranslationError,
    ),
    StraightLineExactIntegerDivideParameters(
        StraightLineExactIntegerDivideParametersTranslationError,
    ),
    StraightLineExactIntegerRemainderParameters(
        StraightLineExactIntegerRemainderParametersTranslationError,
    ),
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
