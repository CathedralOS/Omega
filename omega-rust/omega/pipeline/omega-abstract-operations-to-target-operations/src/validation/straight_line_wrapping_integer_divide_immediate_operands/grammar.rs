//! Exact source grammar for proof-bearing wrapping divide of two integer constants.

use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use psi_core::{
    EdgeId, IntegerCarrier, IntegerType, IntegerValue, MachineId, ObligationId, OperationId,
    ScalarType, ValueId,
};

use super::StraightLineWrappingIntegerDivideImmediateOperandsTranslationError as Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReconstructedWrappingIntegerDivideImmediateOperands {
    pub(super) machine: MachineId,
    pub(super) left_constant_operation: OperationId,
    pub(super) right_constant_operation: OperationId,
    pub(super) divide_operation: OperationId,
    pub(super) obligation: ObligationId,
    pub(super) return_edge: EdgeId,
    pub(super) left_constant_result: ValueId,
    pub(super) right_constant_result: ValueId,
    pub(super) divide_result: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left: IntegerValue,
    pub(super) right: IntegerValue,
    pub(super) quotient: IntegerValue,
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
                AbstractOperation::WrappingIntegerDivide { .. },
                AbstractOperation::Return { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
) -> Result<ReconstructedWrappingIntegerDivideImmediateOperands, Error> {
    let (function_result, function_type) = reconstruct_envelope(function)?;
    let [
        AbstractOperation::IntegerConstant {
            psi_operation: left_constant_operation,
            result: left_constant_result,
            scalar_type: left_constant_type,
            value: left,
        },
        AbstractOperation::IntegerConstant {
            psi_operation: right_constant_operation,
            result: right_constant_result,
            scalar_type: right_constant_type,
            value: right,
        },
        AbstractOperation::WrappingIntegerDivide {
            psi_operation: divide_operation,
            obligation,
            result: divide_result,
            scalar_type,
            left: left_operand,
            right: right_operand,
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
    if duplicate_operation([
        *left_constant_operation,
        *right_constant_operation,
        *divide_operation,
    ]) || duplicate_result([
        *left_constant_result,
        *right_constant_result,
        *divide_result,
        function_result,
    ]) {
        return Err(Error::SourceDefinitionRoster);
    }
    let expected_type = ScalarType::Integer(*scalar_type);
    if *left_constant_type != expected_type || *right_constant_type != expected_type {
        return Err(Error::SourceConstantType);
    }
    if !is_native_fixed(*scalar_type) || *scalar_type != function_type {
        return Err(Error::SourceIntegerType);
    }
    if !scalar_type.admits(*left) || !scalar_type.admits(*right) {
        return Err(Error::SourceConstantOutsideType);
    }
    if *left_operand != *left_constant_result || *right_operand != *right_constant_result {
        return Err(Error::SourceDivideOperands);
    }
    let quotient = scalar_type
        .wrapping_div(*left, *right)
        .ok_or(Error::SourceDivideUndefined)?;
    if *result != function_result || *value != *divide_result || *return_type != expected_type {
        return Err(Error::SourceResultLink);
    }
    Ok(ReconstructedWrappingIntegerDivideImmediateOperands {
        machine: function.machine,
        left_constant_operation: *left_constant_operation,
        right_constant_operation: *right_constant_operation,
        divide_operation: *divide_operation,
        obligation: *obligation,
        return_edge: *return_edge,
        left_constant_result: *left_constant_result,
        right_constant_result: *right_constant_result,
        divide_result: *divide_result,
        scalar_type: *scalar_type,
        left: *left,
        right: *right,
        quotient,
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

fn is_native_fixed(integer: IntegerType) -> bool {
    integer.carrier() == IntegerCarrier::Fixed && matches!(integer.bits(), 8 | 16 | 32 | 64)
}

fn duplicate_operation(ids: [OperationId; 3]) -> bool {
    ids[0] == ids[1] || ids[0] == ids[2] || ids[1] == ids[2]
}

fn duplicate_result(ids: [ValueId; 4]) -> bool {
    ids.iter()
        .enumerate()
        .any(|(index, id)| ids[index + 1..].contains(id))
}
