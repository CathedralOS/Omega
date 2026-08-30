mod error;
mod receipt;

pub(in crate::validation) use error::StraightLineParameterReconstructionError;
pub use error::{
    AbstractToTargetTranslationFamilyError, AbstractToTargetTranslationValidationError,
    StraightLineBooleanEqualParametersTranslationError,
    StraightLineBooleanImmediateTranslationError, StraightLineBooleanNotParameterTranslationError,
    StraightLineBooleanParameterTranslationError,
    StraightLineIntegerBitwiseNotParameterTranslationError,
    StraightLineIntegerEqualParametersTranslationError,
    StraightLineIntegerExactCastParameterTranslationError,
    StraightLineIntegerImmediateTranslationError,
    StraightLineIntegerLessOrEqualParametersTranslationError,
    StraightLineIntegerLessThanParametersTranslationError,
    StraightLineIntegerParameterTranslationError,
    StraightLineIntegerWidenParameterTranslationError, StraightLineScalarCrashTranslationError,
};
pub use receipt::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationValidationReceipt,
    StraightLineBooleanEqualParametersTranslationReceipt,
    StraightLineBooleanImmediateTranslationReceipt,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationReceipt,
    StraightLineIntegerBitwiseNotParameterTranslationReceipt,
    StraightLineIntegerEqualParametersTranslationReceipt,
    StraightLineIntegerExactCastParameterTranslationReceipt,
    StraightLineIntegerImmediateTranslationReceipt,
    StraightLineIntegerLessOrEqualParametersTranslationReceipt,
    StraightLineIntegerLessThanParametersTranslationReceipt,
    StraightLineIntegerParameterTranslationReceipt,
    StraightLineIntegerWidenParameterTranslationReceipt, StraightLineScalarCrashTranslationReceipt,
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
}
