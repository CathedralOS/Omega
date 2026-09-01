//! Optimizer module role: executable entrance. Translation receipt taxonomy and exact family-to-receipt join.

mod family;
mod function_translation;
mod immediate;
mod parameter;
mod roster;
mod terminal;

pub use function_translation::AbstractToTargetFunctionTranslationReceipt;
pub use immediate::{
    StraightLineBooleanImmediateTranslationReceipt, StraightLineIntegerImmediateTranslationReceipt,
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
    IntegerLiteralSequenceMember, StraightLineByteSequenceLiteralUnitReturnTranslationReceipt,
    StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIeeeFloatLiteralUnitReturnTranslationReceipt,
    StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIntegerLiteralUnitReturnTranslationReceipt,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationReceipt,
    StraightLinePortWriteUnitReturnTranslationReceipt, StraightLineScalarCrashTranslationReceipt,
    StraightLineTrivialAffineLocalUnitReturnTranslationReceipt,
    StraightLineUnitCallReturnTranslationReceipt, StraightLineUnitReturnTranslationReceipt,
};
