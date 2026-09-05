//! Optimizer module role: stage group. Independent replay entrance for ordered mixed integer/IEEE literals.

mod grammar;
mod replay;

use abstract_operations::AbstractFunction;
use target::NativeTarget;
use target_operations::TargetFunction;

use super::{
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
};

pub(in crate::validation) fn is_candidate(function: &AbstractFunction) -> bool {
    grammar::is_candidate(function)
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIntegerIeeeFloatLiteralSequenceUnitReturnTranslationError,
> {
    let source = grammar::reconstruct(source)?;
    replay::validate(&source, expected_target, target)?;
    Ok(source.into_receipt())
}
