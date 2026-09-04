//! Integer-shift receipt-to-family identity projection.

use super::super::AbstractToTargetFunctionTranslationReceipt;
use crate::validation::model::AbstractToTargetTranslationFamily;

pub(super) const fn family(
    receipt: &AbstractToTargetFunctionTranslationReceipt,
) -> AbstractToTargetTranslationFamily {
    match receipt {
        AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerShiftLeftParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineExactIntegerShiftLeftParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineExactIntegerShiftRightParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineExactIntegerShiftRightParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftLeftParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftLeftParameters,
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftRightParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftRightParameters,
        _ => panic!("the receipt family entrance routes only shift variants here"),
    }
}
