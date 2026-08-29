mod error;
mod receipt;

pub(in crate::validation) use error::StraightLineParameterReconstructionError;
pub use error::{
    AbstractToTargetTranslationFamilyError, AbstractToTargetTranslationValidationError,
    StraightLineBooleanImmediateTranslationError, StraightLineBooleanNotParameterTranslationError,
    StraightLineBooleanParameterTranslationError, StraightLineIntegerImmediateTranslationError,
    StraightLineIntegerParameterTranslationError, StraightLineScalarCrashTranslationError,
};
pub use receipt::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationValidationReceipt,
    StraightLineBooleanImmediateTranslationReceipt,
    StraightLineBooleanNotParameterTranslationReceipt,
    StraightLineBooleanParameterTranslationReceipt, StraightLineIntegerImmediateTranslationReceipt,
    StraightLineIntegerParameterTranslationReceipt, StraightLineScalarCrashTranslationReceipt,
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
}
