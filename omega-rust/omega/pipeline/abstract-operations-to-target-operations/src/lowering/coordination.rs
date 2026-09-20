use super::function::lower_function;
use super::scalar_abi::{
    derive_fixed_scalar_function_abi, derive_mixed_structural_scalar_function_abi,
};
use super::shared::*;

mod ieee_float_fma_settlements;
mod installed_provider_calls;
pub(crate) mod native_callbacks;
mod projected_qualifications;

#[cfg(test)]
pub(crate) fn lower_to_target_operations_with_settlements(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_settlements_and_installation(
        plan,
        target,
        settlement_bindings,
        None,
        &[],
        &[],
    )
}

pub(super) fn lower_to_target_operations_with_settlements_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlement_bindings: &[BoundarySettlementBinding],
    installation: Option<&dyn ProviderInstallationEvidence>,
    ieee_float_fma: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
    native_callbacks: &[crate::AdmittedNativeCallbackArgument],
) -> Result<TargetOperationPlan, LoweringError> {
    projected_qualifications::reject_unsupported(plan)?;
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(LoweringError::EntryFunctionMissing(plan.entry));
    }
    let functions_by_machine = plan
        .functions
        .iter()
        .map(|function| (function.machine, function))
        .collect::<BTreeMap<_, _>>();
    let mut scalar_abis = BTreeMap::new();
    for function in &plan.functions {
        if let Some(abi) = derive_fixed_scalar_function_abi(function, target)? {
            scalar_abis.insert(function.machine, abi);
        }
    }
    let structural_types = StructuralTypeLookup::new(&plan.structural_types);
    let mut mixed_structural_scalar_abis = BTreeMap::new();
    for function in &plan.functions {
        if let Some(abi) =
            derive_mixed_structural_scalar_function_abi(function, target, &structural_types)?
        {
            mixed_structural_scalar_abis.insert(function.machine, abi);
        }
    }
    let boundary_machines = plan
        .boundary_machines
        .iter()
        .map(|boundary| (boundary.id, boundary))
        .collect::<BTreeMap<_, _>>();
    let mut settlements_by_boundary = BTreeMap::new();
    for binding in settlement_bindings {
        if settlements_by_boundary
            .insert(binding.boundary, binding.clone())
            .is_some()
        {
            return Err(LoweringError::DuplicateBoundarySettlement(binding.boundary));
        }
        if !plan
            .boundary_machines
            .iter()
            .any(|boundary| boundary.id == binding.boundary)
        {
            return Err(LoweringError::UnknownBoundarySettlement(binding.boundary));
        }
    }
    ieee_float_fma_settlements::validate_ieee_float_fma_settlements(plan, target, ieee_float_fma)?;
    let installed_by_call =
        installed_provider_calls::index_installed_provider_calls(plan, installation)?;
    let boundary_calls = installed_provider_calls::index_boundary_calls(plan);
    let native_callbacks_by_operation =
        native_callbacks::bind_native_callback_arguments(plan, target, native_callbacks)?;
    installed_provider_calls::validate_installed_provider_calls(
        plan,
        &installed_by_call,
        &boundary_calls,
    )?;
    let installed_boundaries = installed_by_call
        .keys()
        .map(|(_, _, boundary)| *boundary)
        .collect::<BTreeSet<_>>();
    if let Some(boundary) = settlements_by_boundary
        .keys()
        .find(|boundary| installed_boundaries.contains(boundary))
    {
        return Err(LoweringError::BoundarySettlementOverlapsInstalledProvider(
            *boundary,
        ));
    }
    if let Some((machine, operation, boundary)) = boundary_calls
        .keys()
        .find(|key| installed_boundaries.contains(&key.2) && !installed_by_call.contains_key(key))
        .copied()
    {
        return Err(LoweringError::PartialInstalledProviderBoundary {
            machine,
            operation,
            boundary,
        });
    }
    let required_settlements = boundary_calls
        .keys()
        .filter_map(|key| (!installed_by_call.contains_key(key)).then_some(key.2))
        .collect::<BTreeSet<_>>();
    for boundary in &required_settlements {
        if !settlements_by_boundary.contains_key(boundary) {
            return Err(LoweringError::MissingBoundarySettlement(*boundary));
        }
    }
    if let Some(extra) = settlements_by_boundary
        .keys()
        .find(|boundary| !required_settlements.contains(boundary))
    {
        return Err(LoweringError::UnusedBoundarySettlement(*extra));
    }
    let target_plan = TargetOperationPlan {
        psi: plan.psi,
        target,
        entry: plan.entry,
        native_callback_arguments: native_callbacks_by_operation.values().cloned().collect(),
        functions: plan
            .functions
            .iter()
            .map(|function| {
                let mut lowered = lower_function(
                    function,
                    target,
                    &functions_by_machine,
                    &scalar_abis,
                    &structural_types,
                    &boundary_machines,
                    &settlements_by_boundary,
                    &installed_by_call,
                    &native_callbacks_by_operation,
                )?;
                lowered.scalar_abi = scalar_abis.get(&function.machine).cloned();
                lowered.mixed_structural_scalar_abi =
                    mixed_structural_scalar_abis.get(&function.machine).cloned();
                Ok::<TargetFunction, LoweringError>(lowered)
            })
            .collect::<Result<Vec<_>, _>>()?,
    };
    native_callbacks::validate_native_callback_target_rows(
        &target_plan,
        &native_callbacks_by_operation,
    )?;
    Ok(target_plan)
}
