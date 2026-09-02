//! Attached Unit lowering entrance: preflight, prepare, lower, and assemble.

mod body;
mod boundary_call;
mod conditional_exit;
mod dynamic_scalar;
mod preflight;
mod projected_argument;
mod return_unit;
mod scalar_call;
mod scalar_definitions;
mod setup;
mod structural_call;
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
            &prepared.parameters,
        )?
    };

    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        fixed_integer_scalar_abi: None,
        provenance: lowered.provenance,
        operation: TargetOperation::UnitBody(TargetUnitBody {
            structural_types: structural_types
                .values()
                .map(|declaration| (*declaration).clone())
                .collect(),
            call_plan: prepared.call_plan,
            parameters: prepared.parameters,
            operations: lowered.operations,
        }),
    })
}
