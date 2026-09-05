//! Exact source grammar for one constant whose widening becomes an immediate.

use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use semantic_vocabulary::{
    EdgeId, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};

use super::StraightLineIntegerWidenImmediateTranslationError as Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReconstructedIntegerWidenImmediate {
    pub(super) machine: MachineId,
    pub(super) constant_operation: OperationId,
    pub(super) widen_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) constant_result: ValueId,
    pub(super) widened_result: ValueId,
    pub(super) source_type: IntegerType,
    pub(super) target_type: IntegerType,
    pub(super) source_value: IntegerValue,
    pub(super) materialized_value: IntegerValue,
}

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
                AbstractOperation::IntegerWiden { .. },
                AbstractOperation::Return { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
) -> Result<ReconstructedIntegerWidenImmediate, Error> {
    let (function_result, function_type) = reconstruct_envelope(function)?;
    let [
        AbstractOperation::IntegerConstant {
            psi_operation: constant_operation,
            result: constant_result,
            scalar_type: constant_type,
            value: source_value,
        },
        AbstractOperation::IntegerWiden {
            psi_operation: widen_operation,
            result: widened_result,
            source_type,
            target_type,
            operand,
        },
        AbstractOperation::Return {
            psi_edge: return_edge,
            result,
            value,
            scalar_type: return_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Err(Error::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(Error::SourceCleanup);
    }
    if constant_operation == widen_operation
        || constant_result == widened_result
        || *constant_result == function_result
        || *widened_result == function_result
    {
        return Err(Error::SourceDefinitionRoster);
    }
    if *constant_type != ScalarType::Integer(*source_type) {
        return Err(Error::SourceConstantType);
    }
    if !source_type.admits(*source_value) {
        return Err(Error::SourceConstantOutsideType);
    }
    if *operand != *constant_result {
        return Err(Error::SourceWidenOperand);
    }
    if *target_type != function_type || !source_type.can_widen_to(*target_type) {
        return Err(Error::SourceWidenType);
    }
    if *result != function_result
        || *value != *widened_result
        || *return_type != ScalarType::Integer(*target_type)
    {
        return Err(Error::SourceResultLink);
    }
    let materialized_value = source_type
        .widen_value_to(*target_type, *source_value)
        .ok_or(Error::SourceWidenType)?;
    Ok(ReconstructedIntegerWidenImmediate {
        machine: function.machine,
        constant_operation: *constant_operation,
        widen_operation: *widen_operation,
        return_edge: *return_edge,
        constant_result: *constant_result,
        widened_result: *widened_result,
        source_type: *source_type,
        target_type: *target_type,
        source_value: *source_value,
        materialized_value,
    })
}

fn reconstruct_envelope(function: &AbstractFunction) -> Result<(ValueId, IntegerType), Error> {
    if !function.parameters.is_empty() {
        return Err(Error::SourceParameters);
    }
    if !function.structural_parameters.is_empty() {
        return Err(Error::SourceStructuralParameters);
    }
    let AbstractFunctionResult::Scalar(AbstractResult {
        value,
        scalar_type: ScalarType::Integer(scalar_type),
    }) = function.result
    else {
        return Err(Error::SourceResult);
    };
    if !function.entry_claims.is_empty() {
        return Err(Error::SourceEntryClaims);
    }
    if !function.published_service_ceiling.is_empty() {
        return Err(Error::SourcePublishedServices);
    }
    if !matches!(
        function.block_entries.as_slice(),
        [entry] if entry.block == function.entry
            && entry.parameters.is_empty()
            && entry.operation_offset == 0
    ) {
        return Err(Error::SourceBlockRoster);
    }
    Ok((value, scalar_type))
}
