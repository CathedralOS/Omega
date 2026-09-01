//! Closed tagged carrier for one independently replayed function family.

use super::immediate::{
    StraightLineBooleanImmediateTranslationReceipt, StraightLineIntegerImmediateTranslationReceipt,
};
use super::parameter::{
    StraightLineBooleanEqualParametersTranslationReceipt,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationReceipt,
    StraightLineExactIntegerAddParametersTranslationReceipt,
    StraightLineExactIntegerDivideParametersTranslationReceipt,
    StraightLineExactIntegerMultiplyParametersTranslationReceipt,
    StraightLineExactIntegerRemainderParametersTranslationReceipt,
    StraightLineExactIntegerSubtractParametersTranslationReceipt,
    StraightLineIntegerBitwiseAndParametersTranslationReceipt,
    StraightLineIntegerBitwiseNotParameterTranslationReceipt,
    StraightLineIntegerBitwiseOrParametersTranslationReceipt,
    StraightLineIntegerBitwiseXorParametersTranslationReceipt,
    StraightLineIntegerEqualParametersTranslationReceipt,
    StraightLineIntegerExactCastParameterTranslationReceipt,
    StraightLineIntegerLessOrEqualParametersTranslationReceipt,
    StraightLineIntegerLessThanParametersTranslationReceipt,
    StraightLineIntegerParameterTranslationReceipt,
    StraightLineIntegerWidenParameterTranslationReceipt,
    StraightLineSaturatingIntegerAddParametersTranslationReceipt,
    StraightLineSaturatingIntegerMultiplyParametersTranslationReceipt,
    StraightLineSaturatingIntegerSubtractParametersTranslationReceipt,
    StraightLineWrappingIntegerAddParametersTranslationReceipt,
    StraightLineWrappingIntegerMultiplyParametersTranslationReceipt,
    StraightLineWrappingIntegerSubtractParametersTranslationReceipt,
};
use super::terminal::StraightLineScalarCrashTranslationReceipt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetFunctionTranslationReceipt {
    StraightLineIntegerImmediate(StraightLineIntegerImmediateTranslationReceipt),
    StraightLineBooleanImmediate(StraightLineBooleanImmediateTranslationReceipt),
    StraightLineScalarCrash(StraightLineScalarCrashTranslationReceipt),
    StraightLineIntegerParameter(StraightLineIntegerParameterTranslationReceipt),
    StraightLineBooleanParameter(StraightLineBooleanParameterTranslationReceipt),
    StraightLineBooleanNotParameter(StraightLineBooleanNotParameterTranslationReceipt),
    StraightLineIntegerBitwiseNotParameter(
        StraightLineIntegerBitwiseNotParameterTranslationReceipt,
    ),
    StraightLineBooleanEqualParameters(StraightLineBooleanEqualParametersTranslationReceipt),
    StraightLineIntegerEqualParameters(StraightLineIntegerEqualParametersTranslationReceipt),
    StraightLineIntegerLessThanParameters(StraightLineIntegerLessThanParametersTranslationReceipt),
    StraightLineIntegerLessOrEqualParameters(
        StraightLineIntegerLessOrEqualParametersTranslationReceipt,
    ),
    StraightLineIntegerWidenParameter(StraightLineIntegerWidenParameterTranslationReceipt),
    StraightLineIntegerExactCastParameter(StraightLineIntegerExactCastParameterTranslationReceipt),
    StraightLineIntegerBitwiseAndParameters(
        StraightLineIntegerBitwiseAndParametersTranslationReceipt,
    ),
    StraightLineIntegerBitwiseOrParameters(
        StraightLineIntegerBitwiseOrParametersTranslationReceipt,
    ),
    StraightLineIntegerBitwiseXorParameters(
        StraightLineIntegerBitwiseXorParametersTranslationReceipt,
    ),
    StraightLineExactIntegerAddParameters(StraightLineExactIntegerAddParametersTranslationReceipt),
    StraightLineExactIntegerSubtractParameters(
        StraightLineExactIntegerSubtractParametersTranslationReceipt,
    ),
    StraightLineExactIntegerMultiplyParameters(
        StraightLineExactIntegerMultiplyParametersTranslationReceipt,
    ),
    StraightLineExactIntegerDivideParameters(
        StraightLineExactIntegerDivideParametersTranslationReceipt,
    ),
    StraightLineExactIntegerRemainderParameters(
        StraightLineExactIntegerRemainderParametersTranslationReceipt,
    ),
    StraightLineSaturatingIntegerAddParameters(
        StraightLineSaturatingIntegerAddParametersTranslationReceipt,
    ),
    StraightLineWrappingIntegerAddParameters(
        StraightLineWrappingIntegerAddParametersTranslationReceipt,
    ),
    StraightLineSaturatingIntegerSubtractParameters(
        StraightLineSaturatingIntegerSubtractParametersTranslationReceipt,
    ),
    StraightLineWrappingIntegerSubtractParameters(
        StraightLineWrappingIntegerSubtractParametersTranslationReceipt,
    ),
    StraightLineWrappingIntegerMultiplyParameters(
        StraightLineWrappingIntegerMultiplyParametersTranslationReceipt,
    ),
    StraightLineSaturatingIntegerMultiplyParameters(
        StraightLineSaturatingIntegerMultiplyParametersTranslationReceipt,
    ),
}
