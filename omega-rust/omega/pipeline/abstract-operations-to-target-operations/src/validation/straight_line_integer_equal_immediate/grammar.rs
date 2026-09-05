//! Exact source grammar for equality of two ordered same-type integer constants.

use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use semantic_vocabulary::{
    EdgeId, IntegerCarrier, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};

use super::StraightLineIntegerEqualImmediateTranslationError as Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReconstructedIntegerEqualImmediate {
    pub(super) machine: MachineId,
    pub(super) left_constant_operation: OperationId,
    pub(super) right_constant_operation: OperationId,
    pub(super) equal_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) left_constant_result: ValueId,
    pub(super) right_constant_result: ValueId,
    pub(super) equal_result: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left_value: IntegerValue,
    pub(super) right_value: IntegerValue,
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
                AbstractOperation::IntegerConstant { .. },
                AbstractOperation::IntegerConstant { .. },
                AbstractOperation::IntegerEqual { .. },
                AbstractOperation::Return { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
) -> Result<ReconstructedIntegerEqualImmediate, Error> {
    let function_result = reconstruct_envelope(function)?;
    let [
        AbstractOperation::IntegerConstant {
            psi_operation: left_constant_operation,
            result: left_constant_result,
            scalar_type: left_type,
            value: left_value,
        },
        AbstractOperation::IntegerConstant {
            psi_operation: right_constant_operation,
            result: right_constant_result,
            scalar_type: right_type,
            value: right_value,
        },
        AbstractOperation::IntegerEqual {
            psi_operation: equal_operation,
            result: equal_result,
            left,
            right,
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
        *equal_operation,
    ]) || duplicate_result([
        *left_constant_result,
        *right_constant_result,
        *equal_result,
        function_result,
    ]) {
        return Err(Error::SourceDefinitionRoster);
    }
    let ScalarType::Integer(scalar_type) = *left_type else {
        return Err(Error::SourceConstantType);
    };
    if *right_type != ScalarType::Integer(scalar_type) {
        return Err(Error::SourceConstantType);
    }
    if !is_native(scalar_type) {
        return Err(Error::SourceIntegerType);
    }
    if !scalar_type.admits(*left_value) || !scalar_type.admits(*right_value) {
        return Err(Error::SourceConstantOutsideType);
    }
    if *left != *left_constant_result || *right != *right_constant_result {
        return Err(Error::SourceEqualOperands);
    }
    if *result != function_result || *value != *equal_result || *return_type != ScalarType::Boolean
    {
        return Err(Error::SourceResultLink);
    }
    Ok(ReconstructedIntegerEqualImmediate {
        machine: function.machine,
        left_constant_operation: *left_constant_operation,
        right_constant_operation: *right_constant_operation,
        equal_operation: *equal_operation,
        return_edge: *return_edge,
        left_constant_result: *left_constant_result,
        right_constant_result: *right_constant_result,
        equal_result: *equal_result,
        scalar_type,
        left_value: *left_value,
        right_value: *right_value,
        materialized_value: left_value == right_value,
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
