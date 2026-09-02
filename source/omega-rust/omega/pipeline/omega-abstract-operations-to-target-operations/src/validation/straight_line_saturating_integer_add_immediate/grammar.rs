//! Exact source grammar for saturating addition of two same-type integer constants.

use omega_abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractResult,
};
use psi_core::{
    EdgeId, IntegerCarrier, IntegerType, IntegerValue, MachineId, OperationId, ScalarType, ValueId,
};

use super::StraightLineSaturatingIntegerAddImmediateTranslationError as Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ReconstructedSaturatingIntegerAddImmediate {
    pub(super) machine: MachineId,
    pub(super) left_constant_operation: OperationId,
    pub(super) right_constant_operation: OperationId,
    pub(super) saturating_add_operation: OperationId,
    pub(super) return_edge: EdgeId,
    pub(super) left_constant_result: ValueId,
    pub(super) right_constant_result: ValueId,
    pub(super) saturating_add_result: ValueId,
    pub(super) scalar_type: IntegerType,
    pub(super) left_value: IntegerValue,
    pub(super) right_value: IntegerValue,
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
                AbstractOperation::SaturatingIntegerAdd { .. },
                AbstractOperation::Return { cleanup_actions, .. },
            ] if cleanup_actions.is_empty()
        )
}

pub(super) fn reconstruct(
    function: &AbstractFunction,
) -> Result<ReconstructedSaturatingIntegerAddImmediate, Error> {
    let (function_result, function_type) = reconstruct_envelope(function)?;
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
        AbstractOperation::SaturatingIntegerAdd {
            psi_operation: saturating_add_operation,
            result: saturating_add_result,
            scalar_type,
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
        *saturating_add_operation,
    ]) || duplicate_result([
        *left_constant_result,
        *right_constant_result,
        *saturating_add_result,
        function_result,
    ]) {
        return Err(Error::SourceDefinitionRoster);
    }
    if *left_type != ScalarType::Integer(*scalar_type)
        || *right_type != ScalarType::Integer(*scalar_type)
    {
        return Err(Error::SourceConstantType);
    }
    if !is_native(*scalar_type) || *scalar_type != function_type {
        return Err(Error::SourceIntegerType);
    }
    if !scalar_type.admits(*left_value) || !scalar_type.admits(*right_value) {
        return Err(Error::SourceConstantOutsideType);
    }
    if *left != *left_constant_result || *right != *right_constant_result {
        return Err(Error::SourceSaturatingAddOperands);
    }
    if *result != function_result
        || *value != *saturating_add_result
        || *return_type != ScalarType::Integer(*scalar_type)
    {
        return Err(Error::SourceResultLink);
    }
    let materialized_value = scalar_type
        .saturating_add(*left_value, *right_value)
        .ok_or(Error::SourceIntegerType)?;
    Ok(ReconstructedSaturatingIntegerAddImmediate {
        machine: function.machine,
        left_constant_operation: *left_constant_operation,
        right_constant_operation: *right_constant_operation,
        saturating_add_operation: *saturating_add_operation,
        return_edge: *return_edge,
        left_constant_result: *left_constant_result,
        right_constant_result: *right_constant_result,
        saturating_add_result: *saturating_add_result,
        scalar_type: *scalar_type,
        left_value: *left_value,
        right_value: *right_value,
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
