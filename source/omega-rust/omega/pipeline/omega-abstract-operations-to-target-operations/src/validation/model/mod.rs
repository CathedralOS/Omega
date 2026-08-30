//! Optimizer module role: stage group.
mod error;
mod receipt;

pub(in crate::validation) use error::StraightLineParameterReconstructionError;
pub use error::{
    AbstractToTargetTranslationFamilyError, AbstractToTargetTranslationValidationError,
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanImmediateTranslationError, StraightLineBooleanNotParameterTranslationError,
    StraightLineBooleanParameterTranslationError,
    StraightLineIntegerBitwiseAndParametersTranslationError,
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerBitwiseOrParametersTranslationError,
    StraightLineIntegerBitwiseXorParametersTranslationError,
    StraightLineIntegerEqualParametersTranslationError,
    StraightLineIntegerExactCastParameterTranslationError,
    StraightLineIntegerImmediateTranslationError,
    StraightLineIntegerLessOrEqualParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationError,
    StraightLineIntegerParameterTranslationError,
    StraightLineIntegerWidenParameterTranslationError, StraightLineScalarCrashTranslationError,
    StraightLineWrappingIntegerAddParametersTranslationError,
};
pub use receipt::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationValidationReceipt,
    StraightLineBooleanEqualParametersTranslationReceipt,
    StraightLineBooleanImmediateTranslationReceipt,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationReceipt,
    StraightLineIntegerBitwiseAndParametersTranslationReceipt,
    StraightLineIntegerBitwiseNotParameterTranslationReceipt,
    StraightLineIntegerBitwiseOrParametersTranslationReceipt,
    StraightLineIntegerBitwiseXorParametersTranslationReceipt,
    StraightLineIntegerEqualParametersTranslationReceipt,
    StraightLineIntegerExactCastParameterTranslationReceipt,
    StraightLineIntegerImmediateTranslationReceipt,
    StraightLineIntegerLessOrEqualParametersTranslationReceipt,
    StraightLineIntegerLessThanParametersTranslationReceipt,
    StraightLineIntegerParameterTranslationReceipt,
    StraightLineIntegerWidenParameterTranslationReceipt, StraightLineScalarCrashTranslationReceipt,
    StraightLineWrappingIntegerAddParametersTranslationReceipt,
};

/// Stable identity of one independently replayed abstract-to-target family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractToTargetTranslationFamily {
    StraightLineIntegerImmediate,
    StraightLineBooleanImmediate,
    StraightLineScalarCrash,
    StraightLineIntegerParameter,
    StraightLineBooleanParameter,
    StraightLineBooleanNotParameter,
    StraightLineIntegerBitwiseNotParameter,
    StraightLineBooleanEqualParameters,
    StraightLineIntegerEqualParameters,
    StraightLineIntegerLessThanParameters,
    StraightLineIntegerLessOrEqualParameters,
    StraightLineIntegerWidenParameter,
    StraightLineIntegerExactCastParameter,
    StraightLineIntegerBitwiseAndParameters,
    StraightLineIntegerBitwiseOrParameters,
    StraightLineIntegerBitwiseXorParameters,
    StraightLineWrappingIntegerAddParameters,
}
