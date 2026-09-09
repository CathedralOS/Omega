//! Optimizer module role: executable entrance. Per-function route order by exact result and boundary shape.

use super::shared::*;
use super::structural::lower_structural_function;
use super::unit::lower_unit_function;

mod native_boundaries;

pub(super) fn lower_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
    ieee_float_fma: &BTreeMap<OperationId, target_operations::TargetX86ScalarFmaSettlement>,
    native_callbacks: &BTreeMap<OperationId, target_operations::TargetNativeCallbackArgument>,
) -> Result<TargetFunction, LoweringError> {
    if let Some(edge) = function
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::Jump {
                psi_edge,
                residual_affine_discards,
                ..
            } if !residual_affine_discards.is_empty() => Some(*psi_edge),
            _ => None,
        })
        && !super::unit::continuation::has_shape(function)
    {
        return Err(LoweringError::UnsupportedPartialAffineContinuation {
            machine: function.machine,
            edge,
        });
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
    // Fresh aggregates belong to the ordinary graph. The legacy structural
    // route only forwards existing input carriers and cannot construct a result.
    if super::control_flow::requires_graph(function, structural_types)? {
        return super::control_flow::lower(
            function,
            target,
            functions,
            structural_types,
            boundary_machines,
            settlements,
            installed_calls,
            scalar_abis,
            native_callbacks,
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
            scalar_abis,
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
