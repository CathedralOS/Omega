//! Inline realization of a selected compiler float-arithmetic builtin.
//!
//! Stage 00 admitted this boundary's settlement only after rejoining the
//! selected provider row, the Terminal demand and the local target catalog.
//! The boundary call itself becomes one nearest-even IEEE operation on its
//! two scalar arguments, exactly like an emitted IEEE comparison: there is no
//! frame, argument transport or call, so the result is an ordinary scalar
//! home. The row keeps the boundary and admitted execution so legalization can
//! replay it against the abstract call and its declaration.

use super::LiveDefinitions;
use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractFunction, AbstractOperation};
use semantic_vocabulary::{BoundaryMachineId, ScalarType};
use std::collections::BTreeMap;
use target::NativeTarget;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution, NearestIeeeFloatArithmeticRealization,
    TargetUnitOperation, TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
};

/// Whether this boundary's admitted settlement realizes inline arithmetic.
pub(super) fn is_inline_arithmetic(
    boundary: BoundaryMachineId,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
) -> bool {
    settlements.get(&boundary).is_some_and(|binding| {
        matches!(
            binding.realization,
            BoundarySettlementRealization::Builtin(
                BoundaryRealization::NearestIeeeFloatArithmetic(_)
            )
        )
    })
}

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    target: NativeTarget,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::BoundaryCall {
        psi_operation,
        result,
        boundary,
        arguments,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
    } = operation
    else {
        unreachable!("inline float arithmetic routes only boundary calls")
    };
    let mismatch = LoweringError::BoundaryRealizationMismatch(*boundary);
    let binding = settlements
        .get(boundary)
        .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
    let declaration = boundary_machines
        .get(boundary)
        .copied()
        .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
    let BoundarySettlementRealization::Builtin(BoundaryRealization::NearestIeeeFloatArithmetic(
        realization,
    )) = binding.realization
    else {
        return Err(mismatch);
    };
    // A compiler builtin always pairs with its own realization; a different
    // execution beside this realization is forged by construction.
    let execution = BoundaryExecutionBinding::CompilerBuiltin(
        CompilerBuiltinExecution::NearestIeeeFloatArithmetic {
            operation: realization.operation,
            format: realization.format,
        },
    );
    let scalar = ScalarType::IeeeFloat(realization.format);
    let (AbstractBoundaryResult::Scalar(result), [left, right]) = (result, arguments.as_slice())
    else {
        return Err(mismatch);
    };
    if binding.execution != execution
        || !NearestIeeeFloatArithmeticRealization::supports_target(target)
        || declaration.scalar_parameters != [scalar, scalar]
        || !declaration.structural_parameters.is_empty()
        || declaration.result != terminal_psi::BoundaryMachineResult::Scalar(scalar)
        || result.scalar_type != scalar
        || !structural_arguments.is_empty()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
    {
        return Err(mismatch);
    }
    let left = super::scalar_sources::source(*left, function, live)?;
    let right = super::scalar_sources::source(*right, function, live)?;
    if left.scalar_type() != scalar || right.scalar_type() != scalar {
        return Err(LoweringError::ValueTypeMismatch(result.value));
    }
    let shape = crate::lowering::scalar_abi::fixed_native_scalar_shape(scalar).ok_or(mismatch)?;
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: result.value,
        scalar_type: scalar,
        shape,
    };
    if live
        .scalar_homes
        .insert(result.value, result_home)
        .is_some()
    {
        return Err(LoweringError::DuplicateValue(result.value));
    }
    provenance.operations.push(*psi_operation);
    operations.push(TargetUnitOperation::NearestIeeeFloatArithmetic {
        psi_operation: *psi_operation,
        boundary: *boundary,
        execution,
        realization,
        result_home,
        left,
        right,
    });
    Ok(())
}
