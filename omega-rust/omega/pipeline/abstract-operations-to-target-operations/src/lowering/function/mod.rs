//! Optimizer module role: executable entrance. Per-function route order by exact result and boundary shape.

use super::boundary_settlements::lower_linux_exit_group_i32;
use super::shared::*;
use super::structural::lower_structural_function;
use super::unit::lower_unit_function;

mod native_boundaries;

pub(super) fn lower_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    fixed_integer_scalar_abis: &BTreeMap<MachineId, FixedIntegerScalarFunctionAbi>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
    ieee_float_fma: &BTreeMap<OperationId, target_operations::TargetX86ScalarFmaSettlement>,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
) -> Result<TargetFunction, LoweringError> {
    if !native_boundaries::has_installed_scalar_call(function, installed_calls)
        && let Some(lowered) =
            lower_linux_exit_group_i32(function, target, boundary_machines, settlements)?
    {
        return Ok(lowered);
    }
    if let Some((operation, boundary)) =
        native_boundaries::unsupported_scalar_call(function, settlements, installed_calls)
    {
        return Err(
            LoweringError::ScalarBoundaryArgumentsRequireNativeRealization {
                machine: function.machine,
                operation,
                boundary,
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
