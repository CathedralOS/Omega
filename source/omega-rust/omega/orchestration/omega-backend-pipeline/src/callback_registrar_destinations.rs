use omega_abstract_operations::AbstractBoundarySummary;
use omega_backend_plan::{
    BoundNominalCallbackPlacement, CallbackPrivateRelocationDemand,
    CallbackRegistrarArgumentBinding, CallbackRegistrarPhysicalDestination,
    CallbackRegistrarPhysicalDestinationKind, CallbackThunkPlan,
    replay_callback_registrar_physical_destinations,
};
use omega_calling_conventions::NativePlace;
use omega_layout::LayoutPlan;
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;
use psi_diagnostics::Diagnostic;
use std::sync::Arc;

pub(super) fn plan_callback_registrar_physical_destinations(
    target: NativeTarget,
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
    host_calls: &HostCallPlan,
    boundaries: &AbstractBoundarySummary,
    bindings: &[CallbackRegistrarArgumentBinding],
    layouts: &LayoutPlan,
) -> Result<Arc<[CallbackRegistrarPhysicalDestination]>, Diagnostic> {
    let mut destinations = Vec::with_capacity(bindings.len());
    for (binding_index, binding) in bindings.iter().enumerate() {
        let (_, native_argument) = boundaries
            .host_call_arguments
            .iter()
            .find(|(handle, _)| *handle == binding.native_argument)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "callback registrar argument binding {binding_index} lost its exact native argument"
                ))
            })?;
        let placement = placements
            .get(binding.demand.placement_index)
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "callback registrar argument binding {binding_index} names a missing placement"
                ))
            })?;
        let materialization = placement.private_materialization.as_ref().ok_or_else(|| {
            Diagnostic::error(format!(
                "callback registrar argument binding {binding_index} lost its outbound materialization"
            ))
        })?;
        let parameter_index = usize::try_from(native_argument.formal_ordinal).map_err(|_| {
            Diagnostic::error(format!(
                "callback registrar argument binding {binding_index} formal ordinal is unrepresentable"
            ))
        })?;
        let parameter_placement = materialization
            .registrar_boundary_entry_plan
            .call
            .parameters
            .get(parameter_index)
            .cloned()
            .ok_or_else(|| {
                Diagnostic::error(format!(
                    "callback registrar argument binding {binding_index} formal ordinal has no outbound ABI placement"
                ))
            })?;
        let kind = match &binding.demand.destination {
            NativePlace::Parameter(_) => CallbackRegistrarPhysicalDestinationKind::Parameter,
            NativePlace::Field {
                layout, field_path, ..
            } => {
                if let [field_slot, terminal_slot] = field_path.as_slice() {
                    let matching_paths = layouts
                        .two_hop_private_callback_paths
                        .iter()
                        .enumerate()
                        .filter(|(_, path)| {
                            path.root_layout.layout == *layout
                                && path.field_slot == *field_slot
                                && path.terminal_demand.slot == *terminal_slot
                                && path.terminal_demand.requirement == binding.demand.requirement
                        })
                        .collect::<Vec<_>>();
                    let [(path_demand_index, path_demand)] = matching_paths.as_slice() else {
                        return Err(Diagnostic::error(format!(
                            "callback registrar argument binding {binding_index} resolves to {} exact two-hop layout paths; exactly one is required",
                            matching_paths.len()
                        )));
                    };
                    CallbackRegistrarPhysicalDestinationKind::NestedField {
                        path_demand_index: *path_demand_index,
                        path_demand: (*path_demand).clone(),
                    }
                } else {
                    let [slot] = field_path.as_slice() else {
                        return Err(Diagnostic::error(format!(
                            "callback registrar argument binding {binding_index} requires one or two exact target-closed field slots"
                        )));
                    };
                    let matching_layout_demands = layouts
                        .private_callback_demands
                        .iter()
                        .enumerate()
                        .filter(|(_, candidate)| {
                            candidate.layout == *layout
                                && candidate.slot == *slot
                                && candidate.requirement == binding.demand.requirement
                        })
                        .collect::<Vec<_>>();
                    let [(layout_demand_index, layout_demand)] = matching_layout_demands.as_slice()
                    else {
                        return Err(Diagnostic::error(format!(
                            "callback registrar argument binding {binding_index} resolves to {} target-closed layout demands; exactly one is required",
                            matching_layout_demands.len()
                        )));
                    };
                    CallbackRegistrarPhysicalDestinationKind::Field {
                        layout_demand_index: *layout_demand_index,
                        layout_demand: (*layout_demand).clone(),
                    }
                }
            }
        };
        destinations.push(CallbackRegistrarPhysicalDestination {
            binding_index,
            binding: binding.clone(),
            formal_ordinal: native_argument.formal_ordinal,
            parameter_placement,
            kind,
        });
    }

    replay_callback_registrar_physical_destinations(
        target,
        placements,
        thunks,
        demands,
        host_calls,
        boundaries,
        bindings,
        layouts,
        &destinations,
    )
    .map_err(|error| {
        Diagnostic::error(format!(
            "callback registrar physical-destination replay failed: {error}"
        ))
    })?;
    Ok(Arc::from(destinations))
}

#[cfg(test)]
pub(crate) mod tests;
