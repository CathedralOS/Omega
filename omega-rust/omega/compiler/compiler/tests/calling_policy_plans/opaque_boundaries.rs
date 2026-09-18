use super::{
    INTERRUPT_OPAQUE_RESULT_POLICY, INTERRUPT_POLICY, INTERRUPT_REPRESENTATION_BUILD, POLICY,
    compile_project_negative, interrupt_envelope_policy, retained_interrupt_representation,
    selected_plan_for_external_root, write_program, write_project,
};
use calling_conventions::{CallSignature, CallingPolicy, ValueShape};
use compiler::{CheckedCompileRequest, compile_to_checked};
use provider_planning::calling_policy_plans::{
    BoundaryOpaqueRepresentationMovementRole, BoundaryOpaqueRepresentationPathElement,
    BoundaryValueClass, evaluate_calling_policy_plan,
};
use provider_planning::{
    selected_external_root_entry_fact_bindings, selected_external_root_provider_plan,
    selected_external_root_provider_plan_id,
};
use std::fs;

#[test]
fn source_interrupt_policy_publishes_and_selects_the_complete_entry_plan() {
    let main_path = write_project(
        "interrupt-entry",
        INTERRUPT_POLICY,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("interrupt policy should compile");
    let restore = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptMaskGuard::restore")
        .expect("checked-in core restore requirement");
    assert!(restore.is_public);
    assert_eq!(
        restore.supply_mode,
        language_semantics::MachineSupplyMode::TopLevelRequirement
    );
    assert!(!restore.body_is_present);
    assert!(!restore.service_reach_is_installation_bound);
    let restore_snapshot = checked
        .typed
        .snapshot()
        .roots
        .machines
        .into_iter()
        .find(|machine| machine.name == "InterruptMaskGuard::restore")
        .expect("restore requirement snapshot");
    assert_eq!(restore_snapshot.service_reach, ["MachineControl"]);
    let [restore_contract] = restore_snapshot.contracts.as_slice() else {
        panic!("restore requirement must retain only its Active precondition")
    };
    assert_eq!(restore_contract.kind, "requires");
    let [typed_trees::snapshot::ProofFactSnapshot::Membership { domain, value, .. }] =
        restore_contract.facts.as_slice()
    else {
        panic!("restore requirement must retain one membership precondition")
    };
    assert_eq!(domain, &["InterruptMaskGuard", "Active"]);
    assert!(matches!(
        value,
        typed_trees::snapshot::ExpressionSnapshot::Name { path }
            if path == &["self"]
    ));
    let complete = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("checked-in core completion requirement");
    assert!(complete.is_public);
    assert_eq!(
        complete.supply_mode,
        language_semantics::MachineSupplyMode::TopLevelRequirement
    );
    assert!(!complete.body_is_present);
    assert!(!complete.service_reach_is_installation_bound);
    let complete_snapshot = checked
        .typed
        .snapshot()
        .roots
        .machines
        .into_iter()
        .find(|machine| machine.name == "InterruptAcknowledgement::complete")
        .expect("completion requirement snapshot");
    assert_eq!(complete_snapshot.service_reach, ["PortIo"]);
    let [complete_contract] = complete_snapshot.contracts.as_slice() else {
        panic!("completion requirement must retain only its Pending precondition")
    };
    assert_eq!(complete_contract.kind, "requires");
    let [typed_trees::snapshot::ProofFactSnapshot::Membership { domain, value, .. }] =
        complete_contract.facts.as_slice()
    else {
        panic!("completion requirement must retain one membership precondition")
    };
    assert_eq!(domain, &["InterruptAcknowledgement", "Pending"]);
    assert!(matches!(
        value,
        typed_trees::snapshot::ExpressionSnapshot::Name { path }
            if path == &["self"]
    ));
    let selection = retained_interrupt_representation(&checked);
    let timer_entry = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "TimerProvider::enter")
        .expect("timer adapter");
    let timer_entry = checked
        .typed
        .machine_states(timer_entry)
        .first()
        .expect("timer entry state");
    let acknowledgement = checked
        .typed
        .state_parameters(timer_entry)
        .first()
        .expect("timer acknowledgement parameter");
    let acknowledgement_layout = layout::layout_type_reference(
        &checked,
        target::NativeTarget::host(),
        checked.opaque_representation_selections(),
        acknowledgement.type_reference,
    )
    .expect("selected opaque representation must supply general by-value layout");
    assert_eq!(acknowledgement_layout.size, 40);
    assert_eq!(acknowledgement_layout.alignment, 8);
    let demanded_realizations = checked
        .boundary_calling_plan_realizations()
        .iter()
        .filter(|realization| {
            realization
                .materialized_signature()
                .opaque_representation_uses()
                .iter()
                .any(|use_| use_.opaque() == selection.opaque())
        })
        .collect::<Vec<_>>();
    assert!(
        !demanded_realizations.is_empty(),
        "the by-value boundary crossing must retain its exact representation use"
    );
    assert!(demanded_realizations.iter().all(|realization| {
        realization
            .materialized_signature()
            .opaque_representation_uses()
            .iter()
            .filter(|use_| use_.opaque() == selection.opaque())
            .all(|use_| {
                use_.conformance() == selection.application().declaration
                    && use_.carrier() == selection.carrier()
                    && usize::from(use_.shape_root())
                        < realization.materialized_signature().shapes().len()
                    && use_.application_report_fingerprint()
                        == selection.application().report_fingerprint
                    && use_.conformance_application_commitment()
                        == selection.application().commitment.as_bytes()
                    && use_.representation_schema_version() == selection.schema_version()
                    && use_.origin() == selection.origin()
                    && use_.lifecycle() == selection.lifecycle()
                    && use_.copy_disposition() == selection.copy_disposition()
                    && use_.selected_application_commitment()
                        == selection.selected_application_commitment()
                    && use_.rederived_selected_application_commitment()
                        == use_.selected_application_commitment()
            })
            && realization.replayed_validated_application().is_ok_and(
                |(_, report_fingerprint, commitment)| {
                    report_fingerprint == realization.report_fingerprint
                        && commitment == realization.commitment
                },
            )
    }));
    for realization in &demanded_realizations {
        let (validated, _, _) = realization
            .replayed_validated_application()
            .expect("opaque boundary use must replay its exact validated plan");
        for representation in realization
            .materialized_signature()
            .opaque_representation_uses()
            .iter()
            .filter(|use_| use_.opaque() == selection.opaque())
        {
            let movement = realization
                .materialized_signature()
                .opaque_representation_movement(representation, &validated)
                .expect("opaque shape node must rejoin one exact ABI placement");
            assert!(matches!(
                movement.role(),
                provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationMovementRole::Parameter { .. }
            ));
            assert_eq!(movement.placement().shape.byte_size, 40);
            assert_eq!(movement.placement().shape.alignment, 8);
            assert!(!movement.placement().locations.is_empty());
        }
    }

    let timer = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "TimerRoot")
        .expect("TimerRoot boundary trait");
    let schema = provider_planning::service_schema::from_typed(&checked.typed, timer)
        .expect("TimerRoot service schema");
    assert!(schema.methods[0].calling_plan_report_fingerprint.is_some());
    let selected = selected_plan_for_external_root(checked.selected_provider_plans(), "TimerRoot");
    assert_eq!(selected.name, "TimerProvider::satisfies::TimerRoot");
    let mask_plan =
        selected_plan_for_external_root(checked.selected_provider_plans(), "InterruptMaskControl");
    assert_eq!(
        mask_plan.name,
        "MaskProvider::satisfies::InterruptMaskControl"
    );
    let [mask_save] = mask_plan.schema.methods.as_slice() else {
        panic!("mask provider must publish one save-and-mask requirement");
    };
    let [active] = mask_save.result_claims.as_slice() else {
        panic!("mask transition must publish one structured Active result claim");
    };
    assert_eq!(active.domain, "InterruptMaskGuard::Active");
    assert_eq!(
        active.effective_carry,
        language_semantics::CarryPolicy::STRICT
    );
    let mut missing_active = mask_plan.clone();
    missing_active.schema.methods[0].result_claims.clear();
    assert_ne!(
        mask_plan.report_fingerprint(),
        missing_active.report_fingerprint(),
        "the mask provider receipt must bind its routed Active result claim"
    );
    let mask_selection = selected_external_root_provider_plan(
        checked.selected_provider_plans(),
        "InterruptMaskControl",
    )
    .expect("mask transition bridge should retain its selected provider plan");
    let runtime_active = mask_selection
        .result_claims(&mask_save.requirement_identity)
        .expect("exact Active result claim should lower into the runtime receipt contract");
    let [runtime_active] = runtime_active.as_slice() else {
        panic!("runtime mask bridge must retain one Active claim");
    };
    assert_eq!(runtime_active.provider_plan, mask_selection.identity);
    assert_eq!(
        runtime_active.requirement_identity,
        mask_save.requirement_identity
    );
    assert_eq!(runtime_active.domain, "InterruptMaskGuard::Active");
    let lookalike_plan =
        selected_plan_for_external_root(checked.selected_provider_plans(), "LookalikeMaskControl");
    assert_eq!(
        lookalike_plan.name,
        "LookalikeMaskProvider::satisfies::LookalikeMaskControl"
    );
    let [lookalike_save] = lookalike_plan.schema.methods.as_slice() else {
        panic!("look-alike provider must publish one requirement");
    };
    assert!(
        lookalike_save.result_claims.is_empty(),
        "a requirement not named by the domain route must not publish Active issuance authority"
    );
    assert_eq!(selected.rows.len(), 1);
    let [entry] = selected.schema.methods.as_slice() else {
        panic!("TimerRoot must inherit one exact core entry requirement");
    };
    assert_eq!(entry.name, "enter");
    assert_eq!(entry.requirement_owner, "InterruptEntry");
    assert!(entry.requirement_identity.contains("InterruptEntry"));
    let entry_reach = checked
        .selected_provider_plans()
        .installation_reach_resolution(&entry.requirement_identity)
        .expect("the installed interrupt entry must resolve its bounded reach row");
    assert_eq!(
        entry_reach.upper_bound,
        ["MachineControl".to_owned(), "PortIo".to_owned()]
    );
    assert_eq!(
        entry_reach.resolved_row,
        ["PortIo".to_owned()],
        "the PIC-shaped test provider refines the conservative hardware ceiling to PortIo"
    );
    let [acknowledgement] = entry.parameter_type_identities.as_slice() else {
        panic!("timer root must bind its acknowledgement parameter identity");
    };
    assert!(acknowledgement.contains("InterruptAcknowledgement"));
    assert!(acknowledgement.contains("Pending"));
    let [pending] = entry.entry_claims.as_slice() else {
        panic!("timer root must publish one structured accepted authority claim");
    };
    assert_eq!(pending.parameter_index, 0);
    assert_eq!(pending.domain, "InterruptAcknowledgement::Pending");
    assert_eq!(
        pending.predicate_body,
        language_semantics::DomainPredicateBody::Bodyless
    );
    assert_eq!(
        pending.effective_carry,
        language_semantics::CarryPolicy::STRICT
    );
    assert_eq!(
        pending.authority_flow,
        effects::provider_plan::ServiceEntryAuthorityFlow::Accepts
    );
    let mut weakened = selected.clone();
    weakened.schema.methods[0].parameter_type_identities[0] = "InterruptAcknowledgement".to_owned();
    assert_ne!(
        selected.report_fingerprint(),
        weakened.report_fingerprint(),
        "the provider-plan identity carried into external-root admission must drift if Pending issuance is removed"
    );
    let mut unreported = selected.clone();
    unreported.schema.methods[0].entry_claims.clear();
    assert_ne!(
        selected.report_fingerprint(),
        unreported.report_fingerprint(),
        "the provider-plan receipt must bind the compiler-owned accepted-authority row"
    );
    let root_selection =
        selected_external_root_provider_plan(checked.selected_provider_plans(), "TimerRoot")
            .expect("external-root bridge should retain the qualified timer schema");
    assert_eq!(
        root_selection.identity.normalized_identity(),
        selected.report_fingerprint()
    );
    assert_eq!(
        root_selection.schema.methods[0].parameter_type_identities,
        selected.schema.methods[0].parameter_type_identities,
        "the root bridge must carry the exact qualified source signature beside its receipt identity"
    );
    assert_eq!(
        root_selection.schema.methods[0].entry_claims, selected.schema.methods[0].entry_claims,
        "the root bridge must carry the structured accepted claim and strict carry policy beside its receipt identity"
    );
    let runtime_claims = root_selection
        .entry_claims(&entry.requirement_identity)
        .expect("exact timer entry claims should lower into the runtime ledger");
    let [runtime_pending] = runtime_claims.as_slice() else {
        panic!("runtime root bridge must retain one Pending claim");
    };
    assert_eq!(runtime_pending.parameter_index, 0);
    assert_eq!(runtime_pending.domain, "InterruptAcknowledgement::Pending");
    assert_eq!(
        runtime_pending.effective_carry,
        language_semantics::CarryPolicy::STRICT
    );
    let entry_fact_bindings = selected_external_root_entry_fact_bindings(
        &checked,
        checked.selected_provider_plans(),
        "TimerRoot",
    )
    .expect("installed root must bind its accepted claim to one checked entry fact");
    let [entry_fact] = entry_fact_bindings.as_slice() else {
        panic!("TimerRoot must bind one checked Pending parameter fact");
    };
    assert_eq!(entry_fact.provider_plan(), root_selection.identity);
    assert_eq!(
        entry_fact.requirement_identity(),
        entry.requirement_identity
    );
    assert_eq!(entry_fact.parameter_index(), 0);
    assert_eq!(entry_fact.domain(), "InterruptAcknowledgement::Pending");
    assert_eq!(
        entry_fact.parameter_symbol(),
        checked.typed.state_parameters(
            checked
                .typed
                .machine_states(
                    checked
                        .typed
                        .machines()
                        .iter()
                        .find(|machine| machine.name.as_str() == "TimerProvider::enter")
                        .expect("timer adapter"),
                )
                .first()
                .expect("timer entry state"),
        )[0]
        .symbol
    );
    assert_eq!(
        checked
            .facts
            .semantic
            .facts
            .get(entry_fact.checked_fact())
            .evidence
            .origin,
        language_semantics::QualificationEvidenceOrigin::Propagated,
        "the checked adapter fact remains an ordinary parameter precondition until occurrence admission"
    );
    let mut drifted_checked = checked.clone().into_program();
    drifted_checked
        .facts
        .semantic
        .facts
        .get_mut(entry_fact.checked_fact())
        .evidence
        .origin = language_semantics::QualificationEvidenceOrigin::CheckedTransformation;
    assert!(
        selected_external_root_entry_fact_bindings(
            &drifted_checked,
            checked.selected_provider_plans(),
            "TimerRoot",
        )
        .expect_err("a non-precondition fact must not satisfy installed-root entry binding")
        .0
        .contains("maps to 0 checked entry facts")
    );
    let lookalike_entry_plan =
        selected_plan_for_external_root(checked.selected_provider_plans(), "LookalikeEntry");
    assert_eq!(
        lookalike_entry_plan.name,
        "LookalikeEntryProvider::satisfies::LookalikeEntry"
    );
    let [lookalike_entry] = lookalike_entry_plan.schema.methods.as_slice() else {
        panic!("look-alike entry provider must publish one requirement");
    };
    assert!(
        lookalike_entry.entry_claims.is_empty(),
        "a qualified parameter whose requirement is not named by the domain route must remain an ordinary precondition"
    );
    assert!(
        selected_external_root_entry_fact_bindings(
            &checked,
            checked.selected_provider_plans(),
            "LookalikeEntry",
        )
        .expect("a look-alike root has no routed entry bindings")
        .is_empty()
    );
    assert_eq!(
        selected_external_root_provider_plan_id(checked.selected_provider_plans(), "TimerRoot")
            .expect("external-root bridge should retain the selected timer plan")
            .normalized_identity(),
        selected.report_fingerprint()
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn selected_opaque_representation_supplies_nested_general_layout() {
    let source = INTERRUPT_POLICY.replace(
        "data Main { }",
        "data InterruptEnvelope { acknowledgement: InterruptAcknowledgement }\n\ndata Main { }",
    );
    let main_path = write_project(
        "interrupt-nested-layout",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("selected opaque representation should close nested layout");
    let layouts = layout::build_layout_plan(
        &checked,
        target::NativeTarget::host(),
        checked.opaque_representation_selections(),
    )
    .expect("general layout should consume the selected carrier");
    let envelope = layouts
        .data_layouts
        .iter()
        .map(|(_, layout)| layout)
        .find(|layout| layout.name.as_str() == "InterruptEnvelope")
        .expect("nested opaque envelope layout");
    assert_eq!(envelope.layout.size, 40);
    assert_eq!(envelope.layout.alignment, 8);
    let layout::DataShape::Record { fields } = envelope.shape else {
        panic!("interrupt envelope should remain a semantic record")
    };
    let [acknowledgement] = layouts.fields.span_or_empty(fields) else {
        panic!("interrupt envelope should retain one semantic field")
    };
    assert_eq!(
        acknowledgement.type_name.as_ref(),
        "InterruptAcknowledgement"
    );
    assert_eq!(acknowledgement.layout.size, 40);
    assert_eq!(acknowledgement.layout.alignment, 8);
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn opaque_result_rejoins_its_exact_result_placement() {
    let source = format!("{INTERRUPT_POLICY}\n{INTERRUPT_OPAQUE_RESULT_POLICY}");
    let main_path = write_project(
        "interrupt-opaque-result-movement",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("opaque result policy should compile");
    let selection = retained_interrupt_representation(&checked);
    let mut result_movements = 0;

    for realization in checked.boundary_calling_plan_realizations() {
        let (validated, _, _) = realization
            .replayed_validated_application()
            .expect("opaque result plan should replay exactly");
        for representation in realization
            .materialized_signature()
            .opaque_representation_uses()
            .iter()
            .filter(|representation| representation.opaque() == selection.opaque())
        {
            let movement = realization
                .materialized_signature()
                .opaque_representation_movement(representation, &validated)
                .expect("opaque result must rejoin one exact result placement");
            if movement.role() != BoundaryOpaqueRepresentationMovementRole::Result {
                continue;
            }
            result_movements += 1;
            assert!(movement.path().is_empty());
            assert_eq!(
                realization.materialized_signature().result(),
                Some(representation.shape_root())
            );
            assert_eq!(movement.placement().shape.byte_size, 40);
            assert_eq!(movement.placement().shape.alignment, 8);
            assert!(matches!(
                movement.placement().locations.as_slice(),
                [calling_conventions::ValueLocation::Indirect { .. }]
            ));
        }
    }

    assert_eq!(
        result_movements, 1,
        "the authored result boundary should retain one opaque result movement"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn nested_opaque_path_ignores_an_identically_shaped_ordinary_field() {
    let source = interrupt_envelope_policy(
        "    ordinary: PicAckCarrier;\n    acknowledgement: InterruptAcknowledgement;",
        "",
    );
    let main_path = write_project(
        "interrupt-nested-opaque-movement",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("nested opaque representation policy should compile");
    let selection = retained_interrupt_representation(&checked);
    let mut nested_movements = 0;

    for realization in checked.boundary_calling_plan_realizations() {
        let (validated, _, _) = realization
            .replayed_validated_application()
            .expect("nested opaque plan should replay exactly");
        for representation in realization
            .materialized_signature()
            .opaque_representation_uses()
            .iter()
            .filter(|representation| representation.opaque() == selection.opaque())
        {
            let movement = realization
                .materialized_signature()
                .opaque_representation_movement(representation, &validated)
                .expect("nested opaque must rejoin one exact parameter placement");
            if movement.path()
                != [BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 1 }]
            {
                continue;
            }
            nested_movements += 1;
            assert!(matches!(
                movement.role(),
                BoundaryOpaqueRepresentationMovementRole::Parameter {
                    formal_ordinal: 0,
                    native_ordinal: 0,
                }
            ));
            assert_eq!(movement.placement().shape.byte_size, 80);
            assert_eq!(movement.placement().shape.alignment, 8);

            let [parameter_root] = realization.materialized_signature().parameters() else {
                panic!("nested boundary should retain one semantic parameter")
            };
            let root = realization.materialized_signature().shapes()[usize::from(*parameter_root)];
            let BoundaryValueClass::Record {
                first_field,
                field_count: 2,
            } = root.class()
            else {
                panic!("nested boundary parameter should remain a two-field record")
            };
            let fields = &realization.materialized_signature().fields()
                [usize::from(first_field)..usize::from(first_field) + 2];
            let ordinary_root = fields[0].shape();
            let opaque_root = fields[1].shape();
            assert_eq!(opaque_root, representation.shape_root());
            assert_ne!(ordinary_root, representation.shape_root());
            let ordinary =
                realization.materialized_signature().shapes()[usize::from(ordinary_root)];
            let opaque = realization.materialized_signature().shapes()[usize::from(opaque_root)];
            assert_eq!(ordinary.byte_size(), opaque.byte_size());
            assert_eq!(ordinary.alignment(), opaque.alignment());
            assert_eq!(
                realization
                    .materialized_signature()
                    .opaque_representation_uses()
                    .iter()
                    .filter(|candidate| candidate.opaque() == selection.opaque())
                    .count(),
                1,
                "the equal ordinary carrier field must not be marked as opaque"
            );
        }
    }

    assert_eq!(
        nested_movements, 1,
        "the authored envelope should retain one exact nested opaque movement"
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn repeated_opaque_values_rejoin_distinct_equal_layout_occurrences() {
    let source = interrupt_envelope_policy(
        "    first: InterruptAcknowledgement;\n    second: InterruptAcknowledgement;",
        "",
    );
    let main_path = write_project(
        "interrupt-repeated-opaque-movement",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("repeated opaque representation policy should compile");
    let selection = retained_interrupt_representation(&checked);
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| {
            realization
                .materialized_signature()
                .opaque_representation_uses()
                .iter()
                .filter(|representation| representation.opaque() == selection.opaque())
                .count()
                == 2
        })
        .expect("one boundary signature with two exact opaque occurrences");
    let (validated, _, _) = realization
        .replayed_validated_application()
        .expect("repeated opaque plan should replay exactly");
    let uses = realization
        .materialized_signature()
        .opaque_representation_uses()
        .iter()
        .filter(|representation| representation.opaque() == selection.opaque())
        .collect::<Vec<_>>();
    let [first, second] = uses.as_slice() else {
        panic!("two exact repeated opaque markers")
    };
    assert_ne!(first.shape_root(), second.shape_root());
    let first_shape =
        realization.materialized_signature().shapes()[usize::from(first.shape_root())];
    let second_shape =
        realization.materialized_signature().shapes()[usize::from(second.shape_root())];
    assert_eq!(first_shape.byte_size(), second_shape.byte_size());
    assert_eq!(first_shape.alignment(), second_shape.alignment());

    let movements = uses
        .iter()
        .map(|representation| {
            realization
                .materialized_signature()
                .opaque_representation_movement(representation, &validated)
                .expect("each repeated marker must rejoin its own occurrence")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        movements
            .iter()
            .map(|movement| movement.path())
            .collect::<Vec<_>>(),
        [
            &[BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 0 }][..],
            &[BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 1 }][..],
        ]
    );
    assert!(movements.iter().all(|movement| {
        matches!(
            movement.role(),
            BoundaryOpaqueRepresentationMovementRole::Parameter {
                formal_ordinal: 0,
                native_ordinal: 0,
            }
        ) && movement.placement().shape.byte_size == 80
    }));
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn distinct_opaque_values_with_equal_layout_retain_distinct_nominal_markers() {
    let source = interrupt_envelope_policy(
        "    acknowledgement: InterruptAcknowledgement;\n    shadow: ShadowAcknowledgement;",
        "pub boundary data ShadowAcknowledgement;\n\ndata ShadowAckCarrier {\n    physical_root: u64;\n    execution: u64;\n    invocation: u64;\n    policy: u64;\n    acknowledgement: u64;\n}\n\nShadowAckRepresentation:\n    ShadowAckCarrier satisfies OpaqueRepresentation<ShadowAcknowledgement>;",
    );
    let build = INTERRUPT_REPRESENTATION_BUILD.replace(
        "    >();",
        "    >();\n    builder.select_representation<\n        ShadowAcknowledgement,\n        ShadowAckRepresentation\n    >();",
    );
    let main_path = write_project("interrupt-equal-opaque-movement", &source, &build);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("equal-layout opaque representation policy should compile");
    let opaque_symbols = ["InterruptAcknowledgement", "ShadowAcknowledgement"].map(|name| {
        checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name)
            .unwrap_or_else(|| panic!("exact `{name}` opaque declaration"))
            .symbol
    });
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| {
            opaque_symbols.iter().all(|opaque| {
                realization
                    .materialized_signature()
                    .opaque_representation_uses()
                    .iter()
                    .any(|representation| representation.opaque() == *opaque)
            })
        })
        .expect("one boundary signature carrying both opaque declarations");
    let (validated, _, _) = realization
        .replayed_validated_application()
        .expect("equal-layout opaque plan should replay exactly");
    let uses = opaque_symbols.map(|opaque| {
        realization
            .materialized_signature()
            .opaque_representation_uses()
            .iter()
            .find(|representation| representation.opaque() == opaque)
            .expect("one exact nominal opaque marker")
    });
    assert_ne!(uses[0].opaque(), uses[1].opaque());
    assert_ne!(uses[0].carrier(), uses[1].carrier());
    assert_ne!(uses[0].shape_root(), uses[1].shape_root());
    let shapes = uses.map(|representation| {
        realization.materialized_signature().shapes()[usize::from(representation.shape_root())]
    });
    assert_eq!(shapes[0].byte_size(), shapes[1].byte_size());
    assert_eq!(shapes[0].alignment(), shapes[1].alignment());
    let movements = uses.map(|representation| {
        realization
            .materialized_signature()
            .opaque_representation_movement(representation, &validated)
            .expect("equal layout must not substitute one nominal marker for another")
    });
    assert_eq!(
        movements[0].path(),
        [BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 0 }]
    );
    assert_eq!(
        movements[1].path(),
        [BoundaryOpaqueRepresentationPathElement::RecordField { ordinal: 1 }]
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary policy directory"));
}

#[test]
fn opaque_by_value_boundary_rejects_without_build_selection() {
    let rendered = compile_project_negative(
        "interrupt-missing-representation",
        INTERRUPT_POLICY,
        "machine build(builder: &mut Build) { builder.application(\"interrupt-entry\"); }",
    );
    assert!(
        rendered.contains("InterruptAcknowledgement")
            && rendered.contains("authoritative build selects no exact"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn reference_only_opaque_boundary_retains_unused_selection_without_demanding_one() {
    let source = INTERRUPT_POLICY
        .replace(
            "boundary trait TimerRoot: InterruptEntry + Calling<X86InterruptPolicy> {",
            "boundary trait TimerRoot: Calling<X86InterruptPolicy> {\n    machine inspect(acknowledgement: &InterruptAcknowledgement);",
        )
        .replace("signature.shapes[root].byte_size == 40", "signature.shapes[root].byte_size == 8")
        .replace(
            "machine enter(acknowledgement: InterruptAcknowledgement in Pending)\n    reaches PortIo;",
            "machine enter(acknowledgement: &InterruptAcknowledgement);",
        )
        .replace(
            "machine TimerProvider::enter(acknowledgement: InterruptAcknowledgement in Pending)\n    satisfies InterruptEntry::enter\n    reaches PortIo\n{\n    acknowledgement.complete();\n}",
            "machine TimerProvider::inspect(acknowledgement: &InterruptAcknowledgement)\n    satisfies TimerRoot::inspect\n{\n}",
        )
        .replace(
            "machine LookalikeEntryProvider::enter(acknowledgement: InterruptAcknowledgement in Pending)\n    satisfies LookalikeEntry::enter\n    reaches PortIo\n{\n    acknowledgement.complete();\n}",
            "machine LookalikeEntryProvider::enter(acknowledgement: &InterruptAcknowledgement)\n    satisfies LookalikeEntry::enter\n{\n}",
        )
        .replace(
            "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
            "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;\n\nAlternatePicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
        );
    let unselected = write_project(
        "interrupt-reference-only-unselected-representation",
        &source,
        "machine build(builder: &mut Build) { builder.application(\"interrupt-entry\"); }",
    );
    let unselected = compile_to_checked(CheckedCompileRequest::new(&unselected, None))
        .expect("a reference-only opaque pointee must not demand representation closure");
    assert!(unselected.opaque_representation_selections().is_empty());
    let inspect = unselected
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "TimerProvider::inspect")
        .expect("reference-only timer adapter");
    let inspect = unselected
        .typed
        .machine_states(inspect)
        .first()
        .expect("reference-only entry state");
    let acknowledgement = unselected
        .typed
        .state_parameters(inspect)
        .first()
        .expect("reference-only acknowledgement");
    let reference_layout = layout::layout_type_reference(
        &unselected,
        target::NativeTarget::host(),
        unselected.opaque_representation_selections(),
        acknowledgement.type_reference,
    )
    .expect("reference layout must not demand an opaque representation");
    assert_eq!(
        reference_layout.size,
        target::NativeTarget::host().pointer_size
    );
    assert_eq!(
        reference_layout.alignment,
        target::NativeTarget::host().pointer_alignment
    );
    let typed_trees::types::TypeReferenceNode::Reference { referee, .. } = unselected
        .typed
        .type_reference_table
        .type_reference(acknowledgement.type_reference)
    else {
        panic!("reference-only acknowledgement should retain its referee")
    };
    let diagnostic = layout::layout_type_reference(
        &unselected,
        target::NativeTarget::host(),
        unselected.opaque_representation_selections(),
        *referee,
    )
    .expect_err("the same opaque requested by value must require a selection");
    assert!(
        diagnostic
            .message
            .contains("requires one exact representation")
    );

    let selected = write_project(
        "interrupt-reference-only-selected-representation",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    let selected = compile_to_checked(CheckedCompileRequest::new(&selected, None))
        .expect("an unused valid selection remains activation policy");
    let selected_application = retained_interrupt_representation(&selected).application();
    assert!(
        selected
            .boundary_calling_plan_realizations()
            .iter()
            .all(|realization| realization
                .materialized_signature()
                .opaque_representation_uses()
                .is_empty())
    );

    let alternate_build = INTERRUPT_REPRESENTATION_BUILD
        .replace("PicAckRepresentation", "AlternatePicAckRepresentation");
    let alternate = write_project(
        "interrupt-reference-only-alternate-representation",
        &source,
        &alternate_build,
    );
    let alternate = compile_to_checked(CheckedCompileRequest::new(&alternate, None))
        .expect("an alternate unused valid selection remains activation policy");
    let [alternate_selection] = alternate.opaque_representation_selections() else {
        panic!("one alternate unused opaque-representation selection")
    };
    assert_eq!(
        selected.selected_provider_plans(),
        alternate.selected_provider_plans(),
        "an unused representation selection must not change calling-plan settlement"
    );
    assert_ne!(
        selected_application.commitment,
        alternate_selection.application().commitment,
        "unused selection custody must retain the exact closed application identity"
    );
    assert_ne!(
        selected, alternate,
        "unused opaque-representation policy must participate in checked semantic equality"
    );
}

#[test]
fn opaque_representation_rejects_duplicate_build_selection() {
    let duplicate = INTERRUPT_REPRESENTATION_BUILD.replace(
        "    >();",
        "    >();\n    builder.select_representation<InterruptAcknowledgement, PicAckRepresentation>();",
    );
    let rendered = compile_project_negative(
        "interrupt-duplicate-representation",
        INTERRUPT_POLICY,
        &duplicate,
    );
    assert!(
        rendered.contains("selects opaque representation") && rendered.contains("more than once"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn opaque_representation_rejects_conformance_for_another_opaque_type() {
    let source = INTERRUPT_POLICY.replace(
        "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
        "pub boundary data OtherAcknowledgement [linear];\n\nPicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<OtherAcknowledgement>;",
    );
    let rendered = compile_project_negative(
        "interrupt-mismatched-representation",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    assert!(
        rendered.contains("represents `OtherAcknowledgement`")
            && rendered.contains("InterruptAcknowledgement"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn opaque_representation_rejects_a_trait_lookalike() {
    let source = INTERRUPT_POLICY.replace(
        "PicAckRepresentation:\n    PicAckCarrier satisfies OpaqueRepresentation<InterruptAcknowledgement>;",
        "trait RepresentationLookalike<Opaque> { }\n\nPicAckRepresentation:\n    PicAckCarrier satisfies RepresentationLookalike<InterruptAcknowledgement>;",
    );
    let rendered = compile_project_negative(
        "interrupt-lookalike-representation",
        &source,
        INTERRUPT_REPRESENTATION_BUILD,
    );
    assert!(
        rendered.contains("does not satisfy the exact compiler-owned")
            && rendered.contains("OpaqueRepresentation"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn changing_the_selected_opaque_conformance_reissues_the_calling_application() {
    fn timer_calling_application(checked: &compiler::CheckedCompilation) -> u64 {
        let timer = checked
            .typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "TimerRoot")
            .expect("TimerRoot boundary trait");
        provider_planning::service_schema::from_typed(&checked.typed, timer)
            .expect("TimerRoot service schema")
            .methods[0]
            .calling_plan_report_fingerprint
            .expect("TimerRoot calling application")
    }

    let first = compile_to_checked(CheckedCompileRequest::new(
        &write_project(
            "interrupt-representation-identity-first",
            INTERRUPT_POLICY,
            INTERRUPT_REPRESENTATION_BUILD,
        ),
        None,
    ))
    .expect("first representation application");
    let alternate_source =
        INTERRUPT_POLICY.replace("PicAckRepresentation:", "AlternatePicAckRepresentation:");
    let alternate_build = INTERRUPT_REPRESENTATION_BUILD
        .replace("PicAckRepresentation", "AlternatePicAckRepresentation");
    let alternate = compile_to_checked(CheckedCompileRequest::new(
        &write_project(
            "interrupt-representation-identity-alternate",
            &alternate_source,
            &alternate_build,
        ),
        None,
    ))
    .expect("alternate representation application");

    let first_selection = retained_interrupt_representation(&first);
    let [alternate_selection] = alternate.opaque_representation_selections() else {
        panic!("one alternate opaque-representation selection")
    };
    assert_ne!(
        first_selection.application().commitment,
        alternate_selection.application().commitment,
        "the retained closed application commitment must change with its named conformance"
    );
    assert_ne!(
        timer_calling_application(&first),
        timer_calling_application(&alternate),
        "an exact named-conformance substitution must reissue the calling application even when carrier shape is unchanged"
    );
}

#[test]
fn source_policy_receives_signature_and_publishes_only_validated_acceptance() {
    let main_path = write_program("accepted", POLICY);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("policy program should compile");

    let validated = evaluate_calling_policy_plan(
        &checked.typed,
        "NoResultPolicy::plan",
        &CallSignature::default(),
    )
    .expect("empty signature should be accepted");

    assert_eq!(validated.plan().call.policy, CallingPolicy::MicrosoftX64);
    assert_eq!(validated.plan().call.stack_alignment, 16);
    assert_ne!(validated.contract_report_fingerprint(), 0);

    let tick = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Tick")
        .expect("Tick boundary trait");
    let schema = provider_planning::service_schema::from_typed(&checked.typed, tick)
        .expect("Tick service schema");
    assert_eq!(schema.methods.len(), 1);
    let application_report = schema.methods[0]
        .calling_plan_report_fingerprint
        .expect("boundary method must publish its complete calling-plan application");
    assert_ne!(
        application_report,
        validated.contract_report_fingerprint(),
        "the retained application must bind target and semantic signature beside the accepted policy output"
    );
    let retained = checked
        .typed
        .boundary_calling_plans
        .iter()
        .find(|identity| identity.report_fingerprint == application_report)
        .expect("typed semantic identity for the published boundary contract");
    assert_ne!(retained.report_fingerprint, 0);
    assert_ne!(
        retained.commitment.as_bytes(),
        validated.contract_commitment_digest(),
        "raw policy acceptance is not the complete target-closed application"
    );
}

#[test]
fn source_policy_rejection_preserves_the_authored_reason() {
    let main_path = write_program("rejected", POLICY);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("policy program should compile");

    let error = evaluate_calling_policy_plan(
        &checked.typed,
        "NoResultPolicy::plan",
        &CallSignature {
            parameters: Vec::new(),
            result: Some(ValueShape::integer(8, 8)),
        },
    )
    .expect_err("return-bearing signature should be rejected");

    assert!(error.contains("calling policy rejected the boundary"));
    assert!(error.contains("return values are not supported"));
}

#[test]
fn full_width_unsigned_calling_values_are_not_reinterpreted_as_signed() {
    let source = r#"
use omega::language::std::calling;

data FullWidthPolicy { }
FullWidthPolicyCallingPolicy: FullWidthPolicy satisfies CallingPolicy;
machine FullWidthPolicy::plan(
    signature: BoundarySignature
) -> BoundaryPlanResult
    satisfies CallingPolicy::plan
{
    let mut output: BoundaryEntryPlan;
    output.call.stack_alignment = 18446744073709551615;
    BoundaryPlanResult::Accepted { plan: output }
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let main_path = write_program("full-width-value", source);
    let checked = compile_to_checked(CheckedCompileRequest::new(&main_path, None))
        .expect("full-width u64 policy should compile");
    let error = evaluate_calling_policy_plan(
        &checked.typed,
        "FullWidthPolicy::plan",
        &CallSignature::default(),
    )
    .expect_err("the normalized u16 alignment must reject a full-width source value");

    assert!(
        error.contains("stack_alignment 18446744073709551615 is outside u16 range"),
        "unexpected diagnostic: {error}"
    );
}

/// `BOUNDED-INSTALLATION-REACH-ROWS` keeps one rejection load-bearing: a
/// selected realization that itself reaches through an unresolved
/// installation-bound requirement must not publish a resolved row, because
/// its checked reach is that requirement's conservative bound and provider
/// selection has no nested substitution step. Provider planning owns the
/// check; this pins it from authored source through a compiled project, so
/// the fence cannot be lost when the shipped completion route migrates to
/// `reaches <= MachineControl + PortIo`.
#[test]
fn selected_realization_with_an_unresolved_installation_bound_row_rejects() {
    let source = r#"
boundary trait MachineControl { }
boundary trait PortIo { }
boundary trait Storage { }

pub data Endpoint { }
pub boundary requirement Endpoint::step() reaches <= Storage;

boundary trait InterruptCompletion {
    machine complete() -> u64
    reaches <= MachineControl + PortIo + Storage;
}

data Pic { }
PicInterruptCompletion: Pic satisfies InterruptCompletion;

machine Pic::complete() -> u64
    satisfies InterruptCompletion::complete
    reaches PortIo + Storage
{
    Endpoint::step();
    0
}

data Main { }
machine Main::main(&mut self) { }
"#;
    let rendered = compile_project_negative(
        "unresolved-installation-reach",
        source,
        "machine build(builder: &mut Build) { builder.application(\"unresolved-reach\"); }",
    );
    assert!(
        rendered.contains("retains 1 unresolved installation-bound requirement"),
        "unexpected diagnostics:\n{rendered}"
    );
}
