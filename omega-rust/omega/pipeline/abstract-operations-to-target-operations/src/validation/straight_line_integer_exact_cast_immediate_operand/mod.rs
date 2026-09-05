//! Optimizer module role: executable entrance. Exact constant-to-proof-bearing-cast replay join.

mod grammar;
mod replay;

use abstract_operations::AbstractFunction;
use target_operations::TargetFunction;

use super::{
    StraightLineIntegerExactCastImmediateOperandTranslationError,
    StraightLineIntegerExactCastImmediateOperandTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    grammar::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerExactCastImmediateOperandTranslationReceipt,
    StraightLineIntegerExactCastImmediateOperandTranslationError,
> {
    let source = grammar::reconstruct(source)?;
    replay::validate(source, target)
}
