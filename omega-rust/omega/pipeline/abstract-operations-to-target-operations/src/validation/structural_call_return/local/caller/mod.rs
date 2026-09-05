//! Optimizer module role: executable entrance. Local projected structural-call caller replay.

mod replay;

use abstract_operations::{AbstractFunction, AbstractOperation};
use target_operations::TargetFunction;

use super::super::{
    StructuralCallReturnCallerTranslationReceipt,
    StructuralCallReturnProjectedQualificationValidationError as Error,
};

pub(crate) fn is_candidate(source: &AbstractFunction) -> bool {
    matches!(source.operations.as_slice(), [AbstractOperation::CallStructural { result, .. }, AbstractOperation::ReturnStructural { .. }]
        if !result.projected_qualifications.is_empty())
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<StructuralCallReturnCallerTranslationReceipt, Error> {
    replay::validate(source, target)
}
