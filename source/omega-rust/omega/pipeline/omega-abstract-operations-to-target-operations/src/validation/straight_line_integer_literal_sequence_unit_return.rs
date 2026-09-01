//! Independent replay of a finite multi-literal integer sequence and its Unit return.

use omega_abstract_operations::{AbstractFunction, AbstractFunctionResult, AbstractOperation};
use omega_calling_conventions::{CallSignature, CallingPolicy, evaluate_call_plan};
use omega_target::NativeTarget;
use omega_target_operations::{TargetFunction, TargetOperation, TargetUnitOperation};
use psi_core::ScalarType;

use super::{
    IntegerLiteralSequenceMember, StraightLineIntegerLiteralSequenceUnitReturnTranslationError,
    StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt,
};

pub(in crate::validation) fn is_candidate(function: &AbstractFunction) -> bool {
    let Some((last, literals)) = function.operations.split_last() else {
        return false;
    };
    function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && function.result == AbstractFunctionResult::Unit
        && matches!(function.block_entries.as_slice(), [entry]
            if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0)
        && literals.len() >= 2
        && literals
            .iter()
            .all(|operation| matches!(operation, AbstractOperation::IntegerConstant { .. }))
        && matches!(last, AbstractOperation::ReturnUnit { cleanup_actions, .. }
            if cleanup_actions.is_empty())
}

pub(crate) fn validate(
    source: &AbstractFunction,
    expected_target: NativeTarget,
    target: &TargetFunction,
) -> Result<
    StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt,
    StraightLineIntegerLiteralSequenceUnitReturnTranslationError,
> {
    use StraightLineIntegerLiteralSequenceUnitReturnTranslationError as Error;

    if !source.parameters.is_empty() {
        return Err(Error::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(Error::SourceStructuralParameters);
    }
    if source.result != AbstractFunctionResult::Unit {
        return Err(Error::SourceResult);
    }
    if !source.entry_claims.is_empty() {
        return Err(Error::SourceEntryClaims);
    }
    if !source.published_service_ceiling.is_empty() {
        return Err(Error::SourcePublishedServices);
    }
    if !matches!(source.block_entries.as_slice(), [entry]
        if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0)
    {
        return Err(Error::SourceBlockRoster);
    }
    let Some((source_return, source_literals)) = source.operations.split_last() else {
        return Err(Error::SourceOperationRoster);
    };
    if source_literals.len() < 2
        || !source_literals
            .iter()
            .all(|operation| matches!(operation, AbstractOperation::IntegerConstant { .. }))
    {
        return Err(Error::SourceOperationRoster);
    }
    for literal in source_literals {
        let AbstractOperation::IntegerConstant {
            scalar_type, value, ..
        } = literal
        else {
            unreachable!("integer literal roster was checked above")
        };
        let ScalarType::Integer(integer_type) = scalar_type else {
            return Err(Error::SourceConstantType);
        };
        if !integer_type.admits(*value) {
            return Err(Error::SourceConstantOutsideType);
        }
    }
    let AbstractOperation::ReturnUnit {
        psi_edge,
        cleanup_actions,
    } = source_return
    else {
        return Err(Error::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(Error::SourceCleanupActions);
    }
    if target.fixed_integer_scalar_abi.is_some() {
        return Err(Error::TargetFixedIntegerScalarAbi);
    }
    if target.provenance.operations.len() != source_literals.len()
        || !source_literals
            .iter()
            .zip(&target.provenance.operations)
            .all(|(source, target)| {
                matches!(source,
                AbstractOperation::IntegerConstant { psi_operation, .. }
                    if psi_operation == target)
            })
        || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(Error::TargetProvenance);
    }
    let TargetOperation::UnitBody(body) = &target.operation else {
        return Err(Error::TargetOperation);
    };
    let expected_call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(expected_target),
        &CallSignature::default(),
    )
    .map_err(|_| Error::TargetCallPlan)?;
    if body.call_plan != expected_call_plan {
        return Err(Error::TargetCallPlan);
    }
    if !body.parameters.is_empty() {
        return Err(Error::TargetParameters);
    }
    let Some((target_return, target_literals)) = body.operations.split_last() else {
        return Err(Error::TargetOperationRoster);
    };
    if target_literals.len() != source_literals.len()
        || !target_literals
            .iter()
            .all(|operation| matches!(operation, TargetUnitOperation::IntegerConstant { .. }))
    {
        return Err(Error::TargetOperationRoster);
    }
    if !source_literals
        .iter()
        .zip(target_literals)
        .all(|(source, target)| {
            matches!((source, target),
            (AbstractOperation::IntegerConstant {
                psi_operation, result, scalar_type: ScalarType::Integer(integer_type), value,
            }, TargetUnitOperation::IntegerConstant {
                psi_operation: target_operation, result: target_result,
                scalar_type: target_type, value: target_value,
            }) if target_operation == psi_operation
                && target_result == result
                && target_type == integer_type
                && target_value == value)
        })
    {
        return Err(Error::TargetConstant);
    }
    let TargetUnitOperation::Return {
        psi_edge: target_edge,
        cleanup_actions: target_cleanup,
    } = target_return
    else {
        return Err(Error::TargetOperationRoster);
    };
    if target_edge != psi_edge || target_cleanup != cleanup_actions {
        return Err(Error::TargetReturn);
    }

    let literals = source_literals
        .iter()
        .map(|operation| {
            let AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type: ScalarType::Integer(integer_type),
                value,
            } = operation
            else {
                unreachable!("typed integer literal roster was admitted above")
            };
            IntegerLiteralSequenceMember::new(*psi_operation, *result, *integer_type, *value)
        })
        .collect();
    Ok(
        StraightLineIntegerLiteralSequenceUnitReturnTranslationReceipt::new(
            source.machine,
            literals,
            *psi_edge,
        ),
    )
}
