//! Exact source grammar for one Boolean constant whose negation becomes an immediate.

use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use semantic_vocabulary::{EdgeId, MachineId, OperationId, ScalarType, ValueId};

use super::StraightLineBooleanNotImmediateTranslationError as Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReconstructedBooleanNotImmediate {
    pub(super) machine: MachineId,
    pub(super) constant_operation: OperationId,
    pub(super) boolean_not_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) constant_result: ValueId,
    pub(super) boolean_not_result: ValueId,
    pub(super) source_value: bool,
    pub(super) materialized_value: bool,
}

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
                AbstractOperation::BooleanNot { .. },
                AbstractOperation::Return { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
) -> Result<ReconstructedBooleanNotImmediate, Error> {
    let function_result = reconstruct_envelope(function)?;
    let [
        AbstractOperation::BooleanConstant {
            psi_operation: constant_operation,
            result: constant_result,
            value: source_value,
        },
        AbstractOperation::BooleanNot {
            psi_operation: boolean_not_operation,
            result: boolean_not_result,
            operand,
        },
        AbstractOperation::Return {
            psi_edge: return_edge,
            result,
            value,
            scalar_type,
            cleanup_actions,
        },
    ] = function.operations.as_slice()
    else {
        return Err(Error::SourceOperationRoster);
    };
    if !cleanup_actions.is_empty() {
        return Err(Error::SourceCleanup);
    }
    if constant_operation == boolean_not_operation
        || constant_result == boolean_not_result
        || *constant_result == function_result
        || *boolean_not_result == function_result
    {
        return Err(Error::SourceDefinitionRoster);
    }
    if *operand != *constant_result {
        return Err(Error::SourceBooleanNotOperand);
    }
    if *result != function_result
        || *value != *boolean_not_result
        || *scalar_type != ScalarType::Boolean
    {
        return Err(Error::SourceResultLink);
    }
    Ok(ReconstructedBooleanNotImmediate {
        machine: function.machine,
        constant_operation: *constant_operation,
        boolean_not_operation: *boolean_not_operation,
        return_edge: *return_edge,
        constant_result: *constant_result,
        boolean_not_result: *boolean_not_result,
        source_value: *source_value,
        materialized_value: !*source_value,
    })
}

fn reconstruct_envelope(function: &AbstractFunction) -> Result<ValueId, Error> {
    if !function.parameters.is_empty() {
        return Err(Error::SourceParameters);
    }
    if !function.structural_parameters.is_empty() {
        return Err(Error::SourceStructuralParameters);
    }
    let AbstractFunctionResult::Scalar(AbstractResult {
        value,
        scalar_type: ScalarType::Boolean,
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
    Ok(value)
}
