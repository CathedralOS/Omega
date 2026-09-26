//! The placement row, its 101-byte private symbol, and the fingerprint that
//! separates placements the symbol alone cannot tell apart.

use abstract_operations_to_target_operations::calling_conventions::{
    BoundaryEntryPlan, CallbackMaterializationContext, CallbackRequirementId,
    NativeParameterApplication, NativePlace, StaticMachineBinderId, ValueClass, ValueShape,
};
use abstract_operations_to_target_operations::function_identity::StateKey;
use std::sync::Arc;
use symbols::SymbolHandle;
use typed_trees_to_checked_trees::checked_trees::NominalMachineUseSite;

/// Target-owned callback recipe joined to one admitted nominal machine use.
///
/// The checked program owns the semantic admission and retains a compact
/// report coordinate. This row carries the exact validated plan past
/// orchestration so thunk lowering never has to rediscover ABI placement from
/// names, types, or a convention oracle. Authority never rests on the compact
/// coordinate alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundNominalCallbackPlacement {
    pub site: NominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
    /// Non-authoritative compatibility/report coordinate beside the exact
    /// `boundary_entry_plan` below.
    pub boundary_calling_plan_report_fingerprint: u64,
    /// Exact checked per-entry resource anchor selected by semantic callback
    /// admission. It remains a derivation receipt only; later resource
    /// realizations must independently join it before admission.
    pub resource_receipt:
        typed_trees_to_checked_trees::checked_trees::CheckedCallbackResourceReceipt,
    pub boundary_entry_plan: BoundaryEntryPlan,
    /// Exact target-closed outbound binder/destination row selected by this
    /// nominal callback use. `None` retains the established non-materializing
    /// callback path; a plan containing private rows may not use that form.
    pub private_materialization: Option<BoundCallbackPrivateMaterialization>,
}

/// Complete address-free context and selected row for one private callback
/// materialization. Physical offsets, target operations, bytes, object
/// relocations, runtime storage, registration authority, and leases are absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCallbackPrivateMaterialization {
    pub binder: StaticMachineBinderId,
    pub destination: NativePlace,
    pub requirement: CallbackRequirementId,
    /// Exact target-owned registrar plan whose outbound callback row owns the
    /// binder-to-destination mapping. This is distinct from the callback
    /// handler's inbound `boundary_entry_plan` retained by the placement.
    pub registrar_boundary_entry_plan: BoundaryEntryPlan,
    /// Non-authoritative compatibility/report coordinate beside the exact
    /// registrar plan above.
    pub registrar_calling_plan_report_fingerprint: u64,
    /// Versioned application identity over the exact requirement, ordered
    /// nominal native telescope, callback rows, placements, and physical plan.
    pub registrar_application_report_fingerprint: u64,
    pub registrar_application_commitment: [u8; 32],
    /// Exact target-closed registrar telescope row selected by a direct
    /// parameter destination. Field destinations retain `None`: their root
    /// semantic parameter is not itself a compiler-private callback argument.
    pub direct_registrar_parameter_application: Option<NativeParameterApplication>,
    pub context: CallbackMaterializationContext,
}

/// Exact checked identity retained when a callback placement becomes a thunk.
///
/// The thunk continues to join the placement recipe by index rather than
/// cloning it, but final emission must be able to distinguish replacement of
/// the registration operation, satisfaction row, or canonical overload after
/// backend planning. Those identities are deliberately not encoded into the
/// compiler-private linkage name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackPlacementBindingIdentity {
    pub site: NominalMachineUseSite,
    pub registration_operation: SymbolHandle,
    pub static_machine_ordinal: u32,
    pub selected_machine: SymbolHandle,
    pub selected_entry: SymbolHandle,
    pub satisfaction_trait: SymbolHandle,
    pub satisfaction_requirement: SymbolHandle,
    pub canonical_requirement_overload: String,
    /// Non-authoritative compatibility/report coordinate. Exact thunk replay
    /// compares `boundary_entry_plan` structurally.
    pub boundary_calling_plan_report_fingerprint: u64,
    pub boundary_entry_plan: BoundaryEntryPlan,
    pub resource_receipt:
        typed_trees_to_checked_trees::checked_trees::CheckedCallbackResourceReceipt,
    pub private_materialization: Option<BoundCallbackPrivateMaterialization>,
}

/// One private inbound function that later target lowering must emit.
///
/// `placement_index` joins back to the exact validated placement row without
/// cloning it. `function_identity` is the distinct native-function role bound
/// to that row and selected entry; it cannot impersonate the source entry it
/// adapts. The symbol is compiler-private planned object identity, never an
/// Omega value or a source-level address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackThunkPlan {
    pub placement_index: usize,
    pub placement_identity: CallbackPlacementBindingIdentity,
    pub entry_key: StateKey,
    pub function_identity:
        abstract_operations_to_target_operations::function_identity::MachineFunctionIdentity,
    pub private_symbol: Arc<str>,
    /// Sole address-free activation and ABI bridge owned by this thunk.
    pub root_schedule: Arc<crate::backend_plan::CallbackRootSchedule>,
}

/// Compactly fingerprint the exact ordered checked-placement receipts carried
/// by callback thunks for final-footprint reporting.
///
/// This compact summary is not authority: final emission still compares each
/// structural receipt, including its exact boundary plan, with its placement
/// row before the summary can be used.
pub fn callback_thunk_placement_identity_report_fingerprint(thunks: &[CallbackThunkPlan]) -> u64 {
    let mut report_fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(
        &mut report_fingerprint,
        b"omega.callback-placement-identity.v2",
    );
    fingerprint_into(
        &mut report_fingerprint,
        &(thunks.len() as u64).to_le_bytes(),
    );
    for thunk in thunks {
        fingerprint_placement_identity(
            &mut report_fingerprint,
            thunk.placement_index,
            &thunk.placement_identity,
        );
    }
    report_fingerprint
}

fn fingerprint_placement_identity(
    report_fingerprint: &mut u64,
    placement_index: usize,
    identity: &CallbackPlacementBindingIdentity,
) {
    fingerprint_into(report_fingerprint, &(placement_index as u64).to_le_bytes());
    let (site_tag, site_index, site_generation) = match identity.site {
        NominalMachineUseSite::Statement(handle) => {
            (1u8, handle.arena_index(), handle.generation())
        }
        NominalMachineUseSite::Expression(handle) => {
            (2u8, handle.arena_index(), handle.generation())
        }
    };
    fingerprint_into(report_fingerprint, &[site_tag]);
    fingerprint_into(report_fingerprint, &u64::from(site_index).to_le_bytes());
    fingerprint_into(
        report_fingerprint,
        &u64::from(site_generation).to_le_bytes(),
    );
    fingerprint_symbol(report_fingerprint, identity.registration_operation);
    fingerprint_into(
        report_fingerprint,
        &u64::from(identity.static_machine_ordinal).to_le_bytes(),
    );
    fingerprint_symbol(report_fingerprint, identity.selected_machine);
    fingerprint_symbol(report_fingerprint, identity.selected_entry);
    fingerprint_symbol(report_fingerprint, identity.satisfaction_trait);
    fingerprint_symbol(report_fingerprint, identity.satisfaction_requirement);
    fingerprint_into(
        report_fingerprint,
        &(identity.canonical_requirement_overload.len() as u64).to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        identity.canonical_requirement_overload.as_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &identity
            .boundary_calling_plan_report_fingerprint
            .to_le_bytes(),
    );
    fingerprint_callback_resource_receipt(report_fingerprint, identity.resource_receipt);
    fingerprint_private_materialization(
        report_fingerprint,
        identity.private_materialization.as_ref(),
    );
}

fn fingerprint_callback_resource_receipt(
    report_fingerprint: &mut u64,
    receipt: typed_trees_to_checked_trees::checked_trees::CheckedCallbackResourceReceipt,
) {
    fingerprint_symbol(report_fingerprint, receipt.machine());
    fingerprint_symbol(report_fingerprint, receipt.entry());
    fingerprint_into(
        report_fingerprint,
        &receipt.contract_report_fingerprint().to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &receipt.contract_commitment().as_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &receipt.stack_report_fingerprint().to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &receipt
            .logical_structural_work_report_fingerprint()
            .to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &receipt.machine_state_report_fingerprint().to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &receipt.envelope_report_fingerprint().to_le_bytes(),
    );
}

fn fingerprint_private_materialization(
    report_fingerprint: &mut u64,
    materialization: Option<&BoundCallbackPrivateMaterialization>,
) {
    let Some(materialization) = materialization else {
        fingerprint_into(report_fingerprint, &[0]);
        return;
    };
    fingerprint_into(report_fingerprint, &[1]);
    fingerprint_into(
        report_fingerprint,
        &materialization.binder.get().to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &materialization.requirement.get().to_le_bytes(),
    );
    fingerprint_native_place(report_fingerprint, &materialization.destination);
    fingerprint_into(
        report_fingerprint,
        &materialization
            .registrar_calling_plan_report_fingerprint
            .to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &materialization
            .registrar_application_report_fingerprint
            .to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &materialization.registrar_application_commitment,
    );
    fingerprint_into(
        report_fingerprint,
        &[u8::from(
            materialization
                .direct_registrar_parameter_application
                .is_some(),
        )],
    );
    if let Some(application) = &materialization.direct_registrar_parameter_application {
        fingerprint_into(
            report_fingerprint,
            &application.parameter.get().to_le_bytes(),
        );
        fingerprint_into(
            report_fingerprint,
            &application.native_ordinal.to_le_bytes(),
        );
        fingerprint_value_shape(report_fingerprint, application.shape);
    }
    fingerprint_into(
        report_fingerprint,
        &(materialization.context.binders.len() as u64).to_le_bytes(),
    );
    for binder in &materialization.context.binders {
        fingerprint_into(report_fingerprint, &binder.binder.get().to_le_bytes());
        fingerprint_into(report_fingerprint, &binder.requirement.get().to_le_bytes());
    }
    fingerprint_into(
        report_fingerprint,
        &(materialization.context.demands.len() as u64).to_le_bytes(),
    );
    for demand in &materialization.context.demands {
        fingerprint_native_place(report_fingerprint, &demand.destination);
        fingerprint_into(report_fingerprint, &demand.requirement.get().to_le_bytes());
    }
}

fn fingerprint_value_shape(report_fingerprint: &mut u64, shape: ValueShape) {
    match shape.class {
        ValueClass::Integer => fingerprint_into(report_fingerprint, &[1]),
        ValueClass::Float => fingerprint_into(report_fingerprint, &[2]),
        ValueClass::BorrowedReference => fingerprint_into(report_fingerprint, &[5]),
        ValueClass::HomogeneousFloatAggregate { members } => {
            fingerprint_into(report_fingerprint, &[3, members]);
        }
        ValueClass::SystemVAggregate { first, second } => {
            fingerprint_into(report_fingerprint, &[4, first as u8, second as u8]);
        }
    }
    fingerprint_into(report_fingerprint, &shape.byte_size.to_le_bytes());
    fingerprint_into(report_fingerprint, &shape.alignment.to_le_bytes());
}

fn fingerprint_native_place(report_fingerprint: &mut u64, place: &NativePlace) {
    match place {
        NativePlace::Parameter(parameter) => {
            fingerprint_into(report_fingerprint, &[1]);
            fingerprint_into(report_fingerprint, &parameter.get().to_le_bytes());
        }
        NativePlace::Field {
            parameter,
            layout,
            field_path,
        } => {
            fingerprint_into(report_fingerprint, &[2]);
            fingerprint_into(report_fingerprint, &parameter.get().to_le_bytes());
            fingerprint_into(report_fingerprint, &layout.get().to_le_bytes());
            fingerprint_into(report_fingerprint, &(field_path.len() as u64).to_le_bytes());
            for slot in field_path {
                fingerprint_into(report_fingerprint, &slot.get().to_le_bytes());
            }
        }
    }
}

fn fingerprint_symbol(report_fingerprint: &mut u64, symbol: SymbolHandle) {
    fingerprint_into(
        report_fingerprint,
        &u64::from(symbol.arena_index()).to_le_bytes(),
    );
    fingerprint_into(
        report_fingerprint,
        &u64::from(symbol.generation()).to_le_bytes(),
    );
}

fn fingerprint_into(report_fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *report_fingerprint ^= u64::from(*byte);
        *report_fingerprint = report_fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
    }
}

/// Retain the exact checked identities that authorize one callback placement.
pub fn callback_placement_binding_identity(
    placement: &BoundNominalCallbackPlacement,
) -> CallbackPlacementBindingIdentity {
    CallbackPlacementBindingIdentity {
        site: placement.site,
        registration_operation: placement.registration_operation,
        static_machine_ordinal: placement.static_machine_ordinal,
        selected_machine: placement.selected_machine,
        selected_entry: placement.selected_entry,
        satisfaction_trait: placement.satisfaction_trait,
        satisfaction_requirement: placement.satisfaction_requirement,
        canonical_requirement_overload: placement.canonical_requirement_overload.clone(),
        boundary_calling_plan_report_fingerprint: placement
            .boundary_calling_plan_report_fingerprint,
        boundary_entry_plan: placement.boundary_entry_plan.clone(),
        resource_receipt: placement.resource_receipt,
        private_materialization: placement.private_materialization.clone(),
    }
}

/// Derive the sole compiler-private function identity for one retained
/// callback placement.
///
/// The placement index remains an exact join into the ordered placement
/// roster. The continuation is always the selected machine/entry pair's
/// canonical segment zero; callers may not supply either coordinate
/// independently.
pub fn canonical_callback_thunk_identity(
    placement_index: usize,
    placement: &BoundNominalCallbackPlacement,
) -> Option<abstract_operations_to_target_operations::function_identity::MachineFunctionIdentity> {
    abstract_operations_to_target_operations::function_identity::MachineFunctionIdentity::callback_thunk(
        StateKey {
            machine: placement.selected_machine,
            state: placement.selected_entry,
            segment_index: 0,
        },
        placement_index,
    )
}

/// Independently replay the exact evaluated calling plan retained by one
/// callback placement.
///
/// The source-to-checked join already validates this pair, but backend planning
/// and final emission are separate consumers. Reconstructing the signature
/// from the retained placements and revalidating the complete entry plan keeps
/// either consumer from trusting a copied fingerprint or a noncanonical plan.
pub fn validate_bound_nominal_callback_placement(
    placement: &BoundNominalCallbackPlacement,
) -> Result<
    abstract_operations_to_target_operations::calling_conventions::ValidatedBoundaryEntryPlan,
    abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic,
> {
    placement.resource_receipt.validate().map_err(|error| {
        abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(format!(
            "callback placement retained an invalid checked resource receipt: {error}"
        ))
    })?;
    if placement.resource_receipt.machine() != placement.selected_machine
        || placement.resource_receipt.entry() != placement.selected_entry
    {
        return Err(
            abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                "callback placement resource receipt does not bind its selected machine entry"
                    .to_owned(),
            ),
        );
    }
    let signature = abstract_operations_to_target_operations::calling_conventions::CallSignature {
        parameters: placement
            .boundary_entry_plan
            .call
            .parameters
            .iter()
            .map(|parameter| parameter.shape)
            .collect(),
        result: placement
            .boundary_entry_plan
            .call
            .result
            .as_ref()
            .map(|result| result.shape),
    };
    let validated = abstract_operations_to_target_operations::calling_conventions::validate_boundary_entry_plan(
        placement.boundary_entry_plan.clone(),
        &signature,
    )?;
    if let Some(materialization) = &placement.private_materialization {
        let registrar_signature =
            abstract_operations_to_target_operations::calling_conventions::CallSignature {
                parameters: materialization
                    .registrar_boundary_entry_plan
                    .call
                    .parameters
                    .iter()
                    .map(|parameter| parameter.shape)
                    .collect(),
                result: materialization
                    .registrar_boundary_entry_plan
                    .call
                    .result
                    .as_ref()
                    .map(|result| result.shape),
            };
        let validated_registrar =
            abstract_operations_to_target_operations::calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                materialization.registrar_boundary_entry_plan.clone(),
                &registrar_signature,
                &materialization.context,
            )?;
        if validated_registrar.plan() != &materialization.registrar_boundary_entry_plan
            || materialization.registrar_calling_plan_report_fingerprint == 0
            || validated_registrar.contract_report_fingerprint()
                != materialization.registrar_calling_plan_report_fingerprint
        {
            return Err(abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                "callback placement private materialization drifted from its exact evaluated registrar plan"
                    .to_owned(),
            ));
        }
        let matching_binders = materialization
            .context
            .binders
            .iter()
            .filter(|candidate| candidate.binder == materialization.binder)
            .collect::<Vec<_>>();
        let matching_demands = materialization
            .context
            .demands
            .iter()
            .filter(|candidate| candidate.destination == materialization.destination)
            .collect::<Vec<_>>();
        let matching_rows = materialization
            .registrar_boundary_entry_plan
            .call
            .callback_materializations
            .iter()
            .filter(|candidate| candidate.binder == materialization.binder)
            .collect::<Vec<_>>();
        let ([binder], [demand], [row]) = (
            matching_binders.as_slice(),
            matching_demands.as_slice(),
            matching_rows.as_slice(),
        ) else {
            return Err(abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                    "callback placement private materialization lost its exact binder, demand, or plan row"
                        .to_owned(),
                ));
        };
        if binder.requirement != materialization.requirement
            || demand.requirement != materialization.requirement
            || row.destination != materialization.destination
        {
            return Err(abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                    "callback placement private materialization binder, requirement, or destination drifted"
                        .to_owned(),
                ));
        }
        validate_callback_registrar_native_parameters(materialization)?;
    }
    if validated.plan() != &placement.boundary_entry_plan {
        return Err(
            abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                "callback placement retained a noncanonical boundary entry plan".to_owned(),
            ),
        );
    }
    if placement.boundary_calling_plan_report_fingerprint == 0
        || validated.contract_report_fingerprint()
            != placement.boundary_calling_plan_report_fingerprint
    {
        return Err(
            abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                "callback placement boundary entry plan drifted from its retained fingerprint"
                    .to_owned(),
            ),
        );
    }
    Ok(validated)
}

fn validate_callback_registrar_native_parameters(
    materialization: &BoundCallbackPrivateMaterialization,
) -> Result<(), abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic> {
    let destination_parameter = match &materialization.destination {
        NativePlace::Parameter(parameter) => *parameter,
        NativePlace::Field { .. } => {
            if materialization
                .direct_registrar_parameter_application
                .is_some()
            {
                return Err(
                    abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                        "field callback destination retained a direct native parameter application"
                            .to_owned(),
                    ),
                );
            }
            return Ok(());
        }
    };
    let Some(application) = &materialization.direct_registrar_parameter_application else {
        return Err(
            abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                "direct callback destination lost its exact native parameter application"
                    .to_owned(),
            ),
        );
    };
    let Some(placement) = usize::try_from(application.native_ordinal)
        .ok()
        .and_then(|ordinal| {
            materialization
                .registrar_boundary_entry_plan
                .call
                .parameters
                .get(ordinal)
        })
    else {
        return Err(
            abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                "callback registrar native telescope ordinal is absent from its exact plan"
                    .to_owned(),
            ),
        );
    };
    if application.parameter != destination_parameter
        || application.shape != application.placement.shape
        || application.placement != *placement
    {
        return Err(
            abstract_operations_to_target_operations::calling_conventions::PlanDiagnostic(
                "callback registrar native telescope identity, shape, or placement drifted"
                    .to_owned(),
            ),
        );
    }
    Ok(())
}

/// Derive the one compiler-private object identity bound to an exact validated
/// callback placement. Planning and final emission both use this function so a
/// stored thunk symbol cannot drift from its source/selected identities or
/// evaluated boundary-plan fingerprint.
pub fn canonical_callback_private_symbol(placement: &BoundNominalCallbackPlacement) -> Arc<str> {
    let (site_kind, site_index, site_generation) = match placement.site {
        NominalMachineUseSite::Statement(handle) => {
            ('s', handle.arena_index(), handle.generation())
        }
        NominalMachineUseSite::Expression(handle) => {
            ('e', handle.arena_index(), handle.generation())
        }
    };
    Arc::from(format!(
        "__omega_callback_{site_kind}{site_index:08x}g{site_generation:08x}_a{:08x}_m{:08x}g{:08x}_e{:08x}g{:08x}_f{:016x}",
        placement.static_machine_ordinal,
        placement.selected_machine.arena_index(),
        placement.selected_machine.generation(),
        placement.selected_entry.arena_index(),
        placement.selected_entry.generation(),
        placement.boundary_calling_plan_report_fingerprint,
    ))
}

#[cfg(test)]
mod tests;
