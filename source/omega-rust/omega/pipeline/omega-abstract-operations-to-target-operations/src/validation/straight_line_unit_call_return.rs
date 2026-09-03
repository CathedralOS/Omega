//! Independent replay of one parameterless internal Unit call followed by a Unit return.

use omega_abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetOperation, TargetUnitOperation};

use super::{
    StraightLineUnitCallReturnTranslationError, StraightLineUnitCallReturnTranslationReceipt,
};

pub(in crate::validation) fn is_candidate(function: &AbstractFunction) -> bool {
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
            [
                AbstractOperation::CallUnit {
                    arguments,
                    structural_arguments,
                    claim_transfers,
                    ..
                },
                AbstractOperation::ReturnUnit { cleanup_actions, .. },
            ] if arguments.is_empty()
                && structural_arguments.is_empty()
                && claim_transfers.is_empty()
                && cleanup_actions.is_empty()
        )
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<StraightLineUnitCallReturnTranslationReceipt, StraightLineUnitCallReturnTranslationError>
{
    if !source.parameters.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::SourceStructuralParameters);
    }
    if source.result != AbstractFunctionResult::Unit {
        return Err(StraightLineUnitCallReturnTranslationError::SourceResult);
    }
    if !source.entry_claims.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::SourceEntryClaims);
    }
    if !source.published_service_ceiling.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::SourcePublishedServices);
    }
    if !matches!(
        source.block_entries.as_slice(),
        [entry] if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(StraightLineUnitCallReturnTranslationError::SourceBlockRoster);
    }
    let [
        AbstractOperation::CallUnit {
            psi_operation,
            callee,
            arguments,
            structural_arguments,
            claim_transfers,
            requirement_obligations,
            crash_continuations,
        },
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        },
    ] = source.operations.as_slice()
    else {
        return Err(StraightLineUnitCallReturnTranslationError::SourceOperationRoster);
    };
    if !arguments.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::SourceParameters);
    }
    if !structural_arguments.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::SourceStructuralArguments);
    }
    if !claim_transfers.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::SourceClaimTransfers);
    }
    if !cleanup_actions.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::SourceCleanupActions);
    }
    if target.fixed_integer_scalar_abi.is_some() {
        return Err(StraightLineUnitCallReturnTranslationError::TargetFixedIntegerScalarAbi);
    }
    if target.provenance.operations.as_slice() != [*psi_operation]
        || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(StraightLineUnitCallReturnTranslationError::TargetProvenance);
    }
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(StraightLineUnitCallReturnTranslationError::TargetOperation);
    };
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(expected_target),
        &CallSignature::default(),
    )
    .map_err(|_| StraightLineUnitCallReturnTranslationError::TargetCallPlan)?;
    if body.call_plan != expected_call_plan {
        return Err(StraightLineUnitCallReturnTranslationError::TargetCallPlan);
    }
    if !body.parameters.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::TargetParameters);
    }
    let [
        TargetUnitOperation::Call {
            psi_operation: target_operation,
            callee: target_callee,
            arguments,
            claim_transfers: target_claim_transfers,
            requirement_obligations: target_requirement_obligations,
            crash_continuations: target_crash_continuations,
        },
        TargetUnitOperation::Return {
            psi_edge: target_edge,
            cleanup_actions: target_cleanup_actions,
        },
    ] = body.operations.as_slice()
    else {
        return Err(StraightLineUnitCallReturnTranslationError::TargetOperationRoster);
    };
    if target_operation != psi_operation
        || target_callee != callee
        || !arguments.is_empty()
        || !target_claim_transfers.is_empty()
        || target_requirement_obligations != requirement_obligations
        || target_crash_continuations != crash_continuations
    {
        return Err(StraightLineUnitCallReturnTranslationError::TargetCall);
    }
    if target_edge != psi_edge || !target_cleanup_actions.is_empty() {
        return Err(StraightLineUnitCallReturnTranslationError::TargetReturn);
    }
    Ok(StraightLineUnitCallReturnTranslationReceipt::new(
        source.machine,
        *psi_operation,
        *callee,
        requirement_obligations.clone(),
        crash_continuations.clone(),
        *psi_edge,
    ))
}
