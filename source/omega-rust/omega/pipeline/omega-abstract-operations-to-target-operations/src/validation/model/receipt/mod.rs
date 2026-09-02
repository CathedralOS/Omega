//! Optimizer module role: executable entrance. Translation receipt taxonomy and exact family-to-receipt join.

mod family;
mod function_translation;
mod immediate;
mod parameter;
mod roster;
mod terminal;

pub use function_translation::AbstractToTargetFunctionTranslationReceipt;
pub use immediate::{
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
    StraightLineWrappingIntegerDivideImmediateOperandsTranslationReceipt,
    StraightLineWrappingIntegerMultiplyImmediateTranslationReceipt,
    StraightLineWrappingIntegerShiftLeftImmediateTranslationReceipt,
    StraightLineWrappingIntegerShiftRightImmediateTranslationReceipt,
    StraightLineWrappingIntegerSubtractImmediateTranslationReceipt,
};
pub use parameter::{
    StraightLineBooleanEqualParametersTranslationReceipt,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationReceipt,
    StraightLineExactIntegerAddParametersTranslationReceipt,
    StraightLineExactIntegerDivideParametersTranslationReceipt,
    StraightLineExactIntegerMultiplyParametersTranslationReceipt,
    StraightLineExactIntegerRemainderParametersTranslationReceipt,
    StraightLineExactIntegerShiftLeftParametersTranslationReceipt,
    StraightLineExactIntegerShiftRightParametersTranslationReceipt,
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
    StraightLineSaturatingIntegerDivideParametersTranslationReceipt,
    StraightLineSaturatingIntegerMultiplyParametersTranslationReceipt,
    StraightLineSaturatingIntegerRemainderParametersTranslationReceipt,
    StraightLineSaturatingIntegerSubtractParametersTranslationReceipt,
    StraightLineWrappingIntegerAddParametersTranslationReceipt,
    StraightLineWrappingIntegerDivideParametersTranslationReceipt,
    StraightLineWrappingIntegerMultiplyParametersTranslationReceipt,
    StraightLineWrappingIntegerRemainderParametersTranslationReceipt,
    StraightLineWrappingIntegerShiftLeftParametersTranslationReceipt,
    StraightLineWrappingIntegerShiftRightParametersTranslationReceipt,
    StraightLineWrappingIntegerSubtractParametersTranslationReceipt,
};
pub use roster::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationReceipt,
};
pub use terminal::{
    IeeeFloatFusedMultiplyAddOperandReceipt, IeeeFloatLiteralSequenceMember,
    IntegerIeeeFloatLiteralSequenceMember, IntegerLiteralSequenceMember,
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
