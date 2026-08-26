use crate::{
    BoundNominalCallbackPlacement, CallbackPlacementBindingIdentity, CallbackThunkPlan,
    callback_placement_binding_identity, canonical_callback_private_symbol,
    replay_callback_root_schedule, validate_bound_nominal_callback_placement,
};
use omega_calling_conventions::{
    CallbackRequirementId, NativePlace, PlanDiagnostic, StaticMachineBinderId, ValuePlacement,
};
use omega_control_flow::MachineFunctionIdentity;
use omega_platform_interface::HostCallPlan;
use psi_arena::Handle;
use std::sync::Arc;

/// Address-free demand to place one emitted private callback function into the
/// exact outbound registrar binder destination selected during checking.
///
/// This carrier owns no target operation, physical offset, bytes, object
/// relocation, runtime storage, native address, registration authority, or
/// lease. Those require later joins that this row cannot authorize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackPrivateRelocationDemand {
    pub placement_index: usize,
    pub placement_identity: CallbackPlacementBindingIdentity,
    pub binder: StaticMachineBinderId,
    pub destination: NativePlace,
    pub requirement: CallbackRequirementId,
    pub function_identity: MachineFunctionIdentity,
    pub private_symbol: Arc<str>,
}

/// Address-free join from one exact private relocation demand to the
/// registrar host-call occurrence and native argument that own its destination
/// root.
///
/// The complete `NativePlace` remains inside `demand`. In particular a field
/// destination retains its nominal layout and ordered slot path, never a byte
/// offset. This row grants no target-operation, object-relocation, runtime, or
/// registration authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackRegistrarArgumentBinding {
    pub demand_index: usize,
    pub demand: CallbackPrivateRelocationDemand,
    pub host_call: Handle<omega_abstract_operations::AbstractHostCallOccurrence>,
    pub native_argument: Handle<omega_abstract_operations::AbstractHostCallNativeArgument>,
}

/// Target-closed ABI-relative destination for one exact callback registrar
/// argument binding.
///
/// A field row retains the complete authoritative layout-demand snapshot; its
/// physical offset is evidence produced by native layout closure, never a new
/// identity or an authored coordinate. This carrier owns no selected
/// operation, object symbol, relocation, bytes, runtime address, registration
/// authority, or lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackRegistrarPhysicalDestination {
    pub binding_index: usize,
    pub binding: CallbackRegistrarArgumentBinding,
    pub formal_ordinal: u32,
    pub parameter_placement: ValuePlacement,
    pub kind: CallbackRegistrarPhysicalDestinationKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackRegistrarPhysicalDestinationKind {
    /// Synthetic/internal coverage only until the authored direct-parameter
    /// declaration is settled by OWNER_QUESTIONS Q13.
    Parameter,
    Field {
        layout_demand_index: usize,
        layout_demand: omega_layout::TargetClosedPrivateCallbackDemand,
    },
}

/// Independently replay one address-free demand against its exact checked
/// placement and emitted thunk/root schedule.
pub fn replay_callback_private_relocation_demand(
    demand: &CallbackPrivateRelocationDemand,
    placement_index: usize,
    placement: &BoundNominalCallbackPlacement,
    thunk: &CallbackThunkPlan,
) -> Result<(), PlanDiagnostic> {
    validate_bound_nominal_callback_placement(placement)?;
    let Some(materialization) = &placement.private_materialization else {
        return Err(PlanDiagnostic(
            "callback private relocation demand names a placement without a target-closed materialization"
                .into(),
        ));
    };
    let expected_identity = callback_placement_binding_identity(placement);
    if demand.placement_index != placement_index
        || demand.placement_identity != expected_identity
        || demand.binder != materialization.binder
        || demand.destination != materialization.destination
        || demand.requirement != materialization.requirement
    {
        return Err(PlanDiagnostic(
            "callback private relocation demand drifted from its exact placement row".into(),
        ));
    }
    if thunk.placement_index != placement_index
        || thunk.placement_identity != expected_identity
        || demand.function_identity != thunk.function_identity
        || demand.private_symbol != thunk.private_symbol
        || demand.private_symbol != canonical_callback_private_symbol(placement)
    {
        return Err(PlanDiagnostic(
            "callback private relocation demand drifted from its exact thunk identity".into(),
        ));
    }
    replay_callback_root_schedule(&thunk.root_schedule, placement)?;
    if thunk.root_schedule.placement_index() != placement_index
        || thunk.root_schedule.placement_identity() != &expected_identity
        || thunk.root_schedule.function_identity() != demand.function_identity
        || thunk.root_schedule.private_symbol() != &demand.private_symbol
    {
        return Err(PlanDiagnostic(
            "callback private relocation demand drifted from its exact root schedule".into(),
        ));
    }
    Ok(())
}

/// Replay the complete ordered address-free demand catalog. Each retained
/// private materialization must have exactly one same-index thunk and demand;
/// non-materializing placements must have no demand.
pub fn replay_callback_private_relocation_demands(
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
) -> Result<(), PlanDiagnostic> {
    if thunks.len() != placements.len() {
        return Err(PlanDiagnostic(format!(
            "callback private relocation replay requires one thunk per placement, but retained {} placements and {} thunks",
            placements.len(),
            thunks.len()
        )));
    }
    let expected_count = placements
        .iter()
        .filter(|placement| placement.private_materialization.is_some())
        .count();
    if demands.len() != expected_count {
        return Err(PlanDiagnostic(format!(
            "callback private relocation replay requires {expected_count} exact demands, but retained {}",
            demands.len()
        )));
    }

    let mut next_demand = 0usize;
    for (placement_index, placement) in placements.iter().enumerate() {
        let matching_thunks = thunks
            .iter()
            .filter(|thunk| thunk.placement_index == placement_index)
            .collect::<Vec<_>>();
        let [thunk] = matching_thunks.as_slice() else {
            return Err(PlanDiagnostic(format!(
                "callback placement {placement_index} resolves to {} thunks; exactly one is required",
                matching_thunks.len()
            )));
        };
        if placement.private_materialization.is_none() {
            if demands
                .iter()
                .any(|demand| demand.placement_index == placement_index)
            {
                return Err(PlanDiagnostic(format!(
                    "callback placement {placement_index} has a private relocation demand without a target-closed materialization"
                )));
            }
            continue;
        }
        let demand = demands.get(next_demand).ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback placement {placement_index} is missing its ordered private relocation demand"
            ))
        })?;
        if demand.placement_index != placement_index {
            return Err(PlanDiagnostic(format!(
                "callback private relocation demand order drifted at placement {placement_index}"
            )));
        }
        replay_callback_private_relocation_demand(demand, placement_index, placement, thunk)?;
        next_demand += 1;
    }
    if next_demand != demands.len() {
        return Err(PlanDiagnostic(
            "callback private relocation replay retained an unowned trailing demand".into(),
        ));
    }
    Ok(())
}

/// Independently replay the complete ordered registrar-argument binding
/// catalog against its placement/thunk demands, source host calls, and exact
/// abstract occurrence rows.
pub fn replay_callback_registrar_argument_bindings(
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
    host_calls: &HostCallPlan,
    boundaries: &omega_abstract_operations::AbstractBoundarySummary,
    bindings: &[CallbackRegistrarArgumentBinding],
) -> Result<(), PlanDiagnostic> {
    replay_callback_private_relocation_demands(placements, thunks, demands)?;
    if bindings.len() != demands.len() {
        return Err(PlanDiagnostic(format!(
            "callback registrar argument replay requires {} exact bindings, but retained {}",
            demands.len(),
            bindings.len()
        )));
    }

    for (demand_index, demand) in demands.iter().enumerate() {
        let binding = bindings.get(demand_index).ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback private relocation demand {demand_index} is missing its registrar argument binding"
            ))
        })?;
        if binding.demand_index != demand_index || binding.demand != *demand {
            return Err(PlanDiagnostic(format!(
                "callback registrar argument binding drifted from demand {demand_index}"
            )));
        }

        let matching_occurrences = boundaries
            .host_calls
            .iter()
            .filter(|(_, occurrence)| {
                abstract_site_matches_checked(
                    occurrence.source_site,
                    demand.placement_identity.site,
                ) && occurrence.registration_operation
                    == demand.placement_identity.registration_operation
            })
            .collect::<Vec<_>>();
        let [(occurrence_handle, occurrence)] = matching_occurrences.as_slice() else {
            return Err(PlanDiagnostic(format!(
                "callback private relocation demand {demand_index} resolves to {} exact registrar occurrences; exactly one is required",
                matching_occurrences.len()
            )));
        };
        if binding.host_call != *occurrence_handle {
            return Err(PlanDiagnostic(format!(
                "callback registrar argument binding {demand_index} changed its occurrence handle"
            )));
        }

        let matching_source_calls = host_calls
            .calls
            .iter()
            .filter(|(handle, _)| {
                handle.arena_index() == occurrence.source_call_index
                    && handle.generation() == occurrence.source_call_generation
            })
            .collect::<Vec<_>>();
        let [(_, source_call)] = matching_source_calls.as_slice() else {
            return Err(PlanDiagnostic(format!(
                "callback registrar occurrence {demand_index} resolves to {} source host calls; exactly one is required",
                matching_source_calls.len()
            )));
        };
        if source_call.source_site != Some(demand.placement_identity.site)
            || source_call.registration_operation
                != demand.placement_identity.registration_operation
            || source_call.requirement_identity.is_empty()
            || occurrence.requirement_identity != source_call.requirement_identity
            || occurrence.source_key != source_call.source_key
            || occurrence.statement_index != source_call.statement_index
            || occurrence.call_ordinal != source_call.call_ordinal
            || occurrence.lowering_index != source_call.lowering.arena_index()
            || occurrence.lowering_generation != source_call.lowering.generation()
        {
            return Err(PlanDiagnostic(format!(
                "callback registrar occurrence {demand_index} drifted from its exact source host call target, overload, lowering, or coordinates"
            )));
        }

        let source_arguments = host_calls.arguments.span(source_call.arguments).ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar source host call {demand_index} retained an invalid argument span"
            ))
        })?;
        let abstract_arguments = boundaries
            .host_call_arguments
            .span(occurrence.arguments)
            .ok_or_else(|| {
                PlanDiagnostic(format!(
                    "callback registrar occurrence {demand_index} retained an invalid native-argument span"
                ))
            })?;
        let source_formals = source_arguments
            .iter()
            .enumerate()
            .filter_map(|(index, argument)| argument.formal.map(|formal| (index, formal)))
            .collect::<Vec<_>>();
        if source_formals.len() != abstract_arguments.len() {
            return Err(PlanDiagnostic(format!(
                "callback registrar occurrence {demand_index} changed native-argument cardinality"
            )));
        }
        for ((source_index, formal), abstract_argument) in
            source_formals.iter().zip(abstract_arguments)
        {
            let expected_source_index = usize::try_from(formal.formal_ordinal)
                .map_err(|_| {
                    PlanDiagnostic(format!(
                        "callback registrar occurrence {demand_index} retained an unrepresentable formal ordinal"
                    ))
                })?
                .checked_add(usize::from(source_call.has_result))
                .ok_or_else(|| {
                    PlanDiagnostic(format!(
                        "callback registrar occurrence {demand_index} formal position overflowed"
                    ))
                })?;
            if *source_index != expected_source_index
                || abstract_argument.formal_ordinal != formal.formal_ordinal
                || abstract_argument.native_parameter != Some(formal.native_parameter)
                || formal.native_parameter
                    != omega_calling_conventions::callback_native_parameter_id(
                        &source_call.requirement_identity,
                        formal.formal_ordinal,
                    )
            {
                return Err(PlanDiagnostic(format!(
                    "callback registrar occurrence {demand_index} changed native-argument order or overload-derived identity"
                )));
            }
        }

        let root_parameter = native_place_root_parameter(&demand.destination)?;
        let matching_arguments = abstract_arguments
            .iter()
            .enumerate()
            .filter(|(_, argument)| argument.native_parameter == Some(root_parameter))
            .collect::<Vec<_>>();
        let [(argument_offset, _)] = matching_arguments.as_slice() else {
            return Err(PlanDiagnostic(format!(
                "callback private relocation demand {demand_index} resolves to {} registrar native arguments; exactly one is required",
                matching_arguments.len()
            )));
        };
        let expected_argument =
            span_handle(occurrence.arguments, *argument_offset).ok_or_else(|| {
                PlanDiagnostic(format!(
                    "callback registrar occurrence {demand_index} native-argument handle overflowed"
                ))
            })?;
        if binding.native_argument != expected_argument {
            return Err(PlanDiagnostic(format!(
                "callback registrar argument binding {demand_index} changed its exact native-argument handle"
            )));
        }
    }

    Ok(())
}

/// Independently replay the complete ABI-relative callback destination
/// catalog against the exact registrar binding, outbound `CallPlan`, and
/// authoritative target-closed layout rows.
pub fn replay_callback_registrar_physical_destinations(
    target: omega_target::NativeTarget,
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
    host_calls: &HostCallPlan,
    boundaries: &omega_abstract_operations::AbstractBoundarySummary,
    bindings: &[CallbackRegistrarArgumentBinding],
    layouts: &omega_layout::LayoutPlan,
    destinations: &[CallbackRegistrarPhysicalDestination],
) -> Result<(), PlanDiagnostic> {
    replay_callback_registrar_argument_bindings(
        placements, thunks, demands, host_calls, boundaries, bindings,
    )?;
    if destinations.len() != bindings.len() {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical-destination replay requires {} exact rows, but retained {}",
            bindings.len(),
            destinations.len()
        )));
    }
    let pointer_size = u16::try_from(target.pointer_size)
        .map_err(|_| PlanDiagnostic("callback registrar target pointer size exceeds u16".into()))?;
    let pointer_alignment = u16::try_from(target.pointer_alignment).map_err(|_| {
        PlanDiagnostic("callback registrar target pointer alignment exceeds u16".into())
    })?;

    for (binding_index, binding) in bindings.iter().enumerate() {
        let destination = destinations.get(binding_index).ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar argument binding {binding_index} is missing its physical destination"
            ))
        })?;
        if destination.binding_index != binding_index || destination.binding != *binding {
            return Err(PlanDiagnostic(format!(
                "callback registrar physical destination drifted from binding {binding_index}"
            )));
        }
        let (_, native_argument) = boundaries
            .host_call_arguments
            .iter()
            .find(|(handle, _)| *handle == binding.native_argument)
            .ok_or_else(|| {
                PlanDiagnostic(format!(
                    "callback registrar physical destination {binding_index} lost its exact native argument"
                ))
            })?;
        if destination.formal_ordinal != native_argument.formal_ordinal
            || native_argument.native_parameter
                != Some(native_place_root_parameter(&binding.demand.destination)?)
        {
            return Err(PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} changed its formal or native-parameter identity"
            )));
        }
        let placement = placements
            .get(binding.demand.placement_index)
            .ok_or_else(|| {
                PlanDiagnostic(format!(
                    "callback registrar physical destination {binding_index} names a missing placement"
                ))
            })?;
        let materialization = placement.private_materialization.as_ref().ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} lost its outbound materialization"
            ))
        })?;
        if materialization
            .registrar_boundary_entry_plan
            .call
            .policy
            .architecture()
            != target.architecture
        {
            return Err(PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} retained an outbound ABI policy for a different target architecture"
            )));
        }
        let parameter_index = usize::try_from(destination.formal_ordinal).map_err(|_| {
            PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} formal ordinal is unrepresentable"
            ))
        })?;
        let expected_placement = materialization
            .registrar_boundary_entry_plan
            .call
            .parameters
            .get(parameter_index)
            .ok_or_else(|| {
                PlanDiagnostic(format!(
                    "callback registrar physical destination {binding_index} formal ordinal has no outbound ABI placement"
                ))
            })?;
        if destination.parameter_placement != *expected_placement
            || destination.parameter_placement.shape.byte_size != pointer_size
            || destination.parameter_placement.shape.alignment != pointer_alignment
            || destination.parameter_placement.locations.is_empty()
        {
            return Err(PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} changed its exact pointer-sized outbound ABI placement"
            )));
        }

        match (&binding.demand.destination, &destination.kind) {
            (NativePlace::Parameter(_), CallbackRegistrarPhysicalDestinationKind::Parameter) => {}
            (
                NativePlace::Field {
                    layout, field_path, ..
                },
                CallbackRegistrarPhysicalDestinationKind::Field {
                    layout_demand_index,
                    layout_demand,
                },
            ) => {
                let [slot] = field_path.as_slice() else {
                    return Err(PlanDiagnostic(format!(
                        "callback registrar physical destination {binding_index} requires one exact target-closed field slot; multi-segment paths remain an engineering gap"
                    )));
                };
                let expected_layout_demand = layouts
                    .private_callback_demands
                    .get(*layout_demand_index)
                    .ok_or_else(|| {
                        PlanDiagnostic(format!(
                            "callback registrar physical destination {binding_index} retained an invalid layout-demand index"
                        ))
                    })?;
                if layout_demand != expected_layout_demand
                    || layout_demand.layout != *layout
                    || layout_demand.slot != *slot
                    || layout_demand.requirement != binding.demand.requirement
                {
                    return Err(PlanDiagnostic(format!(
                        "callback registrar physical destination {binding_index} drifted from its exact layout, slot, or requirement row"
                    )));
                }
                replay_physical_layout_geometry(target, layouts, binding_index, layout_demand)?;
            }
            _ => {
                return Err(PlanDiagnostic(format!(
                    "callback registrar physical destination {binding_index} changed its direct/field destination kind"
                )));
            }
        }
    }
    Ok(())
}

fn replay_physical_layout_geometry(
    target: omega_target::NativeTarget,
    layouts: &omega_layout::LayoutPlan,
    binding_index: usize,
    layout_demand: &omega_layout::TargetClosedPrivateCallbackDemand,
) -> Result<(), PlanDiagnostic> {
    if !layout_demand.data_symbol.is_valid()
        || layout_demand.byte_size != target.pointer_size
        || layout_demand.alignment != target.pointer_alignment
        || layout_demand.alignment == 0
        || !layout_demand.offset.is_multiple_of(layout_demand.alignment)
    {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} retained invalid target pointer geometry"
        )));
    }
    let end = layout_demand
        .offset
        .checked_add(layout_demand.byte_size)
        .ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} field extent overflowed"
            ))
        })?;
    let matching_data_layouts = layouts
        .data_layouts
        .iter()
        .filter(|(_, layout)| layout.symbol == layout_demand.data_symbol)
        .collect::<Vec<_>>();
    let [(_, data_layout)] = matching_data_layouts.as_slice() else {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} resolves to {} data layouts; exactly one is required",
            matching_data_layouts.len()
        )));
    };
    if end > data_layout.layout.size
        || data_layout.layout.alignment < layout_demand.alignment
        || !data_layout
            .layout
            .alignment
            .is_multiple_of(layout_demand.alignment)
    {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} field range is outside or misaligned for its exact data layout"
        )));
    }
    Ok(())
}

fn abstract_site_matches_checked(
    abstract_site: omega_abstract_operations::AbstractHostCallSourceSite,
    checked_site: psi_checked_trees::NominalMachineUseSite,
) -> bool {
    matches!(
        (abstract_site, checked_site),
        (
            omega_abstract_operations::AbstractHostCallSourceSite::Statement(left),
            psi_checked_trees::NominalMachineUseSite::Statement(right)
        ) if left == right
    ) || matches!(
        (abstract_site, checked_site),
        (
            omega_abstract_operations::AbstractHostCallSourceSite::Expression(left),
            psi_checked_trees::NominalMachineUseSite::Expression(right)
        ) if left == right
    )
}

fn native_place_root_parameter(
    destination: &NativePlace,
) -> Result<omega_calling_conventions::NativeParameterId, PlanDiagnostic> {
    match destination {
        NativePlace::Parameter(parameter) => Ok(*parameter),
        NativePlace::Field {
            parameter,
            field_path,
            ..
        } if !field_path.is_empty() => Ok(*parameter),
        NativePlace::Field { .. } => Err(PlanDiagnostic(
            "callback registrar field destination retained an empty nominal slot path".into(),
        )),
    }
}

fn span_handle<T>(span: psi_arena::HandleSpan<T>, offset: usize) -> Option<Handle<T>> {
    let offset = u32::try_from(offset).ok()?;
    if offset >= span.count() {
        return None;
    }
    Some(Handle::from_parts(
        span.start().arena_index().checked_add(offset)?,
        span.start().generation(),
    ))
}
