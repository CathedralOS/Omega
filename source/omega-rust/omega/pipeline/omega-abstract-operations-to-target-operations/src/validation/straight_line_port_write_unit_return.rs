//! Independent replay of one immediate port write followed by a Unit return.

use omega_abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetOperation, TargetUnitOperation};

use super::super::{
    StraightLinePortWriteUnitReturnTranslationError,
    StraightLinePortWriteUnitReturnTranslationReceipt,
};

pub(in crate::validation) fn is_candidate(function: &AbstractFunction) -> bool {
    function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
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
                AbstractOperation::PortWrite { service, .. },
                AbstractOperation::ReturnUnit { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
                && function.published_service_ceiling.as_slice() == [*service]
        )
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLinePortWriteUnitReturnTranslationReceipt,
    StraightLinePortWriteUnitReturnTranslationError,
> {
    if !source.parameters.is_empty() {
        return Err(StraightLinePortWriteUnitReturnTranslationError::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(StraightLinePortWriteUnitReturnTranslationError::SourceStructuralParameters);
    }
    if source.result != AbstractFunctionResult::Unit {
        return Err(StraightLinePortWriteUnitReturnTranslationError::SourceResult);
    }
    if !source.entry_claims.is_empty() {
        return Err(StraightLinePortWriteUnitReturnTranslationError::SourceEntryClaims);
    }
    if !matches!(
        source.block_entries.as_slice(),
        [entry] if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(StraightLinePortWriteUnitReturnTranslationError::SourceBlockRoster);
    }
    let [
        AbstractOperation::PortWrite {
            psi_operation,
            service,
            port,
            value,
        },
        AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions,
        },
    ] = source.operations.as_slice()
    else {
        return Err(StraightLinePortWriteUnitReturnTranslationError::SourceOperationRoster);
    };
    if source.published_service_ceiling.as_slice() != [*service] {
        return Err(StraightLinePortWriteUnitReturnTranslationError::SourcePublishedServices);
    }
    if !cleanup_actions.is_empty() {
        return Err(StraightLinePortWriteUnitReturnTranslationError::SourceCleanupActions);
    }
    if target.fixed_integer_scalar_abi.is_some() {
        return Err(StraightLinePortWriteUnitReturnTranslationError::TargetFixedIntegerScalarAbi);
    }
    if target.provenance.operations.as_slice() != [*psi_operation]
        || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(StraightLinePortWriteUnitReturnTranslationError::TargetProvenance);
    }
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(StraightLinePortWriteUnitReturnTranslationError::TargetOperation);
    };
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(expected_target),
        &CallSignature::default(),
    )
    .map_err(|_| StraightLinePortWriteUnitReturnTranslationError::TargetCallPlan)?;
    if body.call_plan != expected_call_plan {
        return Err(StraightLinePortWriteUnitReturnTranslationError::TargetCallPlan);
    }
    if !body.parameters.is_empty() {
        return Err(StraightLinePortWriteUnitReturnTranslationError::TargetParameters);
    }
    let [
        TargetUnitOperation::PortWrite {
            psi_operation: target_operation,
            service: target_service,
            port: target_port,
            value: target_value,
        },
        TargetUnitOperation::Return {
            psi_edge: target_edge,
            cleanup_actions: target_cleanup_actions,
        },
    ] = body.operations.as_slice()
    else {
        return Err(StraightLinePortWriteUnitReturnTranslationError::TargetOperationRoster);
    };
    if target_operation != psi_operation
        || target_service != service
        || target_port != port
        || target_value != value
    {
        return Err(StraightLinePortWriteUnitReturnTranslationError::TargetPortWrite);
    }
    if target_edge != psi_edge || !target_cleanup_actions.is_empty() {
        return Err(StraightLinePortWriteUnitReturnTranslationError::TargetReturn);
    }
    Ok(StraightLinePortWriteUnitReturnTranslationReceipt::new(
        source.machine,
        *psi_operation,
        *service,
        *port,
        *value,
        *psi_edge,
    ))
}
