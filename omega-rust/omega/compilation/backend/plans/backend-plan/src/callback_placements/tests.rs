//! Callback placement tests.

use super::{
    Arc, BoundNominalCallbackPlacement, CallbackThunkPlan, NominalMachineUseSite, StateKey,
    SymbolHandle, callback_placement_binding_identity,
    callback_thunk_placement_identity_report_fingerprint, canonical_callback_private_symbol,
    validate_bound_nominal_callback_placement,
};
use calling_conventions::{
    CallSignature, CallbackBinderRequirement, CallbackMaterialization,
    CallbackMaterializationContext, CallbackRequirementId, CallingPolicy, LayoutPlanId,
    LayoutSlotId, NativeCallbackDemand, NativeParameterApplication, NativeParameterId, NativePlace,
    StaticMachineBinderId, ValueShape,
};

pub(crate) fn materialized_placement(destination: NativePlace) -> BoundNominalCallbackPlacement {
    let binder = StaticMachineBinderId::new(101).unwrap();
    let requirement = CallbackRequirementId::new(103).unwrap();
    let parameter = match &destination {
        NativePlace::Parameter(parameter) | NativePlace::Field { parameter, .. } => *parameter,
    };
    let context = CallbackMaterializationContext {
        binders: vec![CallbackBinderRequirement {
            binder,
            requirement,
        }],
        demands: vec![NativeCallbackDemand {
            destination: destination.clone(),
            requirement,
        }],
    };
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let mut registrar = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &signature,
    )
    .unwrap()
    .plan()
    .clone();
    registrar
        .call
        .callback_materializations
        .push(CallbackMaterialization {
            binder,
            destination: destination.clone(),
        });
    let validated =
        calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            registrar, &signature, &context,
        )
        .expect("registrar plan with exact callback context");
    let application = NativeParameterApplication {
        parameter,
        native_ordinal: 0,
        shape: signature.parameters[0],
        placement: validated.plan().call.parameters[0].clone(),
    };
    let (direct_application, root_application, field_byte_offset) = match destination {
        NativePlace::Parameter(_) => (Some(application), None, None),
        NativePlace::Field { .. } => (None, Some(application), Some(8)),
    };
    let mut placement = placement();
    placement.private_materialization = Some(super::BoundCallbackPrivateMaterialization {
        binder,
        destination,
        requirement,
        registrar_boundary_entry_plan: validated.plan().clone(),
        registrar_calling_plan_report_fingerprint: validated.contract_report_fingerprint(),
        registrar_application_report_fingerprint: 0xfeed,
        registrar_application_commitment: [0x33; 32],
        direct_registrar_parameter_application: direct_application,
        root_parameter_application: root_application,
        field_byte_offset,
        context,
    });
    placement
}

#[test]
fn callback_destinations_replay_their_exact_native_parameter_application() {
    let parameter = NativeParameterId::new(107).unwrap();
    for destination in [
        NativePlace::Parameter(parameter),
        NativePlace::Field {
            parameter,
            layout: LayoutPlanId::new(109).unwrap(),
            field_path: vec![LayoutSlotId::new(113).unwrap()],
        },
    ] {
        let baseline = materialized_placement(destination);
        validate_bound_nominal_callback_placement(&baseline).expect("exact callback placement");
        let materialization = baseline.private_materialization.as_ref().unwrap();
        let application = materialization.destination_parameter_application().unwrap();
        let mut wrong_parameter = application.clone();
        wrong_parameter.parameter = NativeParameterId::new(127).unwrap();
        let mut wrong_ordinal = application.clone();
        wrong_ordinal.native_ordinal = 1;
        let mut wrong_shape = application.clone();
        wrong_shape.shape = ValueShape::integer(4, 4);
        let mut wrong_placement = application.clone();
        wrong_placement.placement.locations.clear();
        for substituted in [wrong_parameter, wrong_ordinal, wrong_shape, wrong_placement] {
            let mut placement = baseline.clone();
            let materialization = placement.private_materialization.as_mut().unwrap();
            let application = match materialization.destination {
                NativePlace::Parameter(_) => {
                    &mut materialization.direct_registrar_parameter_application
                }
                NativePlace::Field { .. } => &mut materialization.root_parameter_application,
            };
            *application = Some(substituted);
            let error = validate_bound_nominal_callback_placement(&placement).expect_err(
                "both direct and field-root applications must join the registrar telescope",
            );
            assert!(error.0.contains("callback registrar native telescope"));
        }

        let mut missing_application = baseline.clone();
        let missing = missing_application
            .private_materialization
            .as_mut()
            .unwrap();
        missing.direct_registrar_parameter_application = None;
        missing.root_parameter_application = None;
        let mut mixed_applications = baseline.clone();
        let mixed = mixed_applications.private_materialization.as_mut().unwrap();
        mixed.direct_registrar_parameter_application = Some(application.clone());
        mixed.root_parameter_application = Some(application.clone());
        let mut wrong_offset_presence = baseline.clone();
        let wrong_offset = wrong_offset_presence
            .private_materialization
            .as_mut()
            .unwrap();
        wrong_offset.field_byte_offset = match wrong_offset.destination {
            NativePlace::Parameter(_) => Some(0),
            NativePlace::Field { .. } => None,
        };
        for placement in [
            missing_application,
            mixed_applications,
            wrong_offset_presence,
        ] {
            let error = validate_bound_nominal_callback_placement(&placement)
                .expect_err("destination requires its own complete, exclusive application");
            assert!(error.0.contains("callback destination"));
        }
    }
}

fn resource_receipt(
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
) -> checked_trees::CheckedCallbackResourceReceipt {
    resource_receipt_with_commitment(
        machine,
        entry,
        contract_report_fingerprint,
        checked_trees::MachineContractCommitment::from_digest([0x55; 32]),
    )
}

fn resource_receipt_with_commitment(
    machine: SymbolHandle,
    entry: SymbolHandle,
    contract_report_fingerprint: u64,
    contract_commitment: checked_trees::MachineContractCommitment,
) -> checked_trees::CheckedCallbackResourceReceipt {
    checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(
        &checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
            machine,
            entry,
            contract_report_fingerprint,
            contract_commitment,
        ),
    )
    .expect("canonical checked callback resource receipt")
}

pub(crate) fn placement() -> BoundNominalCallbackPlacement {
    let validated = calling_conventions::evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature::default(),
    )
    .expect("empty callback entry plan");
    let selected_machine = SymbolHandle::from_parts(4, 2);
    let selected_entry = SymbolHandle::from_parts(5, 3);
    BoundNominalCallbackPlacement {
        application_index: 0,
        site: NominalMachineUseSite::Expression(
            checked_trees::expression::ExpressionHandle::from_parts(9, 1),
        ),
        registration_operation: SymbolHandle::from_arena_index(3),
        static_machine_ordinal: 7,
        selected_machine,
        selected_entry,
        satisfaction_trait: SymbolHandle::from_arena_index(6),
        satisfaction_requirement: SymbolHandle::from_arena_index(8),
        canonical_requirement_overload: "Handler::call".to_owned(),
        boundary_calling_plan_report_fingerprint: validated.contract_report_fingerprint(),
        resource_receipt: resource_receipt(selected_machine, selected_entry, 0xfeed),
        boundary_entry_plan: validated.plan().clone(),
        private_materialization: None,
    }
}

#[test]
fn canonical_private_symbol_binds_site_selected_handles_ordinal_and_plan() {
    let baseline = placement();
    let baseline_symbol = canonical_callback_private_symbol(&baseline);
    let mut drifts = Vec::new();

    let mut site = baseline.clone();
    site.site = NominalMachineUseSite::Expression(
        checked_trees::expression::ExpressionHandle::from_parts(9, 4),
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
    fingerprint.boundary_calling_plan_report_fingerprint ^= 1;
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
        calling_conventions::Preemption::ProviderDefined;
    let error = validate_bound_nominal_callback_placement(&plan_drift)
        .expect_err("changed callback plan must not retain the old fingerprint");
    assert!(error.0.contains("drifted from its retained fingerprint"));

    let mut resource_drift = placement();
    resource_drift.resource_receipt = resource_receipt(
        resource_drift.selected_machine,
        SymbolHandle::from_parts(5, 4),
        0xfeed,
    );
    let error = validate_bound_nominal_callback_placement(&resource_drift)
        .expect_err("another checked resource entry cannot authorize the callback placement");
    assert!(
        error
            .0
            .contains("resource receipt does not bind its selected machine entry")
    );

    let mut fingerprint_drift = baseline;
    fingerprint_drift.boundary_calling_plan_report_fingerprint ^= 1;
    let error = validate_bound_nominal_callback_placement(&fingerprint_drift)
        .expect_err("changed callback fingerprint must not retain the old plan");
    assert!(error.0.contains("drifted from its retained fingerprint"));
}

#[test]
fn callback_placement_binding_identity_binds_registration_and_satisfaction_row() {
    let baseline = placement();
    let identity = callback_placement_binding_identity(&baseline);

    let mut compact_equal_plan_substitution = baseline.clone();
    compact_equal_plan_substitution
        .boundary_entry_plan
        .state
        .preemption = calling_conventions::Preemption::ProviderDefined;
    assert_eq!(
        compact_equal_plan_substitution.boundary_calling_plan_report_fingerprint,
        baseline.boundary_calling_plan_report_fingerprint,
    );
    assert_ne!(
        callback_placement_binding_identity(&compact_equal_plan_substitution),
        identity,
        "an equal compact report coordinate cannot hide exact plan substitution",
    );

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

    let mut resource_drift = placement();
    resource_drift.resource_receipt = resource_receipt(
        resource_drift.selected_machine,
        resource_drift.selected_entry,
        0xbeef,
    );
    assert_ne!(
        callback_placement_binding_identity(&resource_drift),
        identity
    );

    let mut compact_equal_resource_substitution = placement();
    compact_equal_resource_substitution.resource_receipt = resource_receipt_with_commitment(
        compact_equal_resource_substitution.selected_machine,
        compact_equal_resource_substitution.selected_entry,
        0xfeed,
        checked_trees::MachineContractCommitment::from_digest([0x77; 32]),
    );
    assert_eq!(
        compact_equal_resource_substitution
            .resource_receipt
            .contract_report_fingerprint(),
        identity.resource_receipt.contract_report_fingerprint(),
    );
    assert_ne!(
        callback_placement_binding_identity(&compact_equal_resource_substitution),
        identity,
        "an equal compact resource coordinate cannot hide contract substitution",
    );
}

#[test]
fn callback_thunk_placement_report_fingerprint_binds_exact_ordered_receipts() {
    let baseline = placement();
    let entry_key = StateKey {
        machine: baseline.selected_machine,
        state: baseline.selected_entry,
        segment_index: 0,
    };
    let function_identity =
        function_identity::MachineFunctionIdentity::callback_thunk(entry_key, 0)
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
        callback_thunk_placement_identity_report_fingerprint(std::slice::from_ref(&thunk));

    let mut drifted = thunk.clone();
    drifted.placement_identity.satisfaction_requirement = SymbolHandle::from_parts(8, 2);
    assert_ne!(
        callback_thunk_placement_identity_report_fingerprint(&[drifted]),
        fingerprint
    );

    let mut resource_drifted = thunk.clone();
    resource_drifted.placement_identity.resource_receipt = resource_receipt(
        resource_drifted.placement_identity.selected_machine,
        resource_drifted.placement_identity.selected_entry,
        0xbeef,
    );
    assert_ne!(
        callback_thunk_placement_identity_report_fingerprint(&[resource_drifted]),
        fingerprint
    );

    let mut reindexed = thunk;
    reindexed.placement_index = 1;
    assert_ne!(
        callback_thunk_placement_identity_report_fingerprint(&[reindexed]),
        fingerprint
    );
}
