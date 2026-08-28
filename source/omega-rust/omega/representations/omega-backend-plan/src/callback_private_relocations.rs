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
    NestedField {
        path_demand_index: usize,
        path_demand: omega_layout::TargetClosedTwoHopPrivateCallbackPath,
    },
}

/// Exact selected/assigned operand binding for one callback registrar
/// destination.
///
/// This row ends at assigned-operation identity. It owns no object symbol,
/// relocation, emitted bytes, runtime address, registration authority, or
/// callback lease.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackRegistrarAssignedOperandBinding {
    pub destination_index: usize,
    pub destination: CallbackRegistrarPhysicalDestination,
    pub abstract_instruction: Handle<omega_abstract_operations::AbstractOperation>,
    pub target_instruction: Handle<omega_target_operations::TargetOperation>,
    pub assigned_instruction: Handle<omega_assigned_target_operations::AssignedOperation>,
    pub abstract_provenance: omega_abstract_operations::AbstractHostOperationProvenance,
    pub provenance: omega_target_operations::TargetHostOperationProvenance,
    pub formal_operand: omega_target_operations::TargetHostFormalOperandBinding,
    pub target_operand: omega_target_operations::TargetInstructionOperand,
    pub assigned_operand: Handle<omega_assigned_target_operations::AssignedInstructionOperand>,
}

/// Object-relative request to store one exact private callback address into a
/// registrar argument's runtime-storage field.
///
/// Both object symbols and all three store handles are exact retained identity
/// evidence. This row deliberately owns no relocation record or kind, encoded
/// byte site, resolved address, runtime execution, registration authority,
/// callback lease, or publication authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackPrivateObjectStoreRequest {
    pub assigned_binding_index: usize,
    pub assigned_binding: CallbackRegistrarAssignedOperandBinding,
    pub storage_region: omega_target_operations::RuntimeStorageRegion,
    pub storage_base_offset: usize,
    pub slot_offset: usize,
    pub destination_offset: usize,
    pub byte_size: usize,
    pub alignment: usize,
    pub storage_symbol: omega_object_file::ObjectSymbolHandle,
    pub storage_symbol_plan: omega_object_file::SymbolPlan,
    pub function_identity: MachineFunctionIdentity,
    pub function_symbol: omega_object_file::ObjectSymbolHandle,
    pub function_symbol_plan: omega_object_file::SymbolPlan,
    pub abstract_store_instruction: Handle<omega_abstract_operations::AbstractOperation>,
    pub target_store_instruction: Handle<omega_target_operations::TargetOperation>,
    pub assigned_store_instruction: Handle<omega_assigned_target_operations::AssignedOperation>,
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
            (
                NativePlace::Field {
                    layout, field_path, ..
                },
                CallbackRegistrarPhysicalDestinationKind::NestedField {
                    path_demand_index,
                    path_demand,
                },
            ) => {
                let [field_slot, terminal_slot] = field_path.as_slice() else {
                    return Err(PlanDiagnostic(format!(
                        "callback registrar physical destination {binding_index} nested field requires exactly two ordered slots"
                    )));
                };
                let expected = layouts
                    .two_hop_private_callback_paths
                    .get(*path_demand_index)
                    .ok_or_else(|| {
                        PlanDiagnostic(format!(
                            "callback registrar physical destination {binding_index} retained an invalid two-hop path index"
                        ))
                    })?;
                let exact_path_count = layouts
                    .two_hop_private_callback_paths
                    .iter()
                    .filter(|candidate| {
                        candidate.root_layout.layout == *layout
                            && candidate.field_slot == *field_slot
                            && candidate.terminal_demand.slot == *terminal_slot
                    })
                    .count();
                if exact_path_count != 1
                    || path_demand != expected
                    || path_demand.root_layout.layout != *layout
                    || path_demand.field_slot != *field_slot
                    || path_demand.terminal_demand.slot != *terminal_slot
                    || path_demand.terminal_demand.requirement != binding.demand.requirement
                {
                    return Err(PlanDiagnostic(format!(
                        "callback registrar physical destination {binding_index} drifted from its exact two-hop layout path"
                    )));
                }
                replay_two_hop_physical_layout_geometry(
                    target,
                    layouts,
                    binding_index,
                    path_demand,
                )?;
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

/// Independently replay exact callback registrar destination bindings through
/// abstract, target, and assigned operation/operand identity.
#[allow(clippy::too_many_arguments)]
pub fn replay_callback_registrar_assigned_operand_bindings(
    target: omega_target::NativeTarget,
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
    host_calls: &HostCallPlan,
    boundaries: &omega_abstract_operations::AbstractBoundarySummary,
    argument_bindings: &[CallbackRegistrarArgumentBinding],
    layouts: &omega_layout::LayoutPlan,
    destinations: &[CallbackRegistrarPhysicalDestination],
    abstract_operations: &omega_abstract_operations::AbstractOperationPlan,
    target_operations: &omega_target_operations::TargetOperationPlan,
    assigned_operations: &omega_assigned_target_operations::AssignedTargetOperationPlan,
    bindings: &[CallbackRegistrarAssignedOperandBinding],
) -> Result<(), PlanDiagnostic> {
    replay_callback_registrar_physical_destinations(
        target,
        placements,
        thunks,
        demands,
        host_calls,
        boundaries,
        argument_bindings,
        layouts,
        destinations,
    )?;
    if target_operations.target != target
        || assigned_operations.target != target
        || bindings.len() != destinations.len()
    {
        return Err(PlanDiagnostic(
            "callback registrar assigned-operand catalog target or cardinality drifted".into(),
        ));
    }

    for (destination_index, destination) in destinations.iter().enumerate() {
        let binding = bindings.get(destination_index).ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar destination {destination_index} is missing its assigned operand binding"
            ))
        })?;
        if binding.destination_index != destination_index || binding.destination != *destination {
            return Err(PlanDiagnostic(format!(
                "callback registrar assigned operand {destination_index} drifted from its physical destination"
            )));
        }
        let abstract_instruction = abstract_operations
            .code
            .instructions
            .iter()
            .find(|(handle, _)| *handle == binding.abstract_instruction)
            .map(|(_, instruction)| instruction)
            .ok_or_else(|| assigned_operand_error(destination_index, "abstract instruction"))?;
        let target_instruction = target_operations
            .code
            .instructions
            .iter()
            .find(|(handle, _)| *handle == binding.target_instruction)
            .map(|(_, instruction)| instruction)
            .ok_or_else(|| assigned_operand_error(destination_index, "target instruction"))?;
        let assigned_instruction = assigned_operations
            .code
            .instructions
            .iter()
            .find(|(handle, _)| *handle == binding.assigned_instruction)
            .map(|(_, instruction)| instruction)
            .ok_or_else(|| assigned_operand_error(destination_index, "assigned instruction"))?;
        if binding.abstract_instruction.arena_index() != binding.target_instruction.arena_index()
            || binding.abstract_instruction.generation() != binding.target_instruction.generation()
            || binding.target_instruction != binding.assigned_instruction
            || target_instruction != assigned_instruction
        {
            return Err(PlanDiagnostic(format!(
                "callback registrar assigned operand {destination_index} changed its 1:1 instruction identity"
            )));
        }
        let omega_abstract_operations::AbstractOperationKind::HostOperation {
            provenance: Some(abstract_provenance),
            operation_ordinal,
            operands: abstract_operands,
        } = &abstract_instruction.kind
        else {
            return Err(assigned_operand_error(
                destination_index,
                "opted-in abstract host operation",
            ));
        };
        let omega_target_operations::TargetOperationKind::HostOperation {
            operation_key,
            operands: target_operands,
            provenance: Some(target_provenance),
        } = &target_instruction.kind
        else {
            return Err(assigned_operand_error(
                destination_index,
                "opted-in target host operation",
            ));
        };
        if !matches!(
            operation_key.capability,
            omega_calling_conventions::HostCapability::Unknown
                | omega_calling_conventions::HostCapability::Custom(_)
        ) || abstract_provenance.operation_ordinal != *operation_ordinal
            || binding.abstract_provenance != *abstract_provenance
            || binding.provenance != *target_provenance
            || target_provenance.occurrence != destination.binding.host_call
        {
            return Err(PlanDiagnostic(format!(
                "callback registrar assigned operand {destination_index} changed its exact outbound operation provenance"
            )));
        }
        let matching_formals = target_provenance
            .formal_operands
            .iter()
            .filter(|formal| {
                formal.native_argument == destination.binding.native_argument
                    && formal.formal_ordinal == destination.formal_ordinal
            })
            .collect::<Vec<_>>();
        let [formal] = matching_formals.as_slice() else {
            return Err(PlanDiagnostic(format!(
                "callback registrar assigned operand {destination_index} resolves to {} exact formal operands; one is required",
                matching_formals.len()
            )));
        };
        if binding.formal_operand != **formal
            || !handle_in_span(formal.abstract_operand, *abstract_operands)
            || !handle_in_span(formal.operand, *target_operands)
        {
            return Err(PlanDiagnostic(format!(
                "callback registrar assigned operand {destination_index} changed its exact formal or operand handle"
            )));
        }
        let abstract_operand = abstract_operations
            .code
            .operands
            .iter()
            .find(|(handle, _)| *handle == formal.abstract_operand)
            .map(|(_, operand)| operand)
            .ok_or_else(|| assigned_operand_error(destination_index, "abstract operand"))?;
        let target_operand = target_operations
            .code
            .operands
            .iter()
            .find(|(handle, _)| *handle == formal.operand)
            .map(|(_, operand)| operand)
            .ok_or_else(|| assigned_operand_error(destination_index, "target operand"))?;
        let assigned_operand = assigned_operations
            .code
            .operands
            .iter()
            .find(|(handle, _)| *handle == binding.assigned_operand)
            .map(|(_, operand)| operand)
            .ok_or_else(|| assigned_operand_error(destination_index, "assigned operand"))?;
        if formal.abstract_operand.arena_index() != formal.operand.arena_index()
            || formal.abstract_operand.generation() != formal.operand.generation()
            || formal.operand != binding.assigned_operand
            || formal.abstract_operand_kind != abstract_operand.kind
            || binding.target_operand != *target_operand
            || target_operand.kind != assigned_operand.kind
        {
            return Err(PlanDiagnostic(format!(
                "callback registrar assigned operand {destination_index} changed abstract-to-target-to-assigned operand identity or shape"
            )));
        }
    }
    Ok(())
}

/// Independently replay the complete object-relative callback store-request
/// catalog, including its exact executable-store handles, without minting a
/// relocation, runtime registration, lease, or publication authority.
#[allow(clippy::too_many_arguments)]
pub fn replay_callback_private_object_store_requests(
    target: omega_target::NativeTarget,
    placements: &[BoundNominalCallbackPlacement],
    thunks: &[CallbackThunkPlan],
    demands: &[CallbackPrivateRelocationDemand],
    host_calls: &HostCallPlan,
    boundaries: &omega_abstract_operations::AbstractBoundarySummary,
    argument_bindings: &[CallbackRegistrarArgumentBinding],
    layouts: &omega_layout::LayoutPlan,
    destinations: &[CallbackRegistrarPhysicalDestination],
    abstract_operations: &omega_abstract_operations::AbstractOperationPlan,
    target_operations: &omega_target_operations::TargetOperationPlan,
    assigned_operations: &omega_assigned_target_operations::AssignedTargetOperationPlan,
    assigned_bindings: &[CallbackRegistrarAssignedOperandBinding],
    object: &omega_object_file::ObjectPlan,
    entry_machine_name: &str,
    requests: &[CallbackPrivateObjectStoreRequest],
) -> Result<(), PlanDiagnostic> {
    replay_callback_registrar_assigned_operand_bindings(
        target,
        placements,
        thunks,
        demands,
        host_calls,
        boundaries,
        argument_bindings,
        layouts,
        destinations,
        abstract_operations,
        target_operations,
        assigned_operations,
        assigned_bindings,
    )?;
    if object.target != target || requests.len() != assigned_bindings.len() {
        return Err(PlanDiagnostic(
            "callback private object-store target or cardinality drifted".into(),
        ));
    }

    for (binding_index, binding) in assigned_bindings.iter().enumerate() {
        let request = requests
            .get(binding_index)
            .ok_or_else(|| object_store_error(binding_index, "ordered object-store request"))?;
        if request.assigned_binding_index != binding_index || request.assigned_binding != *binding {
            return Err(object_store_error(
                binding_index,
                "assigned-operand binding snapshot",
            ));
        }
        let CallbackRegistrarPhysicalDestinationKind::Field { layout_demand, .. } =
            &binding.destination.kind
        else {
            return Err(object_store_error(
                binding_index,
                "one-slot field destination; direct parameters remain fenced",
            ));
        };
        let assigned_operand = assigned_operations
            .code
            .operands
            .iter()
            .find(|(handle, _)| *handle == binding.assigned_operand)
            .map(|(_, operand)| operand)
            .ok_or_else(|| object_store_error(binding_index, "assigned operand"))?;
        let omega_target_operations::TargetInstructionOperandKind::RuntimeStorageAddress {
            region,
            byte_offset,
        } = assigned_operand.kind
        else {
            return Err(object_store_error(
                binding_index,
                "RuntimeStorageAddress operand; DataAddress remains fenced",
            ));
        };
        let destination_offset = byte_offset
            .checked_add(layout_demand.offset)
            .ok_or_else(|| object_store_error(binding_index, "nonoverflowing destination"))?;
        if request.storage_region != region
            || request.storage_base_offset != byte_offset
            || request.slot_offset != layout_demand.offset
            || request.destination_offset != destination_offset
            || request.byte_size != layout_demand.byte_size
            || request.alignment != layout_demand.alignment
            || request.alignment == 0
            || !request.destination_offset.is_multiple_of(request.alignment)
        {
            return Err(object_store_error(
                binding_index,
                "runtime-storage geometry",
            ));
        }

        let expected_storage = exact_runtime_storage_object_symbol(
            object,
            region,
            entry_machine_name,
            request.destination_offset,
            request.byte_size,
        )
        .ok_or_else(|| object_store_error(binding_index, "exact BSS storage symbol"))?;
        if request.storage_symbol != expected_storage.0
            || request.storage_symbol_plan != *expected_storage.1
        {
            return Err(object_store_error(
                binding_index,
                "BSS storage-symbol snapshot",
            ));
        }

        let function_identity = binding.destination.binding.demand.function_identity;
        let expected_function =
            omega_object_file::object_function_symbol(object, function_identity).ok_or_else(
                || object_store_error(binding_index, "exact callback function symbol"),
            )?;
        if request.function_identity != function_identity
            || request.function_symbol != expected_function.0
            || request.function_symbol_plan != *expected_function.1
        {
            return Err(object_store_error(
                binding_index,
                "callback function-symbol snapshot",
            ));
        }
        let group_count = assigned_bindings
            .iter()
            .filter(|candidate| candidate.assigned_instruction == binding.assigned_instruction)
            .count();
        let group_ordinal = assigned_bindings[..binding_index]
            .iter()
            .filter(|candidate| candidate.assigned_instruction == binding.assigned_instruction)
            .count();
        let store_index = usize::try_from(binding.assigned_instruction.arena_index())
            .ok()
            .and_then(|registrar| registrar.checked_sub(group_count))
            .and_then(|first| first.checked_add(group_ordinal))
            .ok_or_else(|| object_store_error(binding_index, "pre-registrar store position"))?;
        let store_index = u32::try_from(store_index)
            .map_err(|_| object_store_error(binding_index, "pre-registrar store position"))?;
        let abstract_store =
            Handle::from_parts(store_index, binding.abstract_instruction.generation());
        let target_store = Handle::from_parts(store_index, binding.target_instruction.generation());
        let assigned_store =
            Handle::from_parts(store_index, binding.assigned_instruction.generation());
        if !abstract_operations
            .code
            .instructions
            .is_valid(abstract_store)
            || !target_operations.code.instructions.is_valid(target_store)
            || !assigned_operations
                .code
                .instructions
                .is_valid(assigned_store)
        {
            return Err(object_store_error(
                binding_index,
                "valid contiguous pre-registrar address-store handles",
            ));
        }
        let abstract_store_row = abstract_operations.code.instructions.get(abstract_store);
        let target_store_row = target_operations.code.instructions.get(target_store);
        let assigned_store_row = assigned_operations.code.instructions.get(assigned_store);
        let abstract_registrar = abstract_operations
            .code
            .instructions
            .get(binding.abstract_instruction);
        let target_registrar = target_operations
            .code
            .instructions
            .get(binding.target_instruction);
        let assigned_registrar = assigned_operations
            .code
            .instructions
            .get(binding.assigned_instruction);
        let abstract_functions = abstract_operations
            .code
            .functions
            .iter()
            .filter(|(_, function)| {
                handle_in_span(abstract_store, function.instructions)
                    && handle_in_span(binding.abstract_instruction, function.instructions)
            })
            .map(|(_, function)| function)
            .collect::<Vec<_>>();
        let target_functions = target_operations
            .code
            .functions
            .iter()
            .filter(|(_, function)| {
                handle_in_span(target_store, function.instructions)
                    && handle_in_span(binding.target_instruction, function.instructions)
            })
            .map(|(_, function)| function)
            .collect::<Vec<_>>();
        let assigned_functions = assigned_operations
            .code
            .functions
            .iter()
            .filter(|(_, function)| {
                handle_in_span(assigned_store, function.instructions)
                    && handle_in_span(binding.assigned_instruction, function.instructions)
            })
            .map(|(_, function)| function)
            .collect::<Vec<_>>();
        let ([abstract_function], [target_function], [assigned_function]) = (
            abstract_functions.as_slice(),
            target_functions.as_slice(),
            assigned_functions.as_slice(),
        ) else {
            return Err(object_store_error(
                binding_index,
                "one exact store/registrar function span",
            ));
        };
        if request.abstract_store_instruction != abstract_store
            || request.target_store_instruction != target_store
            || request.assigned_store_instruction != assigned_store
            || abstract_function.identity != target_function.identity
            || target_function.identity != assigned_function.identity
            || abstract_function.symbol != target_function.symbol
            || target_function.symbol != assigned_function.symbol
            || abstract_store_row.source_key != abstract_registrar.source_key
            || abstract_store_row.source_statement != abstract_registrar.source_statement
            || target_store_row.source_key != target_registrar.source_key
            || target_store_row.source_statement != target_registrar.source_statement
            || assigned_store_row.source_key != assigned_registrar.source_key
            || assigned_store_row.source_statement != assigned_registrar.source_statement
            || abstract_store_row.source_key != target_store_row.source_key
            || abstract_store_row.source_statement != target_store_row.source_statement
            || target_store_row.source_key != assigned_store_row.source_key
            || target_store_row.source_statement != assigned_store_row.source_statement
            || !matches!(
                abstract_store_row.kind,
                omega_abstract_operations::AbstractOperationKind::WriteFunctionAddressToRuntimeStorage {
                    function,
                    target_region,
                    target_offset,
                } if function == request.function_identity
                    && target_region == request.storage_region
                    && target_offset == request.destination_offset
            )
            || !matches!(
                target_store_row.kind,
                omega_target_operations::TargetOperationKind::WriteFunctionAddressToRuntimeStorage {
                    function,
                    target_region,
                    target_offset,
                } if function == request.function_identity
                    && target_region == request.storage_region
                    && target_offset == request.destination_offset
            )
            || !matches!(
                assigned_store_row.kind,
                omega_assigned_target_operations::AssignedOperationKind::WriteFunctionAddressToRuntimeStorage {
                    function,
                    target_region,
                    target_offset,
                } if function == request.function_identity
                    && target_region == request.storage_region
                    && target_offset == request.destination_offset
            )
        {
            return Err(object_store_error(
                binding_index,
                "exact contiguous pre-registrar address-store operation",
            ));
        }
    }
    let store_count = assigned_operations
        .code
        .instructions
        .iter()
        .filter(|(_, instruction)| {
            matches!(
                instruction.kind,
                omega_assigned_target_operations::AssignedOperationKind::WriteFunctionAddressToRuntimeStorage { .. }
            )
        })
        .count();
    if store_count != requests.len() {
        return Err(PlanDiagnostic(
            "callback address-store operation cardinality drifted".into(),
        ));
    }
    Ok(())
}

fn exact_runtime_storage_object_symbol<'object>(
    object: &'object omega_object_file::ObjectPlan,
    region: omega_target_operations::RuntimeStorageRegion,
    entry_machine_name: &str,
    destination_offset: usize,
    byte_size: usize,
) -> Option<(
    omega_object_file::ObjectSymbolHandle,
    &'object omega_object_file::SymbolPlan,
)> {
    let name = omega_object_file::storage_region_symbol_name(region, entry_machine_name);
    let mut matches = object.layout.symbols.iter().filter(|(_, symbol)| {
        symbol.name == name
            && symbol.kind == omega_object_file::SymbolKind::Object
            && symbol.section
                == omega_object_file::SymbolSection::Section(omega_object_file::SectionKind::Bss)
    });
    let (handle, symbol) = matches.next()?;
    if matches.next().is_some()
        || destination_offset
            .checked_add(byte_size)
            .is_none_or(|end| end > symbol.size)
    {
        return None;
    }
    Some((handle, symbol))
}

fn object_store_error(index: usize, identity: &str) -> PlanDiagnostic {
    PlanDiagnostic(format!(
        "callback private object store {index} lost its {identity}"
    ))
}

fn handle_in_span<T>(handle: Handle<T>, span: psi_arena::HandleSpan<T>) -> bool {
    !span.is_empty()
        && handle.generation() == span.start().generation()
        && handle.arena_index() >= span.start().arena_index()
        && handle.arena_index() < span.start().arena_index().saturating_add(span.count())
}

fn assigned_operand_error(index: usize, identity: &str) -> PlanDiagnostic {
    PlanDiagnostic(format!(
        "callback registrar assigned operand {index} lost its exact {identity}"
    ))
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

fn replay_two_hop_physical_layout_geometry(
    target: omega_target::NativeTarget,
    layouts: &omega_layout::LayoutPlan,
    binding_index: usize,
    path: &omega_layout::TargetClosedTwoHopPrivateCallbackPath,
) -> Result<(), PlanDiagnostic> {
    let root = layouts
        .plan_laid_layout_identities
        .get(path.root_layout_index)
        .ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} lost its root layout identity"
            ))
        })?;
    let child = layouts
        .plan_laid_layout_identities
        .get(path.child_layout_index)
        .ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} lost its child layout identity"
            ))
        })?;
    let terminal = layouts
        .private_callback_demands
        .get(path.terminal_demand_index)
        .ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} lost its terminal layout demand"
            ))
        })?;
    let root_symbol_count = layouts
        .plan_laid_layout_identities
        .iter()
        .filter(|identity| identity.data_symbol == root.data_symbol)
        .count();
    let root_layout_count = layouts
        .plan_laid_layout_identities
        .iter()
        .filter(|identity| identity.layout == root.layout)
        .count();
    let child_symbol_count = layouts
        .plan_laid_layout_identities
        .iter()
        .filter(|identity| identity.data_symbol == child.data_symbol)
        .count();
    let child_layout_count = layouts
        .plan_laid_layout_identities
        .iter()
        .filter(|identity| identity.layout == child.layout)
        .count();
    let terminal_count = layouts
        .private_callback_demands
        .iter()
        .filter(|demand| demand.data_symbol == terminal.data_symbol && demand.slot == terminal.slot)
        .count();
    let field_edge_changed = layouts
        .two_hop_private_callback_paths
        .iter()
        .filter(|candidate| {
            candidate.root_layout.layout == path.root_layout.layout
                && candidate.field_slot == path.field_slot
        })
        .any(|candidate| {
            candidate.field != path.field
                || candidate.field_symbol != path.field_symbol
                || candidate.field_layout != path.field_layout
                || candidate.child_layout != path.child_layout
        });
    if root_symbol_count != 1
        || root_layout_count != 1
        || child_symbol_count != 1
        || child_layout_count != 1
        || terminal_count != 1
        || path.root_layout_index == path.child_layout_index
        || root.data_symbol == child.data_symbol
        || field_edge_changed
    {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} lost unique root, child, or terminal path identity"
        )));
    }
    let matching_roots = layouts
        .data_layouts
        .iter()
        .filter(|(_, layout)| layout.symbol == root.data_symbol)
        .collect::<Vec<_>>();
    let [(_, root_data)] = matching_roots.as_slice() else {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} root resolves to {} exact data layouts",
            matching_roots.len()
        )));
    };
    let matching_children = layouts
        .data_layouts
        .iter()
        .filter(|(_, layout)| layout.symbol == child.data_symbol)
        .collect::<Vec<_>>();
    let [(_, child_data)] = matching_children.as_slice() else {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} child resolves to {} exact data layouts",
            matching_children.len()
        )));
    };
    let omega_layout::DataShape::Record {
        fields: root_fields,
    } = root_data.shape
    else {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} root is not an exact record layout"
        )));
    };
    let root_field_end = root_fields
        .start()
        .arena_index()
        .checked_add(root_fields.count());
    if !matches!(child_data.shape, omega_layout::DataShape::Record { .. })
        || root_data.layout != root.physical
        || child_data.layout != child.physical
        || layouts.fields.span(root_fields).is_none()
        || path.field.generation() != root_fields.start().generation()
        || path.field.arena_index() < root_fields.start().arena_index()
        || root_field_end.is_none_or(|end| path.field.arena_index() >= end)
    {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} changed its root/child data-layout edge"
        )));
    }
    if !layouts.fields.is_valid(path.field) {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} lost its exact field layout"
        )));
    }
    let field = layouts.fields.get(path.field);
    let expected_field_identity =
        omega_calling_conventions::callback_layout_field_slot_id(root.layout, &path.field_identity);
    let composed = path
        .field_relative_offset
        .checked_add(terminal.offset)
        .ok_or_else(|| {
            PlanDiagnostic(format!(
                "callback registrar physical destination {binding_index} two-hop offset overflowed"
            ))
        })?;
    if root != &path.root_layout
        || child != &path.child_layout
        || terminal != &path.terminal_demand
        || field != &path.field_layout
        || field.symbol != path.field_symbol
        || path.field_slot != expected_field_identity
        || field.offset != path.field_relative_offset
        || field.layout.size != path.field_extent
        || field.layout.alignment != path.field_alignment
        || (field.type_symbol.is_valid() && field.type_symbol != child.data_symbol)
        || !matches!(field.type_descriptor, omega_layout::TypeLayoutDescriptor::Named { symbol, .. } if symbol == child.data_symbol)
        || field.layout != child.physical
        || terminal.data_symbol != child.data_symbol
        || terminal.byte_size != target.pointer_size
        || terminal.alignment != target.pointer_alignment
        || terminal.alignment == 0
        || !terminal.offset.is_multiple_of(terminal.alignment)
        || terminal
            .offset
            .checked_add(terminal.byte_size)
            .is_none_or(|end| end > child.physical.size)
        || composed != path.composed_offset
        || path.field_alignment == 0
        || !path
            .field_relative_offset
            .is_multiple_of(path.field_alignment)
        || path
            .field_relative_offset
            .checked_add(path.field_extent)
            .is_none_or(|end| end > root.physical.size)
        || !path.composed_offset.is_multiple_of(terminal.alignment)
        || path
            .composed_offset
            .checked_add(terminal.byte_size)
            .is_none_or(|end| end > root.physical.size)
    {
        return Err(PlanDiagnostic(format!(
            "callback registrar physical destination {binding_index} retained invalid two-hop layout geometry"
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
