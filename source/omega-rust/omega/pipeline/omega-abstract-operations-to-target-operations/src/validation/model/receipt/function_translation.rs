//! Closed tagged carrier for one independently replayed function family.

mod arithmetic;
mod shift;

use super::immediate::{
    StraightLineBooleanImmediateTranslationReceipt, StraightLineIntegerImmediateTranslationReceipt,
};
use super::parameter::{
    StraightLineBooleanEqualParametersTranslationReceipt,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationReceipt,
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
};
use super::terminal::StraightLineScalarCrashTranslationReceipt;
use arithmetic::{
    ExactAddReceipt, ExactDivideReceipt, ExactMultiplyReceipt, ExactRemainderReceipt,
    ExactSubtractReceipt, SaturatingAddReceipt, SaturatingDivideReceipt, SaturatingMultiplyReceipt,
    SaturatingRemainderReceipt, SaturatingSubtractReceipt, WrappingAddReceipt,
    WrappingDivideReceipt, WrappingMultiplyReceipt, WrappingRemainderReceipt,
    WrappingSubtractReceipt,
};
use shift::{
    ExactShiftLeftReceipt, ExactShiftRightReceipt, WrappingShiftLeftReceipt,
    WrappingShiftRightReceipt,
};

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
    StraightLineWrappingIntegerShiftLeftParameters(WrappingShiftLeftReceipt),
    StraightLineWrappingIntegerShiftRightParameters(WrappingShiftRightReceipt),
    StraightLineExactIntegerShiftLeftParameters(ExactShiftLeftReceipt),
    StraightLineExactIntegerShiftRightParameters(ExactShiftRightReceipt),
    StraightLineExactIntegerAddParameters(ExactAddReceipt),
    StraightLineExactIntegerSubtractParameters(ExactSubtractReceipt),
    StraightLineExactIntegerMultiplyParameters(ExactMultiplyReceipt),
    StraightLineExactIntegerDivideParameters(ExactDivideReceipt),
    StraightLineExactIntegerRemainderParameters(ExactRemainderReceipt),
    StraightLineWrappingIntegerDivideParameters(WrappingDivideReceipt),
    StraightLineWrappingIntegerRemainderParameters(WrappingRemainderReceipt),
    StraightLineSaturatingIntegerDivideParameters(SaturatingDivideReceipt),
    StraightLineSaturatingIntegerRemainderParameters(SaturatingRemainderReceipt),
    StraightLineSaturatingIntegerAddParameters(SaturatingAddReceipt),
    StraightLineWrappingIntegerAddParameters(WrappingAddReceipt),
    StraightLineSaturatingIntegerSubtractParameters(SaturatingSubtractReceipt),
    StraightLineWrappingIntegerSubtractParameters(WrappingSubtractReceipt),
    StraightLineWrappingIntegerMultiplyParameters(WrappingMultiplyReceipt),
    StraightLineSaturatingIntegerMultiplyParameters(SaturatingMultiplyReceipt),
}
