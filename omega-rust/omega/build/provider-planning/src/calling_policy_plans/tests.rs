//! Tests for calling-policy plan evaluation.

use super::{
    BoundaryCallbackBinder, BoundaryCallingPlanRealization, BoundaryNativeParameter,
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryOpaqueRepresentationMovementRole, BoundaryOpaqueRepresentationPathElement,
    BoundaryOpaqueRepresentationUse, BoundaryValueClass, BoundaryValueField, BoundaryValueShape,
    MaterializedBoundarySignature, callback_layout_catalog,
    materialized_boundary_signature_from_abi, validate_nominal_callback_placement_bindings,
};
use crate::calling_policy_plans::boundary_signatures::boundary_shape_path;
use crate::calling_policy_plans::boundary_signatures::compatibility_call_signature;
use crate::calling_policy_plans::boundary_signatures::exact_boundary_layout_root_symbol;
use crate::calling_policy_plans::boundary_signatures::exact_compatibility_overload_index;
use crate::calling_policy_plans::build_time_decoding::decode_native_place;
use crate::calling_policy_plans::build_time_decoding::field;
use crate::calling_policy_plans::build_time_decoding::struct_parts;
use crate::calling_policy_plans::build_time_decoding::uint;
use crate::calling_policy_plans::callback_bindings::callback_plan_report_fingerprint;
use crate::calling_policy_plans::callback_bindings::reconcile_closed_callback_plan_identity;
use crate::calling_policy_plans::callback_bindings::validate_callback_requirement_report_collision;
use crate::calling_policy_plans::callback_bindings::validate_fresh_native_parameter_report_identity;
use crate::calling_policy_plans::plan_computation::boundary_plan_application_identity;
use crate::calling_policy_plans::plan_computation::build_boundary_signature;
use crate::calling_policy_plans::plan_computation::validate_authored_abi_shape;
use crate::calling_policy_plans::plan_computation::validate_materialized_boundary_plan_result;
use crate::calling_policy_plans::value_shapes::case;
use build_time_evaluation::BuildTimeValue;
use calling_conventions::BoundaryPlanResult;
use calling_conventions::CallSignature;
use calling_conventions::CallbackBinderRequirement;
use calling_conventions::CallbackMaterialization;
use calling_conventions::CallbackMaterializationContext;
use calling_conventions::CallbackRequirementId;
use calling_conventions::CallingPolicy;
use calling_conventions::LayoutPlanId;
use calling_conventions::LayoutSlotId;
use calling_conventions::NativeCallbackDemand;
use calling_conventions::NativeParameterId;
use calling_conventions::NativePlace;
use calling_conventions::StaticMachineBinderId;
use calling_conventions::ValidatedBoundaryEntryPlan;
use calling_conventions::ValuePlacement;
use calling_conventions::ValueShape;
use calling_conventions::evaluate_ordinary_boundary_entry_plan;
use calling_conventions::nominal_callback_native_parameter_id;
use calling_conventions::validate_boundary_entry_plan_with_callback_materializations;
use sha2::Digest;
use sha2::Sha256;
use target::NativeTarget;
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceNode;

fn test_opaque_representation_use(shape_root: u16) -> BoundaryOpaqueRepresentationUse {
    let conformance_application_commitment = [0x41; 32];
    let lifecycle = representation_planning::OpaqueRepresentationLifecycleDisposition::Inert;
    let copy_disposition =
        representation_planning::OpaqueRepresentationCopyDisposition::PlacementOnly;
    let origin = representation_planning::OpaqueRepresentationApplicationOrigin::NamedConformance;
    BoundaryOpaqueRepresentationUse {
        opaque: symbols::SymbolHandle::from_arena_index(1),
        conformance: symbols::SymbolHandle::from_arena_index(2),
        carrier: symbols::SymbolHandle::from_arena_index(3),
        shape_root,
        application_report_fingerprint: 17,
        conformance_application_commitment,
        representation_schema_version:
            representation_planning::OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION,
        origin,
        lifecycle,
        copy_disposition,
        selected_application_commitment: representation_planning::selected_application_commitment(
            conformance_application_commitment,
            lifecycle,
            copy_disposition,
            origin,
        ),
    }
}

#[test]
fn opaque_shape_path_rejects_one_marker_reused_by_multiple_record_fields() {
    let shapes = vec![
        BoundaryValueShape {
            class: BoundaryValueClass::Integer,
            byte_size: 8,
            alignment: 8,
        },
        BoundaryValueShape {
            class: BoundaryValueClass::Record {
                first_field: 0,
                field_count: 2,
            },
            byte_size: 16,
            alignment: 8,
        },
    ];
    let fields = vec![
        BoundaryValueField {
            shape: 0,
            byte_offset: 0,
        },
        BoundaryValueField {
            shape: 0,
            byte_offset: 8,
        },
    ];

    let error = boundary_shape_path(&shapes, &fields, 1, 0)
        .expect_err("one opaque shape marker cannot name two record occurrences");
    assert!(error.contains("multiple record fields"), "{error}");
}

#[test]
fn opaque_shape_path_rejects_an_out_of_range_root() {
    let shapes = vec![BoundaryValueShape {
        class: BoundaryValueClass::Integer,
        byte_size: 8,
        alignment: 8,
    }];

    let error = boundary_shape_path(&shapes, &[], 1, 0)
        .expect_err("an out-of-range shape-graph root must reject");
    assert!(error.contains("out-of-range node"), "{error}");
}

#[test]
fn opaque_shape_path_rejects_an_out_of_range_record_field_span() {
    let shapes = vec![
        BoundaryValueShape {
            class: BoundaryValueClass::Integer,
            byte_size: 8,
            alignment: 8,
        },
        BoundaryValueShape {
            class: BoundaryValueClass::Record {
                first_field: 0,
                field_count: 2,
            },
            byte_size: 16,
            alignment: 8,
        },
    ];
    let fields = vec![BoundaryValueField {
        shape: 0,
        byte_offset: 0,
    }];

    let error = boundary_shape_path(&shapes, &fields, 1, 0)
        .expect_err("a record field span outside the checked shape graph must reject");
    assert!(error.contains("out-of-range field span"), "{error}");
}

#[test]
fn opaque_shape_path_retains_fixed_array_and_record_coordinates() {
    let shapes = vec![
        BoundaryValueShape {
            class: BoundaryValueClass::Integer,
            byte_size: 8,
            alignment: 8,
        },
        BoundaryValueShape {
            class: BoundaryValueClass::Record {
                first_field: 0,
                field_count: 1,
            },
            byte_size: 8,
            alignment: 8,
        },
        BoundaryValueShape {
            class: BoundaryValueClass::FixedArray {
                element: 1,
                length: 4,
            },
            byte_size: 32,
            alignment: 8,
        },
    ];
    let fields = vec![BoundaryValueField {
        shape: 0,
        byte_offset: 0,
    }];

    assert_eq!(
        boundary_shape_path(&shapes, &fields, 2, 0)
            .expect("closed postorder shape graph")
            .expect("target shape is reachable"),
        vec![
            BoundaryOpaqueRepresentationPathElement::FixedArrayElement,
            BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 0 },
        ]
    );
}

#[test]
fn boundary_plan_application_commitment_binds_opaque_shape_root() {
    let shape = ValueShape::integer(8, 8);
    let call_signature = CallSignature {
        parameters: vec![shape, shape],
        result: None,
    };
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::host()),
        &call_signature,
    )
    .expect("ordinary two-parameter plan");
    let mut signature = materialized_boundary_signature_from_abi(&call_signature)
        .expect("materialized two-parameter signature");
    signature
        .opaque_representations
        .push(test_opaque_representation_use(0));

    let first = boundary_plan_application_identity(&signature, &validated);
    signature.opaque_representations[0].shape_root = 1;
    let second = boundary_plan_application_identity(&signature, &validated);
    assert_ne!(first, second);
}

#[test]
fn opaque_movement_rejects_an_out_of_range_shape_marker() {
    let shape = ValueShape::integer(8, 8);
    let call_signature = CallSignature {
        parameters: vec![shape],
        result: None,
    };
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::host()),
        &call_signature,
    )
    .expect("ordinary one-parameter plan");
    let mut signature = materialized_boundary_signature_from_abi(&call_signature)
        .expect("materialized one-parameter signature");
    signature
        .opaque_representations
        .push(test_opaque_representation_use(u16::MAX));

    let error = signature
        .opaque_representation_movement(&signature.opaque_representations[0], &validated)
        .expect_err("an out-of-range opaque shape marker must not rejoin movement");
    assert!(
        error.contains("belongs to 0 top-level boundary occurrences"),
        "{error}"
    );
}

#[test]
fn opaque_movement_rejoins_an_exact_result_placement() {
    let shape = ValueShape::integer(8, 8);
    let call_signature = CallSignature {
        parameters: vec![shape],
        result: Some(shape),
    };
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::host()),
        &call_signature,
    )
    .expect("ordinary parameter-and-result plan");
    let mut signature = materialized_boundary_signature_from_abi(&call_signature)
        .expect("materialized parameter-and-result signature");
    let result_root = signature.result.expect("materialized result root");
    signature
        .opaque_representations
        .push(test_opaque_representation_use(result_root));

    let movement = signature
        .opaque_representation_movement(&signature.opaque_representations[0], &validated)
        .expect("result marker rejoins one exact validated placement");
    assert_eq!(
        movement.role(),
        BoundaryOpaqueRepresentationMovementRole::Result
    );
    assert!(movement.path().is_empty());
    assert_eq!(
        movement.placement(),
        validated.plan().call.result.as_ref().unwrap()
    );
}

#[test]
fn boundary_application_replay_rejects_stale_opaque_custody_lanes() {
    let call_signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::native_for_target(NativeTarget::host()),
        &call_signature,
    )
    .expect("ordinary one-parameter plan");
    let materialized_signature = materialized_boundary_signature_from_abi(&call_signature)
        .expect("materialized one-parameter signature");
    let (report_fingerprint, application_commitment) =
        boundary_plan_application_identity(&materialized_signature, &validated);
    let realization = BoundaryCallingPlanRealization {
        boundary_trait: symbols::SymbolHandle::from_arena_index(1),
        boundary_arguments: Vec::new(),
        requirement_machine: symbols::SymbolHandle::from_arena_index(2),
        report_fingerprint,
        commitment: typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
            application_commitment,
        ),
        boundary_entry_plan: validated.plan().clone(),
        exact_boundary_entry_plan: validated.plan().clone(),
        callback_binders: Vec::new(),
        callback_demands: Vec::new(),
        callback_context_closed: false,
        native_parameters: Vec::new(),
        materialized_signature,
        policy_machine: String::new(),
        relationship_span: source::SourceSpan::default(),
    };

    // Fresh custody: no retained opaque uses replays the same application
    // identity the realization was minted with.
    let (_, fingerprint, commitment) = realization
        .replayed_validated_application()
        .expect("fresh realization replays its exact validated application");
    assert_eq!(fingerprint, report_fingerprint);
    assert_eq!(commitment, realization.commitment);

    // A correctly committed use is not stale custody: the replay proceeds past
    // the custody arm and can only fail later at movement rejoin.
    let mut fresh = realization.clone();
    fresh
        .materialized_signature
        .opaque_representations
        .push(test_opaque_representation_use(0));
    if let Err(error) = fresh.replayed_validated_application() {
        assert!(
            !error
                .0
                .contains("stale opaque-representation application custody"),
            "{error}"
        );
    }

    // Every stale-custody lane rejects with the gate's own diagnostic.
    let stale_uses = [
        BoundaryOpaqueRepresentationUse {
            opaque: symbols::SymbolHandle::default(),
            ..test_opaque_representation_use(0)
        },
        BoundaryOpaqueRepresentationUse {
            conformance: symbols::SymbolHandle::default(),
            ..test_opaque_representation_use(0)
        },
        BoundaryOpaqueRepresentationUse {
            carrier: symbols::SymbolHandle::default(),
            ..test_opaque_representation_use(0)
        },
        test_opaque_representation_use(u16::MAX),
        BoundaryOpaqueRepresentationUse {
            representation_schema_version:
                representation_planning::OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION + 1,
            ..test_opaque_representation_use(0)
        },
        BoundaryOpaqueRepresentationUse {
            selected_application_commitment: [0xEE; 32],
            ..test_opaque_representation_use(0)
        },
    ];
    for stale in stale_uses {
        let mut tampered = realization.clone();
        tampered
            .materialized_signature
            .opaque_representations
            .push(stale);
        let error = tampered
            .replayed_validated_application()
            .expect_err("a stale opaque-representation use must reject");
        assert!(
            error
                .0
                .contains("stale opaque-representation application custody"),
            "{error}"
        );
    }
}

fn legacy_boundary_plan_application_v2_identity(
    signature: &MaterializedBoundarySignature,
    validated: &ValidatedBoundaryEntryPlan,
) -> (u64, [u8; 32]) {
    let mut strong = Sha256::new();
    strong.update(b"omega.boundary-plan-application.v2");
    strong.update((signature.owner_requirement_identity.len() as u64).to_le_bytes());
    strong.update(signature.owner_requirement_identity.as_bytes());
    strong.update((signature.native_parameters.len() as u64).to_le_bytes());
    for parameter in &signature.native_parameters {
        strong.update(parameter.identity.get().to_le_bytes());
        strong.update(parameter.native_ordinal.to_le_bytes());
        match parameter.origin {
            BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } => {
                strong.update([1]);
                strong.update(formal_ordinal.to_le_bytes());
            }
            BoundaryNativeParameterOrigin::PrivateCallback {
                binder,
                requirement,
            } => {
                strong.update([2]);
                strong.update(binder.get().to_le_bytes());
                strong.update(requirement.get().to_le_bytes());
            }
        }
        match parameter.shape {
            BoundaryNativeParameterShape::Semantic(root) => {
                strong.update([1]);
                strong.update(root.to_le_bytes());
            }
            BoundaryNativeParameterShape::TargetFunctionPointer {
                byte_size,
                alignment,
            } => {
                strong.update([2]);
                strong.update(byte_size.to_le_bytes());
                strong.update(alignment.to_le_bytes());
            }
        }
    }
    strong.update(validated.contract_commitment_digest());
    let commitment: [u8; 32] = strong.finalize().into();
    let report = callback_plan_report_fingerprint(
        b"omega.boundary-plan-application-report.v2",
        &[&commitment],
    );
    (report, commitment)
}

#[test]
fn compact_native_parameter_collision_rejects_within_exact_registrar_catalog() {
    let identity = NativeParameterId::new(0x51).expect("nonzero report identity");
    let prior = [BoundaryNativeParameter {
        identity,
        native_ordinal: 0,
        shape: BoundaryNativeParameterShape::Semantic(0),
        origin: BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 0 },
        layout_data_symbol: symbols::SymbolHandle::from_arena_index(3),
    }];

    let error = validate_fresh_native_parameter_report_identity(&prior, identity, 1)
        .expect_err("a second exact parameter cannot reuse a compact report identity");
    assert!(error.contains("collides with an earlier exact parameter"));
}

#[test]
fn boundary_application_v3_commits_nominal_telescope_order() {
    let call_signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8), ValueShape::integer(8, 8)],
        result: None,
    };
    let validated =
        evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &call_signature)
            .expect("Microsoft x64 ordinary plan");
    let signature = materialized_boundary_signature_from_abi(&call_signature).unwrap();
    let first = boundary_plan_application_identity(&signature, &validated);

    let mut reordered = signature.clone();
    reordered.native_parameters.swap(0, 1);
    let second = boundary_plan_application_identity(&reordered, &validated);
    assert_ne!(
        first, second,
        "equally shaped nominal parameters cannot reorder under one application identity"
    );

    let mut renamed = signature;
    renamed.native_parameters[0].identity =
        nominal_callback_native_parameter_id("omega.synthetic-abi-signature", "renamed-parameter");
    assert_ne!(
        first,
        boundary_plan_application_identity(&renamed, &validated),
        "a changed declaration-owned native parameter identity must reissue the application"
    );
}

#[test]
fn materialized_plan_cannot_create_an_undeclared_native_parameter() {
    let call_signature = CallSignature {
        parameters: vec![ValueShape::integer(8, 8)],
        result: None,
    };
    let validated =
        evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &call_signature)
            .expect("Microsoft x64 ordinary plan");
    let signature = materialized_boundary_signature_from_abi(&call_signature).unwrap();
    let mut invented = validated.plan().clone();
    invented
        .call
        .parameters
        .push(invented.call.parameters[0].clone());

    let error = validate_materialized_boundary_plan_result(
        BoundaryPlanResult::Accepted(invented),
        &signature,
    )
    .expect_err("a calling policy cannot add a parameter to the declared telescope");
    assert!(
        error
            .contains("plan places 2 parameters for a boundary signature with 1 native parameters"),
        "unexpected diagnostic: {error}",
    );
}

#[test]
fn compact_callback_requirement_collision_rejects_exact_requirement_substitution() {
    let report = CallbackRequirementId::new(0x61).expect("nonzero report identity");
    let catalog = [(report, "package::First::call#exact".to_owned())];

    let error = validate_callback_requirement_report_collision(
        &catalog,
        report,
        "package::Second::call#exact",
    )
    .expect_err("compact equality cannot substitute another exact callback requirement");
    assert!(error.contains("distinct exact callback requirements collide"));

    validate_callback_requirement_report_collision(&catalog, report, "package::First::call#exact")
        .expect("the same exact requirement may reuse its catalog report coordinate");
}

#[test]
fn callback_native_place_decoder_preserves_nominal_field_path() {
    let direct = case(
        "Parameter",
        vec![("parameter".to_owned(), BuildTimeValue::Int(7))],
    );
    assert_eq!(
        decode_native_place(&direct).expect("direct native place"),
        NativePlace::Parameter(NativeParameterId::new(7).unwrap())
    );

    let field = case(
        "Field",
        vec![
            ("parameter".to_owned(), BuildTimeValue::Int(7)),
            ("layout".to_owned(), BuildTimeValue::Int(11)),
            (
                "field_path".to_owned(),
                BuildTimeValue::Array(vec![BuildTimeValue::Int(13), BuildTimeValue::Int(17)]),
            ),
            ("field_path_count".to_owned(), BuildTimeValue::Int(2)),
        ],
    );
    assert_eq!(
        decode_native_place(&field).expect("nested native place"),
        NativePlace::Field {
            parameter: NativeParameterId::new(7).unwrap(),
            layout: LayoutPlanId::new(11).unwrap(),
            field_path: vec![
                LayoutSlotId::new(13).unwrap(),
                LayoutSlotId::new(17).unwrap(),
            ],
        }
    );

    let mut empty = field;
    let BuildTimeValue::Case { payload, .. } = &mut empty else {
        unreachable!()
    };
    payload
        .iter_mut()
        .find(|(name, _)| name == "field_path_count")
        .expect("field path count")
        .1 = BuildTimeValue::Int(0);
    assert!(
        decode_native_place(&empty)
            .expect_err("empty native field path")
            .contains("cannot be empty")
    );
}

#[test]
fn boundary_signature_publishes_compiler_issued_callback_catalogs() {
    let binder = BoundaryCallbackBinder {
        binder: StaticMachineBinderId::new(41).unwrap(),
        requirement: CallbackRequirementId::new(43).unwrap(),
        static_machine_ordinal: 2,
        parameter_symbol: symbols::SymbolHandle::from_arena_index(5),
        requirement_trait: symbols::SymbolHandle::from_arena_index(7),
        requirement_machine: symbols::SymbolHandle::from_arena_index(11),
    };
    let demand = NativeCallbackDemand {
        destination: NativePlace::Field {
            parameter: NativeParameterId::new(47).unwrap(),
            layout: LayoutPlanId::new(53).unwrap(),
            field_path: vec![LayoutSlotId::new(59).unwrap()],
        },
        requirement: CallbackRequirementId::new(43).unwrap(),
    };
    let signature = MaterializedBoundarySignature {
        owner_requirement_identity: "test::Registrar::register#exact".to_owned(),
        native_target: NativeTarget::host(),
        opaque_representations: Vec::new(),
        shapes: Vec::new(),
        fields: Vec::new(),
        parameters: Vec::new(),
        callback_binders: vec![binder],
        callback_demands: vec![demand.clone()],
        callback_layout_catalog: Vec::new(),
        native_parameters: Vec::new(),
        direct_callback_parameters: Vec::new(),
        result: None,
    };
    let value = build_boundary_signature(&signature);
    let fields = struct_parts(&value, "BoundarySignature").expect("signature struct");
    assert_eq!(
        uint(
            field(fields, "callback_binder_count", "BoundarySignature")
                .expect("callback binder count"),
            "callback binder count",
        )
        .unwrap(),
        1
    );
    let BuildTimeValue::Array(rows) =
        field(fields, "callback_binders", "BoundarySignature").expect("callback binder rows")
    else {
        panic!("callback binder catalog is not an array")
    };
    let row = struct_parts(&rows[0], "CallbackBinderIdentity").expect("binder row");
    assert_eq!(
        uint(field(row, "binder", "binder row").unwrap(), "binder").unwrap(),
        41
    );
    assert_eq!(
        uint(
            field(row, "requirement", "binder row").unwrap(),
            "requirement",
        )
        .unwrap(),
        43
    );
    assert_eq!(
        uint(
            field(fields, "callback_demand_count", "BoundarySignature")
                .expect("callback demand count"),
            "callback demand count",
        )
        .unwrap(),
        1
    );
    let BuildTimeValue::Array(rows) =
        field(fields, "callback_demands", "BoundarySignature").expect("callback demands")
    else {
        panic!("callback demand catalog is not an array")
    };
    let row = struct_parts(&rows[0], "NativeCallbackDemandIdentity").expect("demand row");
    assert_eq!(
        decode_native_place(field(row, "destination", "demand row").unwrap()).unwrap(),
        demand.destination
    );
    assert_eq!(
        uint(
            field(row, "requirement", "demand row").unwrap(),
            "requirement",
        )
        .unwrap(),
        43
    );
}

fn nominal_callback_fixture() -> (
    checked_trees::CheckedTrees,
    Vec<BoundaryCallingPlanRealization>,
) {
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature::default(),
    )
    .expect("empty ordinary boundary plan");
    let materialized_signature =
        materialized_boundary_signature_from_abi(&CallSignature::default()).unwrap();
    let (report_fingerprint, application_commitment) =
        boundary_plan_application_identity(&materialized_signature, &validated);
    let commitment = typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
        application_commitment,
    );
    let boundary_trait = symbols::SymbolHandle::from_arena_index(1);
    let requirement = symbols::SymbolHandle::from_arena_index(2);
    let registration_operation = symbols::SymbolHandle::from_arena_index(3);
    let selected_machine = symbols::SymbolHandle::from_arena_index(4);
    let selected_entry = symbols::SymbolHandle::from_arena_index(5);
    let selected_actual_fingerprint = 7;
    let resource_envelope = checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
        selected_machine,
        selected_entry,
        selected_actual_fingerprint,
        checked_trees::MachineContractCommitment::from_digest([8; 32]),
    );
    let resource_receipt =
        checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(&resource_envelope)
            .expect("canonical callback resource receipt");
    let nominal_use = checked_trees::CheckedNominalMachineUse {
        site: checked_trees::NominalMachineUseSite::Expression(
            typed_trees::expression::ExpressionHandle::from_arena_index(1),
        ),
        registration_operation,
        static_machine_ordinal: 0,
        selected_machine,
        selected_entry,
        satisfaction_trait: boundary_trait,
        satisfaction_requirement: requirement,
        canonical_requirement_overload: "Handler::call(i32)->i32".to_owned(),
        published_requirement_envelope: checked_trees::CheckedMachineContractEnvelopeIdentity {
            contract_report_fingerprint: 6,
            contract_commitment: checked_trees::MachineContractCommitment::from_digest([6; 32]),
        },
        selected_actual_envelope: checked_trees::CheckedMachineContractEnvelopeIdentity {
            contract_report_fingerprint: selected_actual_fingerprint,
            contract_commitment: checked_trees::MachineContractCommitment::from_digest([8; 32]),
        },
        callback_placement: Some(checked_trees::CheckedCallbackPlacementIdentity {
            boundary_calling_plan_report_fingerprint: report_fingerprint,
            boundary_calling_plan_commitment: commitment,
            resource_receipt,
        }),
        refinement: checked_trees::CheckedMachineContractRefinement {
            published_requirement_report_fingerprint: 6,
            published_requirement_commitment: checked_trees::MachineContractCommitment::from_digest(
                [6; 32],
            ),
            selected_actual_report_fingerprint: selected_actual_fingerprint,
            selected_actual_commitment: checked_trees::MachineContractCommitment::from_digest(
                [8; 32],
            ),
        },
    };
    let mut checked = checked_trees::CheckedTrees::default();
    checked.facts.contract_plans.machines = vec![checked_trees::MachineContractPlan {
        machine: selected_machine,
        closed_scalar_values: Default::default(),
        crash: Default::default(),
        report_fingerprint: selected_actual_fingerprint,
        commitment: checked_trees::MachineContractCommitment::from_digest([8; 32]),
    }];
    checked.facts.contract_plans.crash_capsules =
        vec![checked_trees::CrashContractCapsule::new_with_commitment(
            boundary_trait,
            requirement,
            6,
            checked_trees::MachineContractCommitment::from_digest([6; 32]),
            Vec::new(),
        )];
    checked.facts.contract_plans.realized_envelopes =
        vec![checked_trees::RealizedMachineContractEnvelope {
            machine: selected_machine,
            contract_report_fingerprint: selected_actual_fingerprint,
            contract_commitment: checked_trees::MachineContractCommitment::from_digest([8; 32]),
            effective_service_reach: Vec::new(),
            concrete_service_reach: Vec::new(),
            unresolved_installation_reaches: Vec::new(),
            effective_synchronous_invocations: Vec::new(),
            checked_may_suspend: false,
            checked_may_block: false,
            checked_termination: language_semantics::TerminationGuarantee::NoGuarantee,
            checked_crash: checked_trees::CrashPlan::default(),
            mutation: Vec::new(),
            capabilities: Vec::new(),
            resources:
                checked_trees::CheckedMachineResourceEnvelopes::from_checked_contract_entries(
                    selected_machine,
                    selected_actual_fingerprint,
                    checked_trees::MachineContractCommitment::from_digest([8; 32]),
                    [selected_entry],
                ),
        }];
    checked.facts.nominal_machine_uses =
        checked_trees::NominalMachineUseFacts::try_with_uses([nominal_use])
            .expect("valid nominal callback row");
    let inbound_realization = BoundaryCallingPlanRealization {
        boundary_trait,
        boundary_arguments: Vec::new(),
        requirement_machine: requirement,
        report_fingerprint,
        commitment,
        boundary_entry_plan: validated.plan().clone(),
        exact_boundary_entry_plan: validated.plan().clone(),
        callback_binders: Vec::new(),
        callback_demands: Vec::new(),
        callback_context_closed: false,
        native_parameters: Vec::new(),
        materialized_signature: materialized_signature.clone(),
        policy_machine: String::new(),
        relationship_span: source::SourceSpan::default(),
    };
    let registrar_realization = BoundaryCallingPlanRealization {
        boundary_trait: symbols::SymbolHandle::from_arena_index(9),
        boundary_arguments: Vec::new(),
        requirement_machine: registration_operation,
        report_fingerprint,
        commitment,
        boundary_entry_plan: validated.plan().clone(),
        exact_boundary_entry_plan: validated.plan().clone(),
        callback_binders: vec![BoundaryCallbackBinder {
            binder: StaticMachineBinderId::new(31).unwrap(),
            requirement: CallbackRequirementId::new(37).unwrap(),
            static_machine_ordinal: 0,
            parameter_symbol: symbols::SymbolHandle::from_arena_index(11),
            requirement_trait: boundary_trait,
            requirement_machine: requirement,
        }],
        callback_demands: Vec::new(),
        callback_context_closed: false,
        native_parameters: Vec::new(),
        materialized_signature,
        policy_machine: String::new(),
        relationship_span: source::SourceSpan::default(),
    };
    (checked, vec![inbound_realization, registrar_realization])
}

#[test]
fn nominal_callback_placement_binds_one_exact_evaluated_plan() {
    let (checked, realizations) = nominal_callback_fixture();

    let bound = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect("exact checked and target plan identities should bind");
    let [bound] = bound.as_slice() else {
        panic!("one checked callback must produce one durable target plan");
    };
    let nominal_use = &checked.facts.nominal_machine_uses.uses[0];
    assert_eq!(bound.site, nominal_use.site);
    assert_eq!(
        bound.static_machine_ordinal,
        nominal_use.static_machine_ordinal
    );
    assert_eq!(bound.selected_machine, nominal_use.selected_machine);
    assert_eq!(bound.selected_entry, nominal_use.selected_entry);
    assert_eq!(
        bound.resource_receipt,
        nominal_use
            .callback_placement
            .expect("checked callback placement")
            .resource_receipt
    );
    assert_eq!(
        bound.boundary_calling_plan_report_fingerprint,
        realizations[0]
            .replayed_validated_plan()
            .unwrap()
            .contract_report_fingerprint()
    );
    assert_eq!(
        bound.boundary_entry_plan,
        realizations[0].boundary_entry_plan
    );
}

#[test]
fn nominal_callback_placement_rejects_self_consistent_legacy_v2_evidence() {
    let (mut checked, mut realizations) = nominal_callback_fixture();
    let validated = realizations[0]
        .replayed_validated_plan()
        .expect("fixture plan replays");
    let (legacy_report, legacy_digest) = legacy_boundary_plan_application_v2_identity(
        &realizations[0].materialized_signature,
        &validated,
    );
    let legacy_commitment =
        typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(legacy_digest);
    realizations[0].report_fingerprint = legacy_report;
    realizations[0].commitment = legacy_commitment;
    let placement = checked.facts.nominal_machine_uses.uses[0]
        .callback_placement
        .as_mut()
        .expect("fixture callback placement");
    placement.boundary_calling_plan_report_fingerprint = legacy_report;
    placement.boundary_calling_plan_commitment = legacy_commitment;

    let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect_err("retired application-v2 evidence must not be reinterpreted as v3");
    assert!(
        diagnostics[0]
            .message
            .contains("does not bind its exact evaluated target calling plan"),
        "unexpected diagnostic: {:?}",
        diagnostics[0],
    );
}

#[test]
fn nominal_callback_placement_rejoins_the_current_entry_resource_envelope() {
    let (mut checked, realizations) = nominal_callback_fixture();
    checked.facts.contract_plans.realized_envelopes.clear();
    let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect_err("a callback cannot lose its current resource envelope");
    assert!(
        diagnostics[0]
            .message
            .contains("missing its exact checked entry")
    );

    let (mut checked, realizations) = nominal_callback_fixture();
    let nominal_use = &mut checked.facts.nominal_machine_uses.uses[0];
    let foreign = checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
        nominal_use.selected_machine,
        symbols::SymbolHandle::from_arena_index(17),
        nominal_use
            .selected_actual_envelope
            .contract_report_fingerprint,
        nominal_use.selected_actual_envelope.contract_commitment,
    );
    nominal_use
        .callback_placement
        .as_mut()
        .expect("callback placement")
        .resource_receipt =
        checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(&foreign)
            .expect("canonical foreign entry receipt");
    let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect_err("a callback cannot substitute another entry resource receipt");
    assert!(
        diagnostics[0]
            .message
            .contains("does not bind its exact checked entry resource receipt")
    );
}

#[test]
fn nominal_callback_placement_rejects_compact_equal_resource_contract_substitution() {
    let (mut checked, realizations) = nominal_callback_fixture();
    let nominal_use = &mut checked.facts.nominal_machine_uses.uses[0];
    let substituted = checked_trees::CheckedEntryResourceEnvelope::from_checked_contract(
        nominal_use.selected_machine,
        nominal_use.selected_entry,
        nominal_use
            .selected_actual_envelope
            .contract_report_fingerprint,
        checked_trees::MachineContractCommitment::from_digest([0x77; 32]),
    );
    nominal_use
        .callback_placement
        .as_mut()
        .expect("callback placement")
        .resource_receipt =
        checked_trees::CheckedCallbackResourceReceipt::try_from_entry_envelope(&substituted)
            .expect("independently canonical compact-equal resource receipt");

    let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect_err("compact-equal resource contract substitution must reject");
    assert!(
        diagnostics[0]
            .message
            .contains("does not bind its exact checked entry resource receipt")
    );
}

#[test]
fn nominal_callback_consumer_replays_exact_materialization_context() {
    let (checked, mut realizations) = nominal_callback_fixture();
    let nominal_use = &checked.facts.nominal_machine_uses.uses[0];
    let satisfaction_trait = nominal_use.satisfaction_trait;
    let satisfaction_requirement = nominal_use.satisfaction_requirement;
    let realization = &mut realizations[1];
    let binder = StaticMachineBinderId::new(101).unwrap();
    realization.materialized_signature = callback_layout_catalog::signature_fixture();
    let demand = realization.materialized_signature.callback_demands[0].clone();
    let requirement = demand.requirement;
    let destination = demand.destination.clone();
    let target = realization.materialized_signature.native_target;
    let signature = CallSignature {
        parameters: vec![ValueShape::integer(
            u16::try_from(target.pointer_size).unwrap(),
            u16::try_from(target.pointer_alignment).unwrap(),
        )],
        result: None,
    };
    realization.boundary_entry_plan =
        evaluate_ordinary_boundary_entry_plan(CallingPolicy::native_for_target(target), &signature)
            .unwrap()
            .plan()
            .clone();
    realization.native_parameters = realization.materialized_signature.native_parameters.clone();
    realization.callback_binders = vec![BoundaryCallbackBinder {
        binder,
        requirement,
        static_machine_ordinal: 0,
        parameter_symbol: symbols::SymbolHandle::from_arena_index(13),
        requirement_trait: satisfaction_trait,
        requirement_machine: satisfaction_requirement,
    }];
    realization.callback_demands = vec![demand];
    realization.callback_context_closed = true;
    realization
        .boundary_entry_plan
        .call
        .callback_materializations = vec![CallbackMaterialization {
        binder,
        destination: destination.clone(),
    }];
    let context = CallbackMaterializationContext {
        binders: vec![CallbackBinderRequirement {
            binder,
            requirement,
        }],
        demands: realization.callback_demands.clone(),
    };
    let validated = validate_boundary_entry_plan_with_callback_materializations(
        realization.boundary_entry_plan.clone(),
        &signature,
        &context,
    )
    .unwrap();
    realization.materialized_signature.callback_binders = realization.callback_binders.clone();
    realization.materialized_signature.callback_demands = realization.callback_demands.clone();
    let (report_fingerprint, commitment) =
        boundary_plan_application_identity(&realization.materialized_signature, &validated);
    realization.report_fingerprint = report_fingerprint;
    realization.commitment =
        typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(commitment);
    realization.exact_boundary_entry_plan = realization.boundary_entry_plan.clone();
    let bound = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect("exact retained callback context should replay");
    let retained = bound[0]
        .private_materialization
        .as_ref()
        .expect("target-closed callback use retains its exact private row");
    assert_eq!(retained.binder, binder);
    assert_eq!(retained.destination, destination);
    assert_eq!(retained.requirement, requirement);
    assert_eq!(retained.context, context);
    assert!(
        retained.direct_registrar_parameter_application.is_none(),
        "a field destination must not acquire a direct native parameter application",
    );
    let mut direct_row_on_field = bound[0].clone();
    let shape = ValueShape::integer(8, 8);
    direct_row_on_field
        .private_materialization
        .as_mut()
        .unwrap()
        .direct_registrar_parameter_application =
        Some(calling_conventions::NativeParameterApplication {
            parameter: NativeParameterId::new(107).unwrap(),
            native_ordinal: 0,
            shape,
            placement: ValuePlacement {
                shape,
                locations: Vec::new(),
            },
        });
    assert!(
        backend_plan::validate_bound_nominal_callback_placement(&direct_row_on_field).is_err(),
        "a field destination must reject a substituted direct application row",
    );
    assert_eq!(
        retained.registrar_boundary_entry_plan,
        realizations[1].boundary_entry_plan
    );
    assert_eq!(
        retained.registrar_calling_plan_report_fingerprint,
        validated.contract_report_fingerprint()
    );

    let mut ordinal_drift = checked.clone();
    ordinal_drift.facts.nominal_machine_uses.uses[0].static_machine_ordinal = 1;
    let diagnostics = validate_nominal_callback_placement_bindings(&ordinal_drift, &realizations)
        .expect_err("a different static-machine ordinal cannot select the binder row");
    assert!(
        diagnostics[0]
            .message
            .contains("exact private materialization")
            && diagnostics[0]
                .message
                .contains("outbound registrar realizations"),
        "unexpected diagnostic: {:?}",
        diagnostics[0]
    );

    let mut operation_drift = checked.clone();
    operation_drift.facts.nominal_machine_uses.uses[0].registration_operation =
        symbols::SymbolHandle::from_arena_index(149);
    let diagnostics = validate_nominal_callback_placement_bindings(&operation_drift, &realizations)
        .expect_err("a substituted registrar operation cannot select the outbound plan");
    assert!(
        diagnostics[0]
            .message
            .contains("outbound registrar realizations"),
        "unexpected diagnostic: {:?}",
        diagnostics[0]
    );

    let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations[..1])
        .expect_err("the callback use cannot lose its outbound registrar realization");
    assert!(
        diagnostics[0]
            .message
            .contains("0 outbound registrar realizations"),
        "unexpected diagnostic: {:?}",
        diagnostics[0]
    );
    let duplicate_registrar = vec![
        realizations[0].clone(),
        realizations[1].clone(),
        realizations[1].clone(),
    ];
    let diagnostics = validate_nominal_callback_placement_bindings(&checked, &duplicate_registrar)
        .expect_err("the callback use cannot select duplicate outbound realizations");
    assert!(
        diagnostics[0]
            .message
            .contains("2 outbound registrar realizations"),
        "unexpected diagnostic: {:?}",
        diagnostics[0]
    );

    realizations[1].callback_demands[0].destination =
        NativePlace::Parameter(NativeParameterId::new(127).unwrap());
    let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect_err("demand substitution must fail the later consumer replay");
    assert!(
        diagnostics[0]
            .message
            .contains("callback layout catalog lost its exact ordered demand context"),
        "unexpected diagnostic: {:?}",
        diagnostics[0]
    );
}

#[test]
fn target_neutral_binder_defers_supply_but_target_closed_binder_rejects_missing_supply() {
    let (checked, mut realizations) = nominal_callback_fixture();
    validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect("a no-target checked realization must remain target-neutral");

    realizations[1].callback_context_closed = true;
    let diagnostics = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect_err("target closure must reject a binder without native supply");
    assert!(
        diagnostics[0]
            .message
            .contains("omits a nominal callback binder"),
        "unexpected diagnostic: {:?}",
        diagnostics[0]
    );
}

#[test]
fn private_callback_parameter_mapping_rejects_array_and_slice_wrappers() {
    let mut typed = TypedTrees::default();
    let symbol = symbols::SymbolHandle::from_arena_index(41);
    let named = typed.type_reference_table.insert(TypeReferenceNode::Named {
        symbol,
        name: typed_trees::name::Identifier::generated("NativeRecord"),
    });
    let reference = typed
        .type_reference_table
        .insert(TypeReferenceNode::Reference {
            referee: named,
            access: language_core::ReferenceAccess::Shared,
            lifetime: None,
        });
    let array = typed
        .type_reference_table
        .insert(TypeReferenceNode::FixedArray {
            element_type: named,
            length: typed_trees::types::FixedArrayLength::Literal(2),
        });
    let slice = typed.type_reference_table.insert(TypeReferenceNode::Slice {
        element_type: named,
    });

    assert_eq!(exact_boundary_layout_root_symbol(&typed, named), symbol);
    assert_eq!(exact_boundary_layout_root_symbol(&typed, reference), symbol);
    assert!(!exact_boundary_layout_root_symbol(&typed, array).is_valid());
    assert!(!exact_boundary_layout_root_symbol(&typed, slice).is_valid());
}

#[test]
fn nominal_callback_placement_rejects_missing_or_drifting_plan() {
    let (mut checked, realizations) = nominal_callback_fixture();
    let missing = validate_nominal_callback_placement_bindings(&checked, &[])
        .expect_err("a checked callback cannot lose its target plan");
    assert!(
        missing[0]
            .message
            .contains("0 target calling-plan realizations")
    );

    let duplicated = vec![realizations[0].clone(), realizations[0].clone()];
    let duplicate = validate_nominal_callback_placement_bindings(&checked, &duplicated)
        .expect_err("one callback cannot bind duplicate target plans");
    assert!(
        duplicate[0]
            .message
            .contains("2 target calling-plan realizations")
    );

    checked.facts.nominal_machine_uses.uses[0]
        .callback_placement
        .as_mut()
        .expect("callback placement")
        .boundary_calling_plan_report_fingerprint ^= 1;
    let drift = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect_err("a changed target plan identity must reject");
    assert!(
        drift[0]
            .message
            .contains("does not bind its exact evaluated target calling plan")
    );

    let (mut compact_equal_checked, compact_equal_realizations) = nominal_callback_fixture();
    compact_equal_checked.facts.nominal_machine_uses.uses[0]
        .callback_placement
        .as_mut()
        .expect("callback placement")
        .boundary_calling_plan_commitment =
        typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([0x5a; 32]);
    let substitution = validate_nominal_callback_placement_bindings(
        &compact_equal_checked,
        &compact_equal_realizations,
    )
    .expect_err("a compact-equal strong-plan substitution must reject");
    assert!(
        substitution[0]
            .message
            .contains("does not bind its exact evaluated target calling plan")
    );

    let (mut published_substitution, published_realizations) = nominal_callback_fixture();
    let substituted_contract = checked_trees::MachineContractCommitment::from_digest([0x6a; 32]);
    published_substitution.facts.nominal_machine_uses.uses[0]
        .published_requirement_envelope
        .contract_commitment = substituted_contract;
    published_substitution.facts.nominal_machine_uses.uses[0]
        .refinement
        .published_requirement_commitment = substituted_contract;
    let substitution = validate_nominal_callback_placement_bindings(
        &published_substitution,
        &published_realizations,
    )
    .expect_err("a compact-equal published requirement substitution must reject");
    assert!(
        substitution[0]
            .message
            .contains("does not bind its exact published requirement contract")
    );

    checked.facts.nominal_machine_uses.uses[0].callback_placement = None;
    let lost = validate_nominal_callback_placement_bindings(&checked, &realizations)
        .expect_err("a callback plan realization requires a checked join key");
    assert!(
        lost[0]
            .message
            .contains("lost its evaluated boundary calling-plan identity")
    );
}

#[test]
fn callback_closure_reconciliation_requires_the_preclosure_strong_commitment() {
    let (mut checked, realizations) = nominal_callback_fixture();
    let realization = &realizations[0];
    let original = checked.facts.nominal_machine_uses.uses[0]
        .callback_placement
        .expect("callback placement");
    let substituted_old =
        typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([0x5a; 32]);
    let closed = typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([0x6b; 32]);

    reconcile_closed_callback_plan_identity(
        &mut checked.facts.nominal_machine_uses.uses,
        realization.boundary_trait,
        realization.requirement_machine,
        realization.report_fingerprint,
        substituted_old,
        0xfeed,
        closed,
    );
    assert_eq!(
        checked.facts.nominal_machine_uses.uses[0].callback_placement,
        Some(original),
        "compact equality without the pre-closure commitment must not rewrite custody"
    );

    reconcile_closed_callback_plan_identity(
        &mut checked.facts.nominal_machine_uses.uses,
        realization.boundary_trait,
        realization.requirement_machine,
        realization.report_fingerprint,
        realization.commitment,
        0xfeed,
        closed,
    );
    let reconciled = checked.facts.nominal_machine_uses.uses[0]
        .callback_placement
        .expect("reconciled callback placement");
    assert_eq!(reconciled.boundary_calling_plan_report_fingerprint, 0xfeed);
    assert_eq!(reconciled.boundary_calling_plan_commitment, closed);
    assert_eq!(reconciled.resource_receipt, original.resource_receipt);
}

fn four_f32_array_signature() -> MaterializedBoundarySignature {
    MaterializedBoundarySignature {
        owner_requirement_identity: "test::Array::call#exact".to_owned(),
        native_target: NativeTarget::host(),
        opaque_representations: Vec::new(),
        shapes: vec![
            BoundaryValueShape {
                class: BoundaryValueClass::Float,
                byte_size: 4,
                alignment: 4,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::FixedArray {
                    element: 0,
                    length: 4,
                },
                byte_size: 16,
                alignment: 4,
            },
        ],
        fields: Vec::new(),
        parameters: vec![1],
        callback_binders: Vec::new(),
        callback_demands: Vec::new(),
        callback_layout_catalog: Vec::new(),
        native_parameters: Vec::new(),
        direct_callback_parameters: Vec::new(),
        result: None,
    }
}

#[test]
fn recursive_array_classification_is_policy_specific() {
    let signature = four_f32_array_signature();
    let hfa = ValueShape::homogeneous_float_aggregate(4, 4);
    validate_authored_abi_shape(&signature, 1, hfa, CallingPolicy::Aapcs64)
        .expect("AAPCS64 should classify four f32 elements as an HFA");
    validate_authored_abi_shape(&signature, 1, hfa, CallingPolicy::SystemVAMD64)
        .expect("SysV should classify four f32 elements through its SSE aggregate path");
    validate_authored_abi_shape(
        &signature,
        1,
        ValueShape::integer(16, 4),
        CallingPolicy::MicrosoftX64,
    )
    .expect("Microsoft x64 should classify the fixed array as an indirect aggregate");
}

#[test]
fn compatibility_signature_excludes_dispatch_only_table_parameter() {
    let signature = MaterializedBoundarySignature {
        owner_requirement_identity: "test::Compatibility::call#exact".to_owned(),
        native_target: NativeTarget::host(),
        opaque_representations: Vec::new(),
        shapes: vec![
            BoundaryValueShape {
                class: BoundaryValueClass::Integer,
                byte_size: 8,
                alignment: 8,
            },
            BoundaryValueShape {
                class: BoundaryValueClass::Integer,
                byte_size: 4,
                alignment: 4,
            },
        ],
        fields: Vec::new(),
        parameters: vec![0, 1, 0],
        callback_binders: Vec::new(),
        callback_demands: Vec::new(),
        callback_layout_catalog: Vec::new(),
        native_parameters: Vec::new(),
        direct_callback_parameters: Vec::new(),
        result: Some(1),
    };
    let wire = compatibility_call_signature(&signature, CallingPolicy::MicrosoftX64, 1)
        .expect("one table pointer should project out of the wire signature");
    assert_eq!(
        wire.parameters,
        [ValueShape::integer(4, 4), ValueShape::integer(8, 8)]
    );
    assert_eq!(wire.result, Some(ValueShape::integer(4, 4)));
    assert!(compatibility_call_signature(&signature, CallingPolicy::MicrosoftX64, 4).is_err());
}

#[test]
fn compatibility_lookup_requires_one_exact_nonempty_overload_identity() {
    let cases: [(&str, &str, &[&str], Result<Option<usize>, &str>); 5] = [
        (
            "empty",
            "",
            &["exact"],
            Err(
                "compatibility calling-plan lookup for `pkg::Readable::read` has no exact requirement overload identity",
            ),
        ),
        ("absent", "missing", &["first", "second"], Ok(None)),
        ("name-only singleton", "exact", &["lookalike"], Ok(None)),
        (
            "unique same-name overload",
            "exact",
            &["lookalike", "exact", "other"],
            Ok(Some(1)),
        ),
        (
            "duplicate exact",
            "exact",
            &["exact", "lookalike", "exact"],
            Err(
                "compatibility calling-plan lookup for `pkg::Readable::read` matches 2 exact requirement overload rows for identity `exact`",
            ),
        ),
    ];

    for (case, requirement_identity, candidates, expected) in cases {
        let actual = exact_compatibility_overload_index(
            "pkg::Readable",
            "read",
            requirement_identity,
            candidates.iter().copied(),
        );
        assert_eq!(
            actual
                .as_ref()
                .map(|candidate_index| *candidate_index)
                .map_err(String::as_str),
            expected,
            "case: {case}",
        );
    }
}

#[test]
fn authored_policy_cannot_publish_a_preclassified_shape_for_the_wrong_abi() {
    let signature = four_f32_array_signature();
    let error = validate_authored_abi_shape(
        &signature,
        1,
        ValueShape::integer(16, 4),
        CallingPolicy::Aapcs64,
    )
    .expect_err("AAPCS64 policy must preserve the HFA classification");
    assert!(
        error.contains("requires HomogeneousFloatAggregate"),
        "{error}"
    );
}
