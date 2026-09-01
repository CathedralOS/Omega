//! Optimizer module role: executable entrance. Exact constant-Boolean-equality replay join.

mod grammar;
mod replay;

use omega_abstract_operations::AbstractFunction;
use omega_target_operations::TargetFunction;

use super::{
    StraightLineBooleanEqualImmediateTranslationError,
    StraightLineBooleanEqualImmediateTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    grammar::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<
    StraightLineBooleanEqualImmediateTranslationReceipt,
    StraightLineBooleanEqualImmediateTranslationError,
> {
    let source = grammar::reconstruct(source)?;
    replay::validate(source, target)
}
