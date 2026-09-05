//! Exact source grammar for wrapping right shift of two independently typed integer constants.

use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use semantic_vocabulary::{
    EdgeId, IntegerCarrier, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};

use super::StraightLineWrappingIntegerShiftRightImmediateTranslationError as Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReconstructedWrappingIntegerShiftRightImmediate {
    pub(super) machine: MachineId,
    pub(super) value_constant_operation: OperationId,
    pub(super) count_constant_operation: OperationId,
    pub(super) wrapping_shift_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) value_constant_result: ValueId,
    pub(super) count_constant_result: ValueId,
    pub(super) wrapping_shift_result: ValueId,
    pub(super) value_type: IntegerType,
    pub(super) count_type: IntegerType,
    pub(super) value: IntegerValue,
    pub(super) count: IntegerValue,
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
                AbstractOperation::IntegerConstant { .. },
                AbstractOperation::WrappingIntegerShiftRight { .. },
                AbstractOperation::Return { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
) -> Result<ReconstructedWrappingIntegerShiftRightImmediate, Error> {
    let (function_result, function_type) = reconstruct_envelope(function)?;
    let [
        AbstractOperation::IntegerConstant {
            psi_operation: value_constant_operation,
            result: value_constant_result,
            scalar_type: constant_value_type,
            value,
        },
        AbstractOperation::IntegerConstant {
            psi_operation: count_constant_operation,
            result: count_constant_result,
            scalar_type: constant_count_type,
            value: count,
        },
        AbstractOperation::WrappingIntegerShiftRight {
            psi_operation: wrapping_shift_operation,
            result: wrapping_shift_result,
            value_type,
            count_type,
            value: value_operand,
            count: count_operand,
        },
        AbstractOperation::Return {
            psi_edge: return_edge,
            result,
            value: returned_value,
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
    if duplicate_operation([
        *value_constant_operation,
        *count_constant_operation,
        *wrapping_shift_operation,
    ]) || duplicate_result([
        *value_constant_result,
        *count_constant_result,
        *wrapping_shift_result,
        function_result,
    ]) {
        return Err(Error::SourceDefinitionRoster);
    }
    if *constant_value_type != ScalarType::Integer(*value_type) {
        return Err(Error::SourceValueConstantType);
    }
    if *constant_count_type != ScalarType::Integer(*count_type) {
        return Err(Error::SourceCountConstantType);
    }
    if !is_native(*value_type) || *value_type != function_type {
        return Err(Error::SourceValueType);
    }
    if !is_native(*count_type) {
        return Err(Error::SourceCountType);
    }
    if !value_type.admits(*value) {
        return Err(Error::SourceValueOutsideType);
    }
    if !count_type.admits(*count) {
        return Err(Error::SourceCountOutsideType);
    }
    if *value_operand != *value_constant_result || *count_operand != *count_constant_result {
        return Err(Error::SourceWrappingShiftOperands);
    }
    if *result != function_result
        || *returned_value != *wrapping_shift_result
        || *return_type != ScalarType::Integer(*value_type)
    {
        return Err(Error::SourceResultLink);
    }
    let materialized_value = value_type
        .wrapping_shift_right(*value, *count_type, *count)
        .ok_or(Error::SourceValueType)?;
    Ok(ReconstructedWrappingIntegerShiftRightImmediate {
        machine: function.machine,
        value_constant_operation: *value_constant_operation,
        count_constant_operation: *count_constant_operation,
        wrapping_shift_operation: *wrapping_shift_operation,
        return_edge: *return_edge,
        value_constant_result: *value_constant_result,
        count_constant_result: *count_constant_result,
        wrapping_shift_result: *wrapping_shift_result,
        value_type: *value_type,
        count_type: *count_type,
        value: *value,
        count: *count,
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
        scalar_type: ScalarType::Integer(value_type),
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
    Ok((value, value_type))
}

fn is_native(integer: IntegerType) -> bool {
    match integer.carrier() {
        IntegerCarrier::Fixed => matches!(integer.bits(), 8 | 16 | 32 | 64),
        IntegerCarrier::Address => integer.bits() == 64,
    }
}

fn duplicate_operation(ids: [OperationId; 3]) -> bool {
    ids[0] == ids[1] || ids[0] == ids[2] || ids[1] == ids[2]
}

fn duplicate_result(ids: [ValueId; 4]) -> bool {
    ids.iter()
        .enumerate()
        .any(|(index, id)| ids[index + 1..].contains(id))
}
