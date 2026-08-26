use omega_calling_conventions::BoundaryEntryPlan;
use omega_control_flow::StateKey;
use psi_checked_trees::NominalMachineUseSite;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

/// Target-owned callback recipe joined to one admitted nominal machine use.
///
/// The checked program owns the semantic admission and only retains the
/// evaluated plan fingerprint. This row carries the exact validated plan past
/// orchestration so thunk lowering never has to rediscover ABI placement from
/// names, types, or a convention oracle.
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
    pub boundary_calling_plan_fingerprint: u64,
    pub boundary_entry_plan: BoundaryEntryPlan,
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
    pub boundary_calling_plan_fingerprint: u64,
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
    pub function_identity: omega_control_flow::MachineFunctionIdentity,
    pub private_symbol: Arc<str>,
    /// Sole address-free activation and ABI bridge owned by this thunk.
    pub root_schedule: Arc<crate::CallbackRootSchedule>,
}

/// Fingerprint the exact ordered checked-placement receipts carried by callback
/// thunks for final-footprint evidence.
///
/// This is evidence rather than authority: final emission still compares each
/// structural receipt with its placement row before this summary can be used.
pub fn callback_thunk_placement_identity_fingerprint(thunks: &[CallbackThunkPlan]) -> u64 {
    let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
    fingerprint_into(&mut fingerprint, b"omega.callback-placement-identity.v1");
    fingerprint_into(&mut fingerprint, &(thunks.len() as u64).to_le_bytes());
    for thunk in thunks {
        fingerprint_into(
            &mut fingerprint,
            &(thunk.placement_index as u64).to_le_bytes(),
        );
        let identity = &thunk.placement_identity;
        let (site_tag, site_index, site_generation) = match identity.site {
            NominalMachineUseSite::Statement(handle) => {
                (1u8, handle.arena_index(), handle.generation())
            }
            NominalMachineUseSite::Expression(handle) => {
                (2u8, handle.arena_index(), handle.generation())
            }
        };
        fingerprint_into(&mut fingerprint, &[site_tag]);
        fingerprint_into(&mut fingerprint, &u64::from(site_index).to_le_bytes());
        fingerprint_into(&mut fingerprint, &u64::from(site_generation).to_le_bytes());
        fingerprint_symbol(&mut fingerprint, identity.registration_operation);
        fingerprint_into(
            &mut fingerprint,
            &u64::from(identity.static_machine_ordinal).to_le_bytes(),
        );
        fingerprint_symbol(&mut fingerprint, identity.selected_machine);
        fingerprint_symbol(&mut fingerprint, identity.selected_entry);
        fingerprint_symbol(&mut fingerprint, identity.satisfaction_trait);
        fingerprint_symbol(&mut fingerprint, identity.satisfaction_requirement);
        fingerprint_into(
            &mut fingerprint,
            &(identity.canonical_requirement_overload.len() as u64).to_le_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            identity.canonical_requirement_overload.as_bytes(),
        );
        fingerprint_into(
            &mut fingerprint,
            &identity.boundary_calling_plan_fingerprint.to_le_bytes(),
        );
    }
    fingerprint
}

fn fingerprint_symbol(fingerprint: &mut u64, symbol: SymbolHandle) {
    fingerprint_into(fingerprint, &u64::from(symbol.arena_index()).to_le_bytes());
    fingerprint_into(fingerprint, &u64::from(symbol.generation()).to_le_bytes());
}

fn fingerprint_into(fingerprint: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *fingerprint ^= u64::from(*byte);
        *fingerprint = fingerprint.wrapping_mul(0x0000_0100_0000_01b3);
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
        boundary_calling_plan_fingerprint: placement.boundary_calling_plan_fingerprint,
    }
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
    omega_calling_conventions::ValidatedBoundaryEntryPlan,
    omega_calling_conventions::PlanDiagnostic,
> {
    let signature = omega_calling_conventions::CallSignature {
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
    let validated = omega_calling_conventions::validate_boundary_entry_plan(
        placement.boundary_entry_plan.clone(),
        &signature,
    )?;
    if validated.plan() != &placement.boundary_entry_plan {
        return Err(omega_calling_conventions::PlanDiagnostic(
            "callback placement retained a noncanonical boundary entry plan".to_owned(),
        ));
    }
    if placement.boundary_calling_plan_fingerprint == 0
        || validated.contract_fingerprint() != placement.boundary_calling_plan_fingerprint
    {
        return Err(omega_calling_conventions::PlanDiagnostic(
            "callback placement boundary entry plan drifted from its retained fingerprint"
                .to_owned(),
        ));
    }
    Ok(validated)
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
        placement.boundary_calling_plan_fingerprint,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{CallSignature, CallingPolicy};

    fn placement() -> BoundNominalCallbackPlacement {
        let validated = omega_calling_conventions::evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature::default(),
        )
        .expect("empty callback entry plan");
        BoundNominalCallbackPlacement {
            site: NominalMachineUseSite::Expression(
                psi_checked_trees::expression::ExpressionHandle::from_parts(9, 1),
            ),
            registration_operation: SymbolHandle::from_arena_index(3),
            static_machine_ordinal: 7,
            selected_machine: SymbolHandle::from_parts(4, 2),
            selected_entry: SymbolHandle::from_parts(5, 3),
            satisfaction_trait: SymbolHandle::from_arena_index(6),
            satisfaction_requirement: SymbolHandle::from_arena_index(8),
            canonical_requirement_overload: "Handler::call".to_owned(),
            boundary_calling_plan_fingerprint: validated.contract_fingerprint(),
            boundary_entry_plan: validated.plan().clone(),
        }
    }

    #[test]
    fn canonical_private_symbol_binds_site_selected_handles_ordinal_and_plan() {
        let baseline = placement();
        let baseline_symbol = canonical_callback_private_symbol(&baseline);
        let mut drifts = Vec::new();

        let mut site = baseline.clone();
        site.site = NominalMachineUseSite::Expression(
            psi_checked_trees::expression::ExpressionHandle::from_parts(9, 4),
        );
        drifts.push(site);
        let mut ordinal = baseline.clone();
        ordinal.static_machine_ordinal += 1;
        drifts.push(ordinal);
        let mut machine = baseline.clone();
        machine.selected_machine = SymbolHandle::from_parts(4, 5);
        drifts.push(machine);
        let mut entry = baseline.clone();
        entry.selected_entry = SymbolHandle::from_parts(5, 6);
        drifts.push(entry);
        let mut fingerprint = baseline.clone();
        fingerprint.boundary_calling_plan_fingerprint ^= 1;
        drifts.push(fingerprint);

        assert!(baseline_symbol.starts_with("__omega_callback_e"));
        for drifted in drifts {
            assert_ne!(canonical_callback_private_symbol(&drifted), baseline_symbol);
        }
    }

    #[test]
    fn callback_placement_replay_rejects_plan_or_fingerprint_drift() {
        let baseline = placement();
        validate_bound_nominal_callback_placement(&baseline)
            .expect("exact retained callback plan should replay");

        let mut plan_drift = baseline.clone();
        plan_drift.boundary_entry_plan.state.preemption =
            omega_calling_conventions::Preemption::ProviderDefined;
        let error = validate_bound_nominal_callback_placement(&plan_drift)
            .expect_err("changed callback plan must not retain the old fingerprint");
        assert!(error.0.contains("drifted from its retained fingerprint"));

        let mut fingerprint_drift = baseline;
        fingerprint_drift.boundary_calling_plan_fingerprint ^= 1;
        let error = validate_bound_nominal_callback_placement(&fingerprint_drift)
            .expect_err("changed callback fingerprint must not retain the old plan");
        assert!(error.0.contains("drifted from its retained fingerprint"));
    }

    #[test]
    fn callback_placement_binding_identity_binds_registration_and_satisfaction_row() {
        let baseline = placement();
        let identity = callback_placement_binding_identity(&baseline);

        let mut registration_drift = baseline.clone();
        registration_drift.registration_operation = SymbolHandle::from_parts(3, 2);
        assert_ne!(
            callback_placement_binding_identity(&registration_drift),
            identity
        );

        let mut satisfaction_drift = baseline.clone();
        satisfaction_drift.satisfaction_requirement = SymbolHandle::from_parts(8, 2);
        assert_ne!(
            callback_placement_binding_identity(&satisfaction_drift),
            identity
        );

        let mut overload_drift = baseline;
        overload_drift.canonical_requirement_overload = "Handler::other".to_owned();
        assert_ne!(
            callback_placement_binding_identity(&overload_drift),
            identity
        );
    }

    #[test]
    fn callback_thunk_placement_fingerprint_binds_exact_ordered_receipts() {
        let baseline = placement();
        let entry_key = StateKey {
            machine: baseline.selected_machine,
            state: baseline.selected_entry,
            segment_index: 0,
        };
        let function_identity =
            omega_control_flow::MachineFunctionIdentity::callback_thunk(entry_key, 0)
                .expect("callback identity");
        let private_symbol = canonical_callback_private_symbol(&baseline);
        let root_schedule = Arc::new(
            crate::plan_callback_root_schedule(
                0,
                &baseline,
                entry_key,
                function_identity,
                Arc::clone(&private_symbol),
            )
            .expect("callback root schedule"),
        );
        let thunk = CallbackThunkPlan {
            placement_index: 0,
            placement_identity: callback_placement_binding_identity(&baseline),
            entry_key,
            function_identity,
            private_symbol,
            root_schedule,
        };
        let fingerprint =
            callback_thunk_placement_identity_fingerprint(std::slice::from_ref(&thunk));

        let mut drifted = thunk.clone();
        drifted.placement_identity.satisfaction_requirement = SymbolHandle::from_parts(8, 2);
        assert_ne!(
            callback_thunk_placement_identity_fingerprint(&[drifted]),
            fingerprint
        );

        let mut reindexed = thunk;
        reindexed.placement_index = 1;
        assert_ne!(
            callback_thunk_placement_identity_fingerprint(&[reindexed]),
            fingerprint
        );
    }
}
