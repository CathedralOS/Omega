//! Closed tagged carrier for one independently replayed function family.

mod arithmetic;
mod shift;

use super::immediate::{
    StraightLineBooleanEqualImmediateTranslationReceipt,
    StraightLineBooleanImmediateTranslationReceipt,
    StraightLineBooleanNotImmediateTranslationReceipt,
    StraightLineIntegerBitwiseAndImmediateTranslationReceipt,
    StraightLineIntegerBitwiseNotImmediateTranslationReceipt,
    StraightLineIntegerBitwiseOrImmediateTranslationReceipt,
    StraightLineIntegerBitwiseXorImmediateTranslationReceipt,
    StraightLineIntegerEqualImmediateTranslationReceipt,
    StraightLineIntegerExactCastImmediateOperandTranslationReceipt,
    StraightLineIntegerImmediateTranslationReceipt,
    StraightLineIntegerLessOrEqualImmediateTranslationReceipt,
    StraightLineIntegerLessThanImmediateTranslationReceipt,
    StraightLineIntegerWidenImmediateTranslationReceipt,
    StraightLineSaturatingIntegerAddImmediateTranslationReceipt,
    StraightLineSaturatingIntegerMultiplyImmediateTranslationReceipt,
    StraightLineSaturatingIntegerSubtractImmediateTranslationReceipt,
    StraightLineWrappingIntegerAddImmediateTranslationReceipt,
    StraightLineWrappingIntegerMultiplyImmediateTranslationReceipt,
    StraightLineWrappingIntegerShiftLeftImmediateTranslationReceipt,
    StraightLineWrappingIntegerShiftRightImmediateTranslationReceipt,
    StraightLineWrappingIntegerSubtractImmediateTranslationReceipt,
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
use super::terminal::{
    StraightLineByteSequenceLiteralUnitReturnTranslationReceipt,
    StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIeeeFloatLiteralUnitReturnTranslationReceipt,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIntegerLiteralUnitReturnTranslationReceipt,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt,
    StraightLinePortWriteUnitReturnTranslationReceipt, StraightLineScalarCrashTranslationReceipt,
    StraightLineTrivialAffineLocalUnitReturnTranslationReceipt,
    StraightLineUnitCallReturnTranslationReceipt, StraightLineUnitReturnTranslationReceipt,
};
use crate::validation::{
    StructuralCallReturnCallerTranslationReceipt, StructuralParameterReturnCalleeTranslationReceipt,
};
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
    StraightLineIntegerWidenImmediate(StraightLineIntegerWidenImmediateTranslationReceipt),
    StraightLineIntegerBitwiseAndImmediate(
        StraightLineIntegerBitwiseAndImmediateTranslationReceipt,
    ),
    StraightLineIntegerBitwiseOrImmediate(StraightLineIntegerBitwiseOrImmediateTranslationReceipt),
    StraightLineIntegerBitwiseXorImmediate(
        StraightLineIntegerBitwiseXorImmediateTranslationReceipt,
    ),
    StraightLineSaturatingIntegerAddImmediate(
        StraightLineSaturatingIntegerAddImmediateTranslationReceipt,
    ),
    StraightLineSaturatingIntegerSubtractImmediate(
        StraightLineSaturatingIntegerSubtractImmediateTranslationReceipt,
    ),
    StraightLineSaturatingIntegerMultiplyImmediate(
        StraightLineSaturatingIntegerMultiplyImmediateTranslationReceipt,
    ),
    StraightLineWrappingIntegerAddImmediate(
        StraightLineWrappingIntegerAddImmediateTranslationReceipt,
    ),
    StraightLineWrappingIntegerSubtractImmediate(
        StraightLineWrappingIntegerSubtractImmediateTranslationReceipt,
    ),
    StraightLineWrappingIntegerMultiplyImmediate(
        StraightLineWrappingIntegerMultiplyImmediateTranslationReceipt,
    ),
    StraightLineWrappingIntegerShiftLeftImmediate(
        StraightLineWrappingIntegerShiftLeftImmediateTranslationReceipt,
    ),
    StraightLineWrappingIntegerShiftRightImmediate(
        StraightLineWrappingIntegerShiftRightImmediateTranslationReceipt,
    ),
    StraightLineIntegerBitwiseNotImmediate(
        StraightLineIntegerBitwiseNotImmediateTranslationReceipt,
    ),
    StraightLineIntegerEqualImmediate(StraightLineIntegerEqualImmediateTranslationReceipt),
    StraightLineIntegerLessThanImmediate(StraightLineIntegerLessThanImmediateTranslationReceipt),
    StraightLineIntegerLessOrEqualImmediate(
        StraightLineIntegerLessOrEqualImmediateTranslationReceipt,
    ),
    StraightLineIntegerExactCastImmediateOperand(
        StraightLineIntegerExactCastImmediateOperandTranslationReceipt,
    ),
    StraightLineBooleanImmediate(StraightLineBooleanImmediateTranslationReceipt),
    StraightLineBooleanNotImmediate(StraightLineBooleanNotImmediateTranslationReceipt),
    StraightLineBooleanEqualImmediate(StraightLineBooleanEqualImmediateTranslationReceipt),
    StraightLineUnitReturn(StraightLineUnitReturnTranslationReceipt),
    StraightLinePortWriteUnitReturn(StraightLinePortWriteUnitReturnTranslationReceipt),
    StraightLineUnitCallReturn(StraightLineUnitCallReturnTranslationReceipt),
    StraightLineByteSequenceLiteralUnitReturn(
        StraightLineByteSequenceLiteralUnitReturnTranslationReceipt,
    ),
    StraightLineIntegerLiteralUnitReturn(StraightLineIntegerLiteralUnitReturnTranslationReceipt),
    StraightLineIntegerLiteralSequenceUnitReturn(
        StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt,
    ),
    StraightLineIeeeFloatLiteralUnitReturn(
        StraightLineIeeeFloatLiteralUnitReturnTranslationReceipt,
    ),
    StraightLineIeeeFloatLiteralSequenceUnitReturn(
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    ),
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturn(
        StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    ),
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn(
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt,
    ),
    StraightLineTrivialAffineLocalUnitReturn(
        StraightLineTrivialAffineLocalUnitReturnTranslationReceipt,
    ),
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
    StructuralCallReturnCaller(StructuralCallReturnCallerTranslationReceipt),
    StructuralParameterReturnCallee(StructuralParameterReturnCalleeTranslationReceipt),
}
