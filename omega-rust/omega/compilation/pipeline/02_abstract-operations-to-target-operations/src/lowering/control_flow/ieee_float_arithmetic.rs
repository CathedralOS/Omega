//! Inline realization of a selected compiler float builtin.
//!
//! Stage 00 admitted this boundary's settlement only after rejoining the
//! selected provider row, the Terminal demand and the local target catalog.
//! The boundary call itself becomes one nearest-even IEEE operation on its
//! ordered scalar arguments (arithmetic, multiply-then-add, minimum/maximum, negation,
//! square root, or conversion into the result format), exactly like an
//! emitted IEEE comparison: there is no frame, argument transport or call, so
//! the result is an ordinary scalar home. The row keeps the boundary and admitted execution so legalization can
//! replay it against the abstract call and its declaration.

use super::RowSite;
use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use semantic_vocabulary::BoundaryMachineId;
use std::collections::BTreeMap;
use target::NativeTarget;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
    TargetUnitScalarHomeRequirement,
};

pub(super) fn lower(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
    target: NativeTarget,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
) -> Result<(), LoweringError> {
    let (function, types, live, operations, provenance) = site.parts();
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
    if realization.operation == target_operations::IeeeFloatArithmeticOperation::Classify {
        let AbstractBoundaryResult::Structural(result) = result else {
            return Err(mismatch);
        };
        let operand = semantic_vocabulary::ScalarType::IeeeFloat(realization.format);
        if binding.execution != execution
            || !realization.supports_target(target)
            || declaration.scalar_parameters != [operand]
            || !declaration.structural_parameters.is_empty()
            || !matches!(&declaration.result, terminal_psi::BoundaryMachineResult::Structural(declared) if declared.structural_type == result.structural_type && declared.multiplicity == result.multiplicity && declared.qualifications == result.qualifications && declared.projected_qualifications == result.projected_qualifications)
            || arguments.len() != 1
            || !structural_arguments.is_empty()
            || !completion_claim_sources.is_empty()
            || !completion_receipts.is_empty()
        {
            return Err(mismatch);
        }
        let argument = super::scalar_sources::source(arguments[0], function, live)?;
        if argument.scalar_type() != operand {
            return Err(mismatch);
        }
        if !result.is_unrestricted_plain() || !result.qualification_establishments.is_empty() {
            return Err(mismatch);
        }
        let layout = types
            .get(&result.structural_type)
            .and_then(|declaration| target_operations::conventional_float_class_layout(declaration))
            .ok_or(LoweringError::BoundaryRealizationMismatch(*boundary))?;
        let layout = target_operations::TargetStructuralHomeLayout::Sum(layout);
        let home = target_operations::TargetStructuralHomeRequirement {
            origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                operation: *psi_operation,
                result: result.clone(),
            },
            layout,
        };
        if live
            .structural_homes
            .insert(result.place, home.clone())
            .is_some()
        {
            return Err(mismatch);
        }
        provenance.operations.push(*psi_operation);
        operations.push(TargetUnitOperation::IeeeFloatClassify {
            psi_operation: *psi_operation,
            boundary: *boundary,
            execution,
            realization,
            result_home: home,
            argument,
        });
        return Ok(());
    }
    let scalar = realization.operation.result_type(realization.format);
    let operand = realization.operation.operand_type(realization.format);
    let AbstractBoundaryResult::Scalar(result) = result else {
        return Err(mismatch);
    };
    if binding.execution != execution
        || !realization.supports_target(target)
        || !realization.signature_matches(&declaration.scalar_parameters, scalar)
        || !declaration.structural_parameters.is_empty()
        || declaration.result != terminal_psi::BoundaryMachineResult::Scalar(scalar.into())
        || result.scalar_type != scalar
        || arguments.len() != realization.operation.operand_count()
        || !structural_arguments.is_empty()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
    {
        return Err(mismatch);
    }
    let sources = arguments
        .iter()
        .map(|argument| super::scalar_sources::source(*argument, function, live))
        .collect::<Result<Vec<_>, _>>()?;
    if sources.iter().any(|source| source.scalar_type() != operand) {
        return Err(LoweringError::ValueTypeMismatch(result.value));
    }
    let shape = crate::lowering::scalar_abi::fixed_native_scalar_shape(scalar).ok_or(mismatch)?;
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: result.value,
        scalar_type: scalar,
        shape,
    };
    // An integer result joins the known integers, as every other integer
    // result home does, so integer comparisons and stores observe it; an
    // IEEE result stays an ordinary scalar home.
    if matches!(scalar, semantic_vocabulary::ScalarType::Integer(_)) {
        crate::lowering::unit::known_integers::insert_known_unit_integer(
            &mut live.integers,
            result.value,
            crate::lowering::unit::known_integers::KnownUnitInteger::Home(result_home),
        )?;
    } else if live
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
        arguments: sources,
    });
    Ok(())
}
