//! Optimizer module role: executable entrance. Exact constant-Boolean-not replay join.

mod grammar;
mod replay;

use abstract_operations::AbstractFunction;
use target_operations::TargetFunction;

use super::{
    StraightLineBooleanNotImmediateTranslationError,
    StraightLineBooleanNotImmediateTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    grammar::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<
    StraightLineBooleanNotImmediateTranslationReceipt,
    StraightLineBooleanNotImmediateTranslationError,
> {
    let source = grammar::reconstruct(source)?;
    replay::validate(source, target)
}
