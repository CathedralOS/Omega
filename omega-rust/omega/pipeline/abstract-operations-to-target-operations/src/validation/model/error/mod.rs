//! Optimizer module role: executable entrance. Translation error taxonomy and exact family-to-error join.

mod family;
mod immediate;
mod parameter;
mod terminal;
mod validation;

pub use self::terminal::{
    StraightLineByteSequenceLiteralUnitReturnTranslationError,
    StraightLineIeeeFloatLiteralSequenceUnitReturnTranslationError,
    StraightLineIeeeFloatLiteralUnitReturnTranslationError,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError,
    StraightLineIntegerLiteralSequenceUnitReturnTranslationError,
    StraightLineIntegerLiteralUnitReturnTranslationError,
    StraightLineNearestIeeeFloatFusedMultiplyAddUnitReturnTranslationError,
    StraightLinePortWriteUnitReturnTranslationError, StraightLineScalarCrashTranslationError,
    StraightLineTrivialAffineLocalUnitReturnTranslationError,
    StraightLineUnitCallReturnTranslationError, StraightLineUnitReturnTranslationError,
};
pub use family::AbstractToTargetTranslationFamilyError;
pub use immediate::{
    StraightLineBooleanEqualImmediateTranslationError,
    StraightLineBooleanImmediateTranslationError, StraightLineBooleanNotImmediateTranslationError,
    StraightLineIntegerBitwiseAndImmediateTranslationError,
    StraightLineIntegerBitwiseNotImmediateTranslationError,
    StraightLineIntegerBitwiseOrImmediateTranslationError,
    StraightLineIntegerBitwiseXorImmediateTranslationError,
    StraightLineIntegerEqualImmediateTranslationError,
    StraightLineIntegerExactCastImmediateOperandTranslationError,
    StraightLineIntegerImmediateTranslationError,
    StraightLineIntegerLessOrEqualImmediateTranslationError,
    StraightLineIntegerLessThanImmediateTranslationError,
    StraightLineIntegerWidenImmediateTranslationError,
    StraightLineSaturatingIntegerAddImmediateTranslationError,
    StraightLineSaturatingIntegerDivideImmediateOperandsTranslationError,
    StraightLineSaturatingIntegerMultiplyImmediateTranslationError,
    StraightLineSaturatingIntegerSubtractImmediateTranslationError,
    StraightLineWrappingIntegerAddImmediateTranslationError,
    StraightLineWrappingIntegerDivideImmediateOperandsTranslationError,
    StraightLineWrappingIntegerMultiplyImmediateTranslationError,
    StraightLineWrappingIntegerRemainderImmediateOperandsTranslationError,
    StraightLineWrappingIntegerShiftLeftImmediateTranslationError,
    StraightLineWrappingIntegerShiftRightImmediateTranslationError,
    StraightLineWrappingIntegerSubtractImmediateTranslationError,
};
pub(in crate::validation) use parameter::StraightLineParameterReconstructionError;
pub use parameter::{
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanNotParameterTranslationError, StraightLineBooleanParameterTranslationError,
    StraightLineExactIntegerAddParametersTranslationError,
    StraightLineExactIntegerDivideParametersTranslationError,
    StraightLineExactIntegerMultiplyParametersTranslationError,
    StraightLineExactIntegerRemainderParametersTranslationError,
    StraightLineExactIntegerShiftLeftParametersTranslationError,
    StraightLineExactIntegerShiftRightParametersTranslationError,
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
    StraightLineSaturatingIntegerDivideParametersTranslationError,
    StraightLineSaturatingIntegerMultiplyParametersTranslationError,
    StraightLineSaturatingIntegerRemainderParametersTranslationError,
    StraightLineSaturatingIntegerSubtractParametersTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationError,
    StraightLineWrappingIntegerDivideParametersTranslationError,
    StraightLineWrappingIntegerMultiplyParametersTranslationError,
    StraightLineWrappingIntegerRemainderParametersTranslationError,
    StraightLineWrappingIntegerShiftLeftParametersTranslationError,
    StraightLineWrappingIntegerShiftRightParametersTranslationError,
    StraightLineWrappingIntegerSubtractParametersTranslationError,
};
pub use validation::AbstractToTargetTranslationValidationError;
