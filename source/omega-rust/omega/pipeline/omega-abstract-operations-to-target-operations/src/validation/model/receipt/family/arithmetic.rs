//! Exact arithmetic receipt-to-family identity projection.

use super::super::AbstractToTargetFunctionTranslationReceipt;
use crate::validation::model::AbstractToTargetTranslationFamily;

pub(super) const fn family(
    receipt: &AbstractToTargetFunctionTranslationReceipt,
) -> AbstractToTargetTranslationFamily {
    match receipt {
        AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerAddParameters(_) => {
            AbstractToTargetTranslationFamily::StraightLineExactIntegerAddParameters
        }
        AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerSubtractParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineExactIntegerSubtractParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerMultiplyParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineExactIntegerMultiplyParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerDivideParameters(_) => {
            AbstractToTargetTranslationFamily::StraightLineExactIntegerDivideParameters
        }
        AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerRemainderParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineExactIntegerRemainderParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerDivideParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineWrappingIntegerDivideParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerRemainderParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineWrappingIntegerRemainderParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerDivideParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerDivideParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerRemainderParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerRemainderParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerAddParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerAddParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerAddParameters(_) => {
            AbstractToTargetTranslationFamily::StraightLineWrappingIntegerAddParameters
        }
        AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerSubtractParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerSubtractParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerSubtractParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineWrappingIntegerSubtractParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerMultiplyParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineWrappingIntegerMultiplyParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineSaturatingIntegerMultiplyParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineSaturatingIntegerMultiplyParameters,
        _ => panic!("the receipt family entrance routes only arithmetic variants here"),
    }
}
