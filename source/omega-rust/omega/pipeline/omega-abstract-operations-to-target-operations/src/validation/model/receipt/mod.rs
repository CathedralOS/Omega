//! Translation receipt taxonomy and exact family-to-receipt join.

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
    StraightLineBooleanParameterTranslationReceipt, StraightLineIntegerParameterTranslationReceipt,
};
pub use roster::{
    AbstractToTargetFunctionRosterReceipt, AbstractToTargetFunctionTranslationDisposition,
    AbstractToTargetTranslationValidationReceipt,
};
pub use terminal::StraightLineScalarCrashTranslationReceipt;

use super::AbstractToTargetTranslationFamily;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbstractToTargetFunctionTranslationReceipt {
    StraightLineIntegerImmediate(StraightLineIntegerImmediateTranslationReceipt),
    StraightLineBooleanImmediate(StraightLineBooleanImmediateTranslationReceipt),
    StraightLineScalarCrash(StraightLineScalarCrashTranslationReceipt),
    StraightLineIntegerParameter(StraightLineIntegerParameterTranslationReceipt),
    StraightLineBooleanParameter(StraightLineBooleanParameterTranslationReceipt),
    StraightLineBooleanNotParameter(StraightLineBooleanNotParameterTranslationReceipt),
    StraightLineBooleanEqualParameters(StraightLineBooleanEqualParametersTranslationReceipt),
}

impl AbstractToTargetFunctionTranslationReceipt {
    pub const fn family(&self) -> AbstractToTargetTranslationFamily {
        match self {
            Self::StraightLineIntegerImmediate(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerImmediate
            }
            Self::StraightLineBooleanImmediate(_) => {
                AbstractToTargetTranslationFamily::StraightLineBooleanImmediate
            }
            Self::StraightLineScalarCrash(_) => {
                AbstractToTargetTranslationFamily::StraightLineScalarCrash
            }
            Self::StraightLineIntegerParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerParameter
            }
            Self::StraightLineBooleanParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineBooleanParameter
            }
            Self::StraightLineBooleanNotParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineBooleanNotParameter
            }
            Self::StraightLineBooleanEqualParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineBooleanEqualParameters
            }
        }
    }
}
