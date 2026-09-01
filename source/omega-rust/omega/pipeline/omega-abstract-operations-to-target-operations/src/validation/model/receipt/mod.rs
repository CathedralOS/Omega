//! Optimizer module role: executable entrance. Translation receipt taxonomy and exact family-to-receipt join.

mod family;
mod immediate;
mod parameter;
mod roster;
mod terminal;

pub use immediate::{
    StraightLineBooleanImmediateTranslationReceipt, StraightLineIntegerImmediateTranslationReceipt,
};
pub use parameter::{
    StraightLineBooleanEqualParametersTranslationReceipt,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationReceipt,
    StraightLineExactIntegerAddParametersTranslationReceipt,
    StraightLineExactIntegerMultiplyParametersTranslationReceipt,
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
pub use roster::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationReceipt,
};
pub use terminal::StraightLineScalarCrashTranslationReceipt;

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
