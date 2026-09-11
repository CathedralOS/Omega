//! Optimizer module role: executable entrance. Every function lowers through its explicit control graph.

use super::shared::*;

mod native_boundaries;

pub(super) fn lower_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    structural_types: &StructuralTypeLookup<'_>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
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
    super::control_flow::lower(
        function,
        target,
        functions,
        structural_types,
        boundary_machines,
        settlements,
        installed_calls,
        scalar_abis,
        native_callbacks,
    )
}
