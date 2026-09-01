//! Optimizer module role: executable entrance. Translation error taxonomy and exact family-to-error join.

mod family;
mod immediate;
mod parameter;
mod terminal;
mod validation;

pub use family::AbstractToTargetTranslationFamilyError;
pub use immediate::{
    StraightLineBooleanEqualImmediateTranslationError,
    StraightLineBooleanImmediateTranslationError, StraightLineBooleanNotImmediateTranslationError,
    StraightLineIntegerBitwiseAndImmediateTranslationError,
    StraightLineIntegerBitwiseNotImmediateTranslationError,
    StraightLineIntegerBitwiseOrImmediateTranslationError,
    StraightLineIntegerEqualImmediateTranslationError,
    StraightLineIntegerLessThanImmediateTranslationError,
    StraightLineIntegerLessOrEqualImmediateTranslationError,
    StraightLineIntegerExactCastImmediateOperandTranslationError,
    StraightLineIntegerImmediateTranslationError,
    StraightLineIntegerWidenImmediateTranslationError,
    StraightLineWrappingIntegerAddImmediateTranslationError,
    StraightLineWrappingIntegerMultiplyImmediateTranslationError,
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
pub use terminal::{
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
pub use validation::AbstractToTargetTranslationValidationError;
