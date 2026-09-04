//! Optimizer module role: executable entrance. Proof-bearing constant wrapping-remainder replay join.

mod grammar;
mod replay;

use omega_abstract_operations::AbstractFunction;
use omega_target_operations::TargetFunction;

use super::{
    StraightLineWrappingIntegerRemainderImmediateOperandsTranslationError,
    StraightLineWrappingIntegerRemainderImmediateOperandsTranslationReceipt,
};

pub(crate) fn is_candidate(function: &AbstractFunction) -> bool {
    grammar::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<
    StraightLineWrappingIntegerRemainderImmediateOperandsTranslationReceipt,
    StraightLineWrappingIntegerRemainderImmediateOperandsTranslationError,
> {
    let source = grammar::reconstruct(source)?;
    replay::validate(source, target)
}
