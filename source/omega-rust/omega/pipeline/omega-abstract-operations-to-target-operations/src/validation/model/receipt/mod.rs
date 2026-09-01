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
