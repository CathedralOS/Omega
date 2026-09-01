//! Integer-shift receipt-to-family identity projection.

use super::super::AbstractToTargetFunctionTranslationReceipt;
use crate::validation::model::AbstractToTargetTranslationFamily;

pub(super) const fn family(
    receipt: &AbstractToTargetFunctionTranslationReceipt,
) -> AbstractToTargetTranslationFamily {
    match receipt {
        AbstractToTargetFunctionTranslationReceipt::StraightLineWrappingIntegerShiftLeftParameters(
            _,
        ) => AbstractToTargetTranslationFamily::StraightLineWrappingIntegerShiftLeftParameters,
        _ => panic!("the receipt family entrance routes only shift variants here"),
    }
}
