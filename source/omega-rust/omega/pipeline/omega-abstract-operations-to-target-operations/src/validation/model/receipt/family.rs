//! Exact receipt-to-family identity projection.

mod arithmetic;
mod shift;

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
            Self::StraightLineUnitReturn(_) => {
                AbstractToTargetTranslationFamily::StraightLineUnitReturn
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
            receipt @ (Self::StraightLineWrappingIntegerShiftLeftParameters(_)
            | Self::StraightLineWrappingIntegerShiftRightParameters(_)
            | Self::StraightLineExactIntegerShiftLeftParameters(_)
            | Self::StraightLineExactIntegerShiftRightParameters(_)) => shift::family(receipt),
            receipt @ (Self::StraightLineExactIntegerAddParameters(_)
            | Self::StraightLineExactIntegerSubtractParameters(_)
            | Self::StraightLineExactIntegerMultiplyParameters(_)
            | Self::StraightLineExactIntegerDivideParameters(_)
            | Self::StraightLineExactIntegerRemainderParameters(_)
            | Self::StraightLineWrappingIntegerDivideParameters(_)
            | Self::StraightLineWrappingIntegerRemainderParameters(_)
            | Self::StraightLineSaturatingIntegerDivideParameters(_)
            | Self::StraightLineSaturatingIntegerRemainderParameters(_)
            | Self::StraightLineSaturatingIntegerAddParameters(_)
            | Self::StraightLineWrappingIntegerAddParameters(_)
            | Self::StraightLineSaturatingIntegerSubtractParameters(_)
            | Self::StraightLineWrappingIntegerSubtractParameters(_)
            | Self::StraightLineWrappingIntegerMultiplyParameters(_)
            | Self::StraightLineSaturatingIntegerMultiplyParameters(_)) => {
                arithmetic::family(receipt)
            }
        }
    }
}
