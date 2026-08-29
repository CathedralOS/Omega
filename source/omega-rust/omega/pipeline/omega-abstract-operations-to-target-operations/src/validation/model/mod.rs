mod error;
mod receipt;

pub use error::{
    AbstractToTargetTranslationFamilyError, AbstractToTargetTranslationValidationError,
    StraightLineBooleanImmediateTranslationError, StraightLineIntegerImmediateTranslationError,
    StraightLineIntegerParameterTranslationError, StraightLineScalarCrashTranslationError,
};
pub use receipt::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetFunctionTranslationReceipt, AbstractToTargetTranslationValidationReceipt,
    StraightLineBooleanImmediateTranslationReceipt, StraightLineIntegerImmediateTranslationReceipt,
    StraightLineIntegerParameterTranslationReceipt, StraightLineScalarCrashTranslationReceipt,
};

/// Stable identity of one independently replayed abstract-to-target family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AbstractToTargetTranslationFamily {
    StraightLineIntegerImmediate,
    StraightLineBooleanImmediate,
    StraightLineScalarCrash,
    StraightLineIntegerParameter,
}
