//! Optimizer module role: executable entrance. Exact constant saturating-integer-subtract replay join.

mod grammar;
mod replay;

use abstract_operations::AbstractFunction;
use target_operations::TargetFunction;

use super::{
    StraightLineSaturatingIntegerSubtractImmediateTranslationError,
    StraightLineSaturatingIntegerSubtractImmediateTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    grammar::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<
    StraightLineSaturatingIntegerSubtractImmediateTranslationReceipt,
    StraightLineSaturatingIntegerSubtractImmediateTranslationError,
> {
    let source = grammar::reconstruct(source)?;
    replay::validate(source, target)
}
