//! Exact source grammar for a proof-bearing integer cast whose operand is constant.

use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use psi_core::{
    EdgeId, IntegerType, IntegerValue, MachineId, ObligationId, OperationId, ScalarType, ValueId,
};

use super::StraightLineIntegerExactCastImmediateOperandTranslationError as Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReconstructedIntegerExactCastImmediateOperand {
    pub(super) machine: MachineId,
    pub(super) constant_operation: OperationId,
    pub(super) cast_operation: OperationId,
    pub(super) obligation: ObligationId,
    pub(super) return_edge: EdgeId,
    pub(super) constant_result: ValueId,
    pub(super) cast_result: ValueId,
    pub(super) source_type: IntegerType,
    pub(super) target_type: IntegerType,
    pub(super) source_value: IntegerValue,
    pub(super) cast_value: IntegerValue,
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
                AbstractOperation::IntegerExactCast { .. },
                AbstractOperation::Return { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
) -> Result<ReconstructedIntegerExactCastImmediateOperand, Error> {
    let (function_result, function_type) = reconstruct_envelope(function)?;
    let [
        AbstractOperation::IntegerConstant {
            psi_operation: constant_operation,
            result: constant_result,
            scalar_type: constant_type,
            value: source_value,
        },
        AbstractOperation::IntegerExactCast {
            psi_operation: cast_operation,
            obligation,
            result: cast_result,
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
    if constant_operation == cast_operation
        || constant_result == cast_result
        || *constant_result == function_result
        || *cast_result == function_result
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
        return Err(Error::SourceCastOperand);
    }
    if !is_native(*source_type)
        || !is_native(*target_type)
        || source_type == target_type
        || source_type.can_widen_to(*target_type)
        || !source_type.can_exact_cast_to(*target_type)
        || *target_type != function_type
    {
        return Err(Error::SourceCastType);
    }
    let cast_value = source_type
        .exact_cast_value_to(*target_type, *source_value)
        .ok_or(Error::SourceCastValueOutsideTarget)?;
    if *result != function_result
        || *value != *cast_result
        || *return_type != ScalarType::Integer(*target_type)
    {
        return Err(Error::SourceResultLink);
    }
    Ok(ReconstructedIntegerExactCastImmediateOperand {
        machine: function.machine,
        constant_operation: *constant_operation,
        cast_operation: *cast_operation,
        obligation: *obligation,
        return_edge: *return_edge,
        constant_result: *constant_result,
        cast_result: *cast_result,
        source_type: *source_type,
        target_type: *target_type,
        source_value: *source_value,
        cast_value,
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

fn is_native(integer: IntegerType) -> bool {
    matches!(integer.bits(), 8 | 16 | 32 | 64)
}
