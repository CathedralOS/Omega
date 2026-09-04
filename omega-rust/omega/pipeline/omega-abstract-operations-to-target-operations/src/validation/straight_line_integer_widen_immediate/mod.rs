//! Optimizer module role: executable entrance. Exact constant-widen-to-immediate replay join.

mod grammar;
mod replay;

use omega_abstract_operations::AbstractFunction;
use omega_target_operations::TargetFunction;

use super::{
    StraightLineIntegerWidenImmediateTranslationError,
    StraightLineIntegerWidenImmediateTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    grammar::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerWidenImmediateTranslationReceipt,
    StraightLineIntegerWidenImmediateTranslationError,
> {
    let source = grammar::reconstruct(source)?;
    replay::validate(source, target)
}
