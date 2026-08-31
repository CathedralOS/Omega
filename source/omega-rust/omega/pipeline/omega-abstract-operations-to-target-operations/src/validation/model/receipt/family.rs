//! Exact receipt-to-family identity projection.

use super::AbstractToTargetFunctionTranslationReceipt;
use crate::validation::model::AbstractToTargetTranslationFamily;

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
            Self::StraightLineIntegerBitwiseNotParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseNotParameter
            }
            Self::StraightLineBooleanEqualParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineBooleanEqualParameters
            }
            Self::StraightLineIntegerEqualParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerEqualParameters
            }
            Self::StraightLineIntegerLessThanParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerLessThanParameters
            }
            Self::StraightLineIntegerLessOrEqualParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerLessOrEqualParameters
            }
            Self::StraightLineIntegerWidenParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerWidenParameter
            }
            Self::StraightLineIntegerExactCastParameter(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerExactCastParameter
            }
            Self::StraightLineIntegerBitwiseAndParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseAndParameters
            }
            Self::StraightLineIntegerBitwiseOrParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseOrParameters
            }
            Self::StraightLineIntegerBitwiseXorParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineIntegerBitwiseXorParameters
            }
            Self::StraightLineExactIntegerAddParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineExactIntegerAddParameters
            }
            Self::StraightLineSaturatingIntegerAddParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerAddParameters
            }
            Self::StraightLineWrappingIntegerAddParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineWrappingIntegerAddParameters
            }
            Self::StraightLineSaturatingIntegerSubtractParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerSubtractParameters
            }
            Self::StraightLineWrappingIntegerSubtractParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineWrappingIntegerSubtractParameters
            }
            Self::StraightLineWrappingIntegerMultiplyParameters(_) => {
                AbstractToTargetTranslationFamily::StraightLineWrappingIntegerMultiplyParameters
            }
        }
    }
}
