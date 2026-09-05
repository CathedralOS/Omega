use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use semantic_vocabulary::ScalarType;
use target_operations::{TargetFunction, TargetOperation};

use super::{
    StraightLineIntegerImmediateTranslationError, StraightLineIntegerImmediateTranslationReceipt,
};

pub(super) fn is_candidate(function: &AbstractFunction) -> bool {
    function.parameters.is_empty()
        && function.structural_parameters.is_empty()
        && function.entry_claims.is_empty()
        && function.published_service_ceiling.is_empty()
        && matches!(
            function.result,
            AbstractFunctionResult::Scalar(AbstractResult {
                scalar_type: ScalarType::Integer(_),
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
                AbstractOperation::IntegerConstant { .. },
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
    StraightLineIntegerImmediateTranslationReceipt,
    StraightLineIntegerImmediateTranslationError,
> {
    if !source.parameters.is_empty() {
        return Err(StraightLineIntegerImmediateTranslationError::SourceParameters);
    }
    if !source.structural_parameters.is_empty() {
        return Err(StraightLineIntegerImmediateTranslationError::SourceStructuralParameters);
    }
    let AbstractFunctionResult::Scalar(AbstractResult {
        value: function_result,
        scalar_type: ScalarType::Integer(function_type),
    }) = source.result
    else {
        return Err(StraightLineIntegerImmediateTranslationError::SourceResult);
    };
    if !source.entry_claims.is_empty() {
        return Err(StraightLineIntegerImmediateTranslationError::SourceEntryClaims);
    }
    if !source.published_service_ceiling.is_empty() {
        return Err(StraightLineIntegerImmediateTranslationError::SourcePublishedServices);
    }
    if !matches!(
        source.block_entries.as_slice(),
        [entry] if entry.block == source.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(StraightLineIntegerImmediateTranslationError::SourceBlockRoster);
    }
    let [
        AbstractOperation::IntegerConstant {
            psi_operation,
            result: constant_result,
            scalar_type: constant_type,
            value,
        },
        AbstractOperation::Return {
            psi_edge,
            result,
            value: returned_value,
            scalar_type: return_type,
            cleanup_actions,
        },
    ] = source.operations.as_slice()
    else {
        return Err(StraightLineIntegerImmediateTranslationError::SourceOperationRoster);
    };
    if *constant_type != ScalarType::Integer(function_type) {
        return Err(StraightLineIntegerImmediateTranslationError::SourceConstantType);
    }
    if !function_type.admits(*value) {
        return Err(StraightLineIntegerImmediateTranslationError::SourceConstantOutsideType);
    }
    if *result != function_result
        || *returned_value != *constant_result
        || *return_type != ScalarType::Integer(function_type)
    {
        return Err(StraightLineIntegerImmediateTranslationError::SourceResultLink);
    }
    if !cleanup_actions.is_empty() {
        return Err(StraightLineIntegerImmediateTranslationError::SourceCleanup);
    }
    if target.provenance.operations.as_slice() != [*psi_operation]
        || target.provenance.edges.as_slice() != [*psi_edge]
    {
        return Err(StraightLineIntegerImmediateTranslationError::TargetProvenance);
    }
    if !matches!(
        target.operation,
        TargetOperation::ReturnIntegerImmediate {
            psi_edge: target_edge,
            source_value,
            scalar_type,
            value: target_value,
        } if target_edge == *psi_edge
            && source_value == *constant_result
            && scalar_type == function_type
            && target_value == *value
    ) {
        return Err(StraightLineIntegerImmediateTranslationError::TargetOperation);
    }
    Ok(StraightLineIntegerImmediateTranslationReceipt::new(
        source.machine,
        *psi_operation,
        *psi_edge,
        *constant_result,
        function_type,
        *value,
    ))
}
