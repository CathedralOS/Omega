//! Optimizer module role: executable entrance. Exact constant-integer-wrapping integer addition replay join.

mod grammar;
mod replay;

use abstract_operations::AbstractFunction;
use target_operations::TargetFunction;

use super::{
    StraightLineWrappingIntegerAddImmediateTranslationError,
    StraightLineWrappingIntegerAddImmediateTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    grammar::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<
    StraightLineWrappingIntegerAddImmediateTranslationReceipt,
    StraightLineWrappingIntegerAddImmediateTranslationError,
> {
    let source = grammar::reconstruct(source)?;
    replay::validate(source, target)
}
