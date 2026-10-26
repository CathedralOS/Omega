//! A selected image address becomes an ordinary scalar home. It neither
//! introduces a storage root nor loans any bytes of its referent.

use super::RowSite;
use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use semantic_vocabulary::{BoundaryMachineId, IntegerType, ScalarType};
use std::collections::BTreeMap;
use target_operations::{
    BoundaryExecutionBinding, BoundaryRealization, BoundarySettlementBinding,
    BoundarySettlementRealization, CompilerBuiltinExecution, TargetUnitOperation,
    TargetUnitScalarHomeRequirement,
};

pub(super) fn lower(
    operation: &AbstractOperation,
    target: target::NativeTarget,
    declarations: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    site: &mut RowSite<'_>,
) -> Result<(), LoweringError> {
    let (_, _, live, operations, provenance) = site.parts();
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
        return Err(LoweringError::ProviderInstallationIdentityMismatch);
    };
    let mismatch = LoweringError::BoundaryRealizationMismatch(*boundary);
    let binding = settlements
        .get(boundary)
        .ok_or(LoweringError::MissingBoundarySettlement(*boundary))?;
    let declaration = declarations
        .get(boundary)
        .ok_or(LoweringError::UnknownBoundarySettlement(*boundary))?;
    let execution = BoundaryExecutionBinding::CompilerBuiltin(
        CompilerBuiltinExecution::ProgramImageRetainedSuffixStart,
    );
    let scalar = ScalarType::Integer(IntegerType::address(64).map_err(|_| mismatch.clone())?);
    let AbstractBoundaryResult::Scalar(result) = result else {
        return Err(mismatch);
    };
    if target != target::NativeTarget::uefi_x64()
        || binding.execution != execution
        || binding.realization
            != BoundarySettlementRealization::Builtin(
                BoundaryRealization::ProgramImageRetainedSuffixStart,
            )
        || !declaration.scalar_parameters.is_empty()
        || !declaration.structural_parameters.is_empty()
        || declaration.result != terminal_psi::BoundaryMachineResult::Scalar(scalar.into())
        || result.scalar_type != scalar
        || !arguments.is_empty()
        || !structural_arguments.is_empty()
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
    {
        return Err(mismatch);
    }
    let shape = crate::lowering::scalar_abi::fixed_native_scalar_shape(scalar).ok_or(mismatch)?;
    let result_home = TargetUnitScalarHomeRequirement {
        defining_operation: *psi_operation,
        source_value: result.value,
        scalar_type: scalar,
        shape,
    };
    crate::lowering::unit::known_integers::insert_known_unit_integer(
        &mut live.integers,
        result.value,
        crate::lowering::unit::known_integers::KnownUnitInteger::Home(result_home),
    )?;
    provenance.operations.push(*psi_operation);
    operations.push(TargetUnitOperation::ProgramImageAddress {
        psi_operation: *psi_operation,
        boundary: *boundary,
        execution,
        result_home,
    });
    Ok(())
}
