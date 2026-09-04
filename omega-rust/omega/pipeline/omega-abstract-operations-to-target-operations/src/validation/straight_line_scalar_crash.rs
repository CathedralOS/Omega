use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use omega_target_operations::{TargetFunction, TargetOperation};

use super::{StraightLineScalarCrashTranslationError, StraightLineScalarCrashTranslationReceipt};

pub(super) fn is_candidate(function: &AbstractFunction) -> bool {
    function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && matches!(
            function.result,
            AbstractFunctionResult::Scalar(AbstractResult { .. })
        )
        && matches!(
            function.block_entries.as_slice(),
            [entry] if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0
        )
        && matches!(
            function.operations.as_slice(),
            [AbstractOperation::Crash { .. }]
        )
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<StraightLineScalarCrashTranslationReceipt, StraightLineScalarCrashTranslationError> {
    if !source.parameters.is_empty() {
        return Err(StraightLineScalarCrashTranslationError::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(StraightLineScalarCrashTranslationError::SourceStructuralParameters);
    }
    let AbstractFunctionResult::Scalar(AbstractResult {
        scalar_type: result_type,
        ..
    }) = source.result
    else {
        return Err(StraightLineScalarCrashTranslationError::SourceResult);
    };
    if !source.entry_claims.is_empty() {
        return Err(StraightLineScalarCrashTranslationError::SourceEntryClaims);
    }
    if !source.published_service_ceiling.is_empty() {
        return Err(StraightLineScalarCrashTranslationError::SourcePublishedServices);
    }
    if !matches!(
        source.block_entries.as_slice(),
        [entry] if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(StraightLineScalarCrashTranslationError::SourceBlockRoster);
    }
    let [
        AbstractOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        },
    ] = source.operations.as_slice()
    else {
        return Err(StraightLineScalarCrashTranslationError::SourceOperationRoster);
    };
    if !target.provenance.operations.is_empty() || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(StraightLineScalarCrashTranslationError::TargetProvenance);
    }
    let TargetOperation::Crash {
        psi_edge: target_edge,
        cause: target_cause,
        site_guard: target_guard,
        frontier_lower_bound: target_frontier,
    } = &target.operation
    else {
        return Err(StraightLineScalarCrashTranslationError::TargetOperation);
    };
    if target_edge != psi_edge
        || target_cause != cause
        || target_guard != site_guard
        || target_frontier != frontier_lower_bound
    {
        return Err(StraightLineScalarCrashTranslationError::TargetOperation);
    }
    Ok(StraightLineScalarCrashTranslationReceipt::new(
        source.machine,
        result_type,
        *psi_edge,
        *cause,
        site_guard.clone(),
        frontier_lower_bound.clone(),
    ))
}
