//! Optimizer module role: executable entrance. Every function lowers through its explicit control graph.

use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use installation_evidence::InstalledProviderCallEvidence;
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use std::collections::BTreeMap;
use target::NativeTarget;
use target_operations::{BoundarySettlementBinding, ScalarFunctionAbi, TargetFunction};

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
