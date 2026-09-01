//! Optimizer module role: executable entrance. Per-function route order by exact result and boundary shape.

use super::boundary_settlements::lower_linux_exit_group_i32;
use super::shared::*;
use super::structural::lower_structural_function;
use super::unit::lower_unit_function;

pub(super) fn lower_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    fixed_integer_scalar_abis: &BTreeMap<MachineId, FixedIntegerScalarFunctionAbi>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    ieee_float_fma: &BTreeMap<OperationId, omega_target_operations::TargetX86ScalarFmaSettlement>,
    native_callbacks: &BTreeMap<OperationId, omega_target_operations::TargetNativeCallbackArgument>,
) -> Result<TargetFunction, LoweringError> {
    if let Some(lowered) =
        lower_linux_exit_group_i32(function, target, boundary_machines, settlements)?
    {
        return Ok(lowered);
    }
    if let Some(AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        ..
    }) = function.operations.iter().find(|operation| {
        matches!(
            operation,
            AbstractOperation::BoundaryCall {
                boundary,
                arguments,
                ..
            } if !arguments.is_empty()
                && !matches!(
                    settlements.get(boundary).map(|binding| &binding.realization),
                    Some(omega_target_operations::BoundarySettlementRealization::Builtin(
                        BoundaryRealization::LinuxExitGroupI32(_)
                    ))
                        | Some(
                            omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(_)
                        )
                )
        )
    }) {
        return Err(
            LoweringError::ScalarBoundaryArgumentsRequireNativeRealization {
                machine: function.machine,
                operation: *psi_operation,
                boundary: *boundary,
            },
        );
    }
    if let Some(result) = function.result.structural() {
        return lower_structural_function(function, result, target, functions, structural_types);
    }
    let Some(function_result) = function.result.scalar() else {
        return lower_unit_function(
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
        );
    };

    super::scalar::lower_scalar_function(
        function,
        function_result,
        target,
        functions,
        structural_types,
        settlements,
    )
}
