use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use omega_target_operations::{TargetFunction, TargetOperation};
use psi_core::ScalarType;

use super::{
    StraightLineBooleanImmediateTranslationError, StraightLineBooleanImmediateTranslationReceipt,
};

pub(super) fn is_candidate(function: &AbstractFunction) -> bool {
    function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && matches!(
            function.result,
            AbstractFunctionResult::Scalar(AbstractResult {
                scalar_type: ScalarType::Boolean,
                ..
            })
        )
        && matches!(
            function.block_entries.as_slice(),
            [entry] if entry.block == function.entry
                && entry.parameters.is_empty()
                && entry.operation_offset == 0
        )
        && matches!(
            function.operations.as_slice(),
            [
                AbstractOperation::BooleanConstant { .. },
                AbstractOperation::Return {
                    cleanup_actions,
                    ..
                }
            ] if cleanup_actions.is_empty()
        )
}

pub(crate) fn validate(
    source: &AbstractFunction,
    target: &TargetFunction,
) -> Result<
    StraightLineBooleanImmediateTranslationReceipt,
    StraightLineBooleanImmediateTranslationError,
> {
    if !source.parameters.is_empty() {
        return Err(StraightLineBooleanImmediateTranslationError::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(StraightLineBooleanImmediateTranslationError::SourceStructuralParameters);
    }
    let AbstractFunctionResult::Scalar(AbstractResult {
        value: function_result,
        scalar_type: ScalarType::Boolean,
    }) = source.result
    else {
        return Err(StraightLineBooleanImmediateTranslationError::SourceResult);
    };
    if !source.entry_claims.is_empty() {
        return Err(StraightLineBooleanImmediateTranslationError::SourceEntryClaims);
    }
    if !source.published_service_ceiling.is_empty() {
        return Err(StraightLineBooleanImmediateTranslationError::SourcePublishedServices);
    }
    if !matches!(
        source.block_entries.as_slice(),
        [entry] if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(StraightLineBooleanImmediateTranslationError::SourceBlockRoster);
    }
    let [
        AbstractOperation::BooleanConstant {
            psi_operation,
            result: constant_result,
            value,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value: returned_value,
            scalar_type,
            cleanup_actions,
        },
    ] = source.operations.as_slice()
    else {
        return Err(StraightLineBooleanImmediateTranslationError::SourceOperationRoster);
    };
    if *result != function_result
        || *returned_value != *constant_result
        || *scalar_type != ScalarType::Boolean
    {
        return Err(StraightLineBooleanImmediateTranslationError::SourceResultLink);
    }
    if !cleanup_actions.is_empty() {
        return Err(StraightLineBooleanImmediateTranslationError::SourceCleanup);
    }
    if target.provenance.operations.as_slice() != [*psi_operation]
        || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(StraightLineBooleanImmediateTranslationError::TargetProvenance);
    }
    if !matches!(
        target.operation,
        TargetOperation::ReturnBooleanImmediate {
            psi_edge: target_edge,
            source_value,
            value: target_value,
        } if target_edge == *psi_edge
            && source_value == *constant_result
            && target_value == *value
    ) {
        return Err(StraightLineBooleanImmediateTranslationError::TargetOperation);
    }
    Ok(StraightLineBooleanImmediateTranslationReceipt::new(
        source.machine,
        *psi_operation,
        *psi_edge,
        *constant_result,
        *value,
    ))
}
