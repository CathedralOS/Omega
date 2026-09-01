//! Closed tagged error carrier for one independently replayed function family.

mod arithmetic;
mod shift;

use super::immediate::{
    StraightLineBooleanImmediateTranslationError, StraightLineIntegerImmediateTranslationError,
};
use super::parameter::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanNotParameterTranslationError, StraightLineBooleanParameterTranslationError,
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
};
use super::terminal::{
    StraightLineByteSequenceLiteralUnitReturnTranslationError,
    StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError,
    StraightLineIeeeFloatLiteralUnitReturnTranslationError,
    StraightLineIntegerLiteralSequenceUnitReturnTranslationError,
    StraightLineIntegerLiteralUnitReturnTranslationError,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError,
    StraightLinePortWriteUnitReturnTranslationError, StraightLineScalarCrashTranslationError,
    StraightLineTrivialAffineLocalUnitReturnTranslationError,
    StraightLineUnitCallReturnTranslationError, StraightLineUnitReturnTranslationError,
};
use arithmetic::{
    ExactAddError, ExactDivideError, ExactMultiplyError, ExactRemainderError, ExactSubtractError,
    SaturatingAddError, SaturatingDivideError, SaturatingMultiplyError, SaturatingRemainderError,
    SaturatingSubtractError, WrappingAddError, WrappingDivideError, WrappingMultiplyError,
    WrappingRemainderError, WrappingSubtractError,
};
use shift::{
    ExactShiftLeftError, ExactShiftRightError, WrappingShiftLeftError, WrappingShiftRightError,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetTranslationFamilyError {
    StraightLineIntegerImmediate(StraightLineIntegerImmediateTranslationError),
    StraightLineBooleanImmediate(StraightLineBooleanImmediateTranslationError),
    StraightLineUnitReturn(StraightLineUnitReturnTranslationError),
    StraightLinePortWriteUnitReturn(StraightLinePortWriteUnitReturnTranslationError),
    StraightLineUnitCallReturn(StraightLineUnitCallReturnTranslationError),
    StraightLineByteSequenceLiteralUnitReturn(
        StraightLineByteSequenceLiteralUnitReturnTranslationError,
    ),
    StraightLineIntegerLiteralUnitReturn(StraightLineIntegerLiteralUnitReturnTranslationError),
    StraightLineIntegerLiteralSequenceUnitReturn(
        StraightLineIntegerLiteralSequenceUnitReturnTranslationError,
    ),
    StraightLineIeeeFloatLiteralUnitReturn(StraightLineIeeeFloatLiteralUnitReturnTranslationError),
    StraightLineIeeeFloatLiteralSequenceUnitReturn(
        StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError,
    ),
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturn(
        StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError,
    ),
    StraightLineTrivialAffineLocalUnitReturn(
        StraightLineTrivialAffineLocalUnitReturnTranslationError,
    ),
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
    StraightLineWrappingIntegerShiftLeftParameters(WrappingShiftLeftError),
    StraightLineWrappingIntegerShiftRightParameters(WrappingShiftRightError),
    StraightLineExactIntegerShiftLeftParameters(ExactShiftLeftError),
    StraightLineExactIntegerShiftRightParameters(ExactShiftRightError),
    StraightLineExactIntegerAddParameters(ExactAddError),
    StraightLineExactIntegerSubtractParameters(ExactSubtractError),
    StraightLineExactIntegerMultiplyParameters(ExactMultiplyError),
    StraightLineExactIntegerDivideParameters(ExactDivideError),
    StraightLineExactIntegerRemainderParameters(ExactRemainderError),
    StraightLineWrappingIntegerDivideParameters(WrappingDivideError),
    StraightLineWrappingIntegerRemainderParameters(WrappingRemainderError),
    StraightLineSaturatingIntegerDivideParameters(SaturatingDivideError),
    StraightLineSaturatingIntegerRemainderParameters(SaturatingRemainderError),
    StraightLineSaturatingIntegerAddParameters(SaturatingAddError),
    StraightLineWrappingIntegerAddParameters(WrappingAddError),
    StraightLineSaturatingIntegerSubtractParameters(SaturatingSubtractError),
    StraightLineWrappingIntegerSubtractParameters(WrappingSubtractError),
    StraightLineWrappingIntegerMultiplyParameters(WrappingMultiplyError),
    StraightLineSaturatingIntegerMultiplyParameters(SaturatingMultiplyError),
}
