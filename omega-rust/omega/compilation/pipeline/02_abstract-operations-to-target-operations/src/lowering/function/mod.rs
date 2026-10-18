//! Optimizer module role: executable entrance. Every function lowers through its explicit control graph.

use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId};
use std::collections::BTreeMap;
use target::NativeTarget;
use target_operations::{BoundarySettlementBinding, ScalarFunctionAbi, TargetFunction};

mod native_boundaries;

pub(super) fn lower_function(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    borrowed_reference_sources: super::control_flow::references::BorrowedSources,
    stored_reference_effects: super::control_flow::references::StoredEffectCatalog,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    structural_types: &StructuralTypeLookup<'_>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    selected_calls: &crate::lowering::selected_boundary_calls::SelectedCallsByCall,
    native_callbacks: &BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
) -> Result<TargetFunction, LoweringError> {
    if let Some((operation, boundary)) =
        native_boundaries::unsupported_scalar_call(function, settlements, selected_calls)
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
        dynamic_dispatch,
        borrowed_reference_sources,
        stored_reference_effects,
        structural_types,
        boundary_machines,
        settlements,
        selected_calls,
        scalar_abis,
        native_callbacks,
    )
}
