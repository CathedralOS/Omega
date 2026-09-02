//! Attached Unit lowering entrance: preflight, prepare, lower, and assemble.

mod body;
mod boundary_call;
mod conditional_exit;
mod dynamic;
mod preflight;
mod projected_argument;
mod return_unit;
mod scalar_call;
mod scalar_definitions;
mod setup;
mod structural_call;
mod structural_result;
mod structural_scalar;

use super::shared::*;
use body::lower_unit_body;
use preflight::validate_unit_function_shape;
use scalar_definitions::validate_unit_scalar_definitions;
use setup::prepare_unit_function;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_unit_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    fixed_integer_scalar_abis: &BTreeMap<MachineId, FixedIntegerScalarFunctionAbi>,
    ieee_float_fma: &BTreeMap<OperationId, TargetX86ScalarFmaSettlement>,
    native_callbacks: &BTreeMap<OperationId, omega_target_operations::TargetNativeCallbackArgument>,
) -> Result<TargetFunction, LoweringError> {
    let bounded_conditional_exit = conditional_exit::has_bounded_shape(function);
    if !bounded_conditional_exit {
        validate_unit_function_shape(function)?;
    }
    validate_unit_scalar_definitions(function)?;

    let prepared = prepare_unit_function(function, target, structural_types)?;
    let lowered = if bounded_conditional_exit {
        conditional_exit::lower(
            function,
            target,
            functions,
            structural_types,
            boundary_machines,
            settlements,
            installed_calls,
            native_callbacks,
            &prepared.parameters,
        )?
    } else {
        lower_unit_body(
            function,
            target,
            functions,
            structural_types,
            boundary_machines,
            settlements,
            installed_calls,
            fixed_integer_scalar_abis,
            ieee_float_fma,
            native_callbacks,
            &prepared.scalar_parameters,
            &prepared.parameters,
        )?
    };
    validate_bounded_installed_scalar_body(function.machine, &lowered.operations)?;

    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance: lowered.provenance,
        operation: TargetOperation::UnitBody(TargetUnitBody {
            structural_types: structural_types
                .values()
                .map(|declaration| (*declaration).clone())
                .collect(),
            call_plan: prepared.call_plan,
            scalar_parameters: prepared.scalar_parameters,
            parameters: prepared.parameters,
            operations: lowered.operations,
        }),
    })
}

fn validate_bounded_installed_scalar_body(
    machine: MachineId,
    operations: &[TargetUnitOperation],
) -> Result<(), LoweringError> {
    let Some((psi_operation, boundary)) = operations.iter().find_map(|operation| match operation {
        TargetUnitOperation::InstalledProviderCall {
            psi_operation,
            boundary,
            scalar_arguments,
            ..
        } if !scalar_arguments.is_empty() => Some((*psi_operation, *boundary)),
        _ => None,
    }) else {
        return Ok(());
    };
    if !matches!(
        operations,
        [
            TargetUnitOperation::InstalledProviderCall {
                psi_operation: actual_operation,
                boundary: actual_boundary,
                scalar_arguments,
                source_arguments,
                arguments,
                claim_transfers,
                completion_claim_sources,
                completion_receipts,
                ..
            },
            TargetUnitOperation::Return { .. },
        ] if *actual_operation == psi_operation
            && *actual_boundary == boundary
            && scalar_arguments.len() == 1
            && source_arguments.is_empty()
            && arguments.is_empty()
            && claim_transfers.is_empty()
            && completion_claim_sources.is_empty()
            && completion_receipts.is_empty()
    ) {
        return Err(LoweringError::InstalledProviderCallShapeMismatch {
            machine,
            operation: psi_operation,
            boundary,
        });
    }
    Ok(())
}
