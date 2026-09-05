//! Independent replay of one parameterless straight-line Unit return.

use abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use target::NativeTarget;
use target_operations::{TargetFunction, TargetOperation, TargetUnitOperation};

use super::{StraightLineUnitReturnTranslationError, StraightLineUnitReturnTranslationReceipt};

pub(super) fn is_candidate(function: &AbstractFunction) -> bool {
    function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && function.result == AbstractFunctionResult::Unit
        && matches!(
            function.block_entries.as_slice(),
            [entry] if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0
        )
        && matches!(
            function.operations.as_slice(),
            [AbstractOperation::ReturnUnit { cleanup_actions, .. }]
                if cleanup_actions.is_empty()
        )
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<StraightLineUnitReturnTranslationReceipt, StraightLineUnitReturnTranslationError> {
    if !source.parameters.is_empty() {
        return Err(StraightLineUnitReturnTranslationError::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(StraightLineUnitReturnTranslationError::SourceStructuralParameters);
    }
    if source.result != AbstractFunctionResult::Unit {
        return Err(StraightLineUnitReturnTranslationError::SourceResult);
    }
    if !source.entry_claims.is_empty() {
        return Err(StraightLineUnitReturnTranslationError::SourceEntryClaims);
    }
    if !source.published_service_ceiling.is_empty() {
        return Err(StraightLineUnitReturnTranslationError::SourcePublishedServices);
    }
    if !matches!(
        source.block_entries.as_slice(),
        [entry] if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(StraightLineUnitReturnTranslationError::SourceBlockRoster);
    }
    let [
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        },
    ] = source.operations.as_slice()
    else {
        return Err(StraightLineUnitReturnTranslationError::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(StraightLineUnitReturnTranslationError::SourceCleanupActions);
    }
    if target.fixed_integer_scalar_abi.is_some() {
        return Err(StraightLineUnitReturnTranslationError::TargetFixedIntegerScalarAbi);
    }
    if !target.provenance.operations.is_empty() || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(StraightLineUnitReturnTranslationError::TargetProvenance);
    }
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(StraightLineUnitReturnTranslationError::TargetOperation);
    };
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(expected_target),
        &CallSignature::default(),
    )
    .map_err(|_| StraightLineUnitReturnTranslationError::TargetCallPlan)?;
    if body.call_plan != expected_call_plan {
        return Err(StraightLineUnitReturnTranslationError::TargetCallPlan);
    }
    if !body.parameters.is_empty() {
        return Err(StraightLineUnitReturnTranslationError::TargetParameters);
    }
    let [
        TargetUnitOperation::Return {
            psi_edge: target_edge,
            cleanup_actions: target_cleanup_actions,
        },
    ] = body.operations.as_slice()
    else {
        return Err(StraightLineUnitReturnTranslationError::TargetOperationRoster);
    };
    if target_edge != psi_edge || !target_cleanup_actions.is_empty() {
        return Err(StraightLineUnitReturnTranslationError::TargetReturn);
    }
    Ok(StraightLineUnitReturnTranslationReceipt::new(
        source.machine,
        *psi_edge,
    ))
}
