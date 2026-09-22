use super::{
    CALLBACK_MATERIALIZATION_POLICY, bundled_standard_library_root, callback_fixture_source,
    compile_std_negative, write_callback_package, write_program,
};
use compiler::{CheckedCompileRequest, compile_to_checked};
use package_compilation::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding,
};
use provider_planning::calling_policy_plans::BoundaryOpaqueRepresentationMovementRole;
use semantic_vocabulary::PackageKeyIdentity;
use std::fs;

#[test]
fn calling_vocabulary_is_public_across_package_boundaries() {
    let main = write_program(
        "public-calling-vocabulary",
        r#"use policy::calling;
data Consumer { signature: BoundarySignature; plan: BoundaryPlanResult; }
data RejectingPolicy {}
RejectingPolicyConformance: RejectingPolicy satisfies CallingPolicy;
machine RejectingPolicy::plan(signature: BoundarySignature) -> BoundaryPlanResult
satisfies CallingPolicy::plan
{
    BoundaryPlanResult::Rejected {
        reason: CallingPolicyRejection { reason: "unsupported signature" },
    }
}
"#,
    );
    let root = main.parent().expect("consumer directory").to_path_buf();
    let dependency = write_program("public-calling-dependency", "")
        .parent()
        .expect("policy directory")
        .to_path_buf();
    fs::copy(
        bundled_standard_library_root().join("calling.omg"),
        dependency.join("calling.omg"),
    )
    .expect("copy public calling vocabulary");
    let consumer = PackageKeyIdentity::from_digest([74; 32]).expect("consumer identity");
    let policy = PackageKeyIdentity::from_digest([75; 32]).expect("policy identity");
    let inputs = PackageCompilationInputs::new_package(
        consumer,
        vec![
            PackageSourceBinding::new(consumer, "consumer", root),
            PackageSourceBinding::new(policy, "policy", dependency),
        ],
        vec![PackageDependencyBinding::new(consumer, "policy", policy)],
    )
    .expect("two-package policy graph");
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&main, Some("uefi_x86_64"))
    })
    .expect("a dependent package can author a policy using the public calling vocabulary");
}

#[test]
fn target_selected_callback_policy_consumes_two_closed_layout_demands() {
    let source = callback_fixture_source("callback_materialization_closure.omg");
    assert_eq!(
        source.trim_start_matches('\n'),
        CALLBACK_MATERIALIZATION_POLICY.trim_start_matches('\n'),
        "the source canary and its readable test fixture must agree apart from leading blank lines"
    );
    let (main_path, package_inputs) =
        write_callback_package("materialization-closure", CALLBACK_MATERIALIZATION_POLICY);
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&main_path, Some("windows_x86_64"))
    })
    .expect("target-selected registrar should consume both exact closed layout demands");
    let registrar = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "WindowRegistrar")
        .expect("exact registrar declaration");
    let register = checked
        .typed
        .trait_machine_signatures(registrar)
        .iter()
        .find(|signature| signature.name.as_str() == "register")
        .expect("exact registrar requirement");
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| realization.requirement_machine == register.symbol)
        .expect("retained registrar calling-plan realization");
    let catalog = realization
        .materialized_signature()
        .callback_layout_catalog();
    assert_eq!(catalog.len(), 2);
    assert!(
        catalog
            .windows(2)
            .all(|pair| pair[0].destination() < pair[1].destination())
    );
    let recorded = checked
        .typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_name == "Spread<ForeignRecord>")
        .expect("exact named plan-laid registrar parameter");
    let closed = layout::build_layout_plan(
        &checked,
        target::NativeTarget::windows_x64(),
        checked.opaque_representation_selections(),
    )
    .expect("independent target-closed layout catalog");
    let root = closed
        .plan_laid_layout_identities
        .iter()
        .find(|layout| layout.data_symbol == recorded.data_symbol)
        .expect("target-closed named root layout");
    assert_eq!(root.physical.size, 24);
    assert_eq!(root.physical.alignment, 8);
    let mut destinations = realization.callback_demands.iter().collect::<Vec<_>>();
    destinations.sort_by(|left, right| left.destination.cmp(&right.destination));
    assert_eq!(destinations.len(), catalog.len());
    for (entry, demanded) in catalog.iter().zip(destinations) {
        assert_eq!(entry.formal_ordinal(), 0);
        assert_eq!(entry.native_ordinal(), 0);
        assert_eq!(entry.root_layout(), root);
        assert!(entry.inline_field().is_none());
        assert_eq!(entry.destination(), &demanded.destination);
        let terminal = entry.terminal_slot();
        let typed_demand = recorded
            .private_callback_demands
            .iter()
            .find(|demand| demand.slot_identity == terminal.slot_identity.as_ref())
            .expect("each catalog entry rejoins one exact typed slot");
        assert_eq!(terminal.slot_application, typed_demand.slot_application);
        assert_eq!(
            terminal.callback_requirement_identity.as_ref(),
            typed_demand.callback_requirement_identity
        );
        assert_eq!(
            terminal.layout_subject_identity.as_ref(),
            typed_demand.layout_subject_identity
        );
        assert_eq!(terminal.data_symbol, recorded.data_symbol);
        assert_eq!(
            terminal.offset,
            usize::try_from(typed_demand.offset).unwrap()
        );
        assert_eq!(terminal.byte_size, 8);
        assert_eq!(terminal.alignment, 8);
        assert_eq!(entry.composed_offset(), terminal.offset);
        assert_eq!(terminal.requirement, demanded.requirement);
        let calling_conventions::NativePlace::Field {
            parameter,
            layout,
            field_path,
        } = entry.destination()
        else {
            panic!("plan-laid callback destination must name its root and slot")
        };
        assert_eq!(*layout, terminal.layout);
        assert_eq!(root.data_symbol, terminal.data_symbol);
        assert_eq!(
            root.layout_subject_identity,
            terminal.layout_subject_identity
        );
        assert_eq!(field_path, &[terminal.slot]);
        assert_eq!(terminal.native_demand(*parameter), *demanded);
        assert_eq!(
            realization
                .boundary_entry_plan
                .call
                .callback_materializations
                .iter()
                .filter(|materialization| &materialization.destination == entry.destination())
                .count(),
            1,
        );
    }
    let mut offsets = catalog
        .iter()
        .map(|entry| entry.composed_offset())
        .collect::<Vec<_>>();
    offsets.sort_unstable();
    assert_eq!(offsets, [8, 16]);
    assert_ne!(
        catalog[0].terminal_slot().slot_application,
        catalog[1].terminal_slot().slot_application
    );
    let (_, report, commitment) = realization
        .replayed_validated_application()
        .expect("catalog preserves validated calling application");
    assert_eq!(report, realization.report_fingerprint);
    assert_eq!(commitment, realization.commitment);
}

#[test]
fn target_selected_callback_policy_retains_inline_child_layout_catalog() {
    let source = CALLBACK_MATERIALIZATION_POLICY
        .replace(
            "specification: &Spread<ForeignRecord>",
            "specification: &Envelope<Registration>",
        )
        .replace(
            "data Main { }",
            r#"
data Envelope { entries: [FieldEntry; 64]; }

machine Envelope::plan(&mut self, schema: Schema) -> Plan {
    let mut owned_entries: [FieldEntry; 64];
    owned_entries[0] = FieldEntry {
        key: schema.fields[0].key,
        placement: FieldPlan::At { offset: 8 },
    };
    Plan {
        entries: owned_entries,
        entry_count: 1,
        size_fixed: 32,
        size_is_dynamic: false,
        align: 8,
    }
}

data Registration { callbacks: Spread<ForeignRecord>; }
data Main { }
"#,
        );
    let (main_path, package_inputs) = write_callback_package("inline-callback-catalog", &source);
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&main_path, Some("windows_x86_64"))
    })
    .expect("registrar should consume both slots through its named inline child");
    let registrar = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "WindowRegistrar")
        .unwrap();
    let register = checked
        .typed
        .trait_machine_signatures(registrar)
        .iter()
        .find(|signature| signature.name.as_str() == "register")
        .unwrap();
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| realization.requirement_machine == register.symbol)
        .expect("retained nested registrar calling-plan realization");
    let catalog = realization
        .materialized_signature()
        .callback_layout_catalog();
    let closed = layout::build_layout_plan(
        &checked,
        target::NativeTarget::windows_x64(),
        checked.opaque_representation_selections(),
    )
    .expect("independently closed root-field-child-slot paths");
    let recorded_root = checked
        .typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_name == "Envelope<Registration>")
        .unwrap();
    let recorded_child = checked
        .typed
        .plan_laid_layouts
        .iter()
        .find(|layout| layout.data_name == "Spread<ForeignRecord>")
        .unwrap();
    assert_eq!(catalog.len(), 2);
    assert_eq!(realization.callback_demands.len(), 2);
    assert_eq!(closed.two_hop_private_callback_paths.len(), 2);
    assert!(catalog[0].destination() < catalog[1].destination());
    for entry in catalog {
        let terminal = entry.terminal_slot();
        let field = entry
            .inline_field()
            .expect("one retained named inline field");
        let path = closed
            .two_hop_private_callback_paths
            .iter()
            .find(|path| path.terminal_demand.slot_application == terminal.slot_application)
            .expect("exact independently closed slot application");
        let typed_demand = recorded_child
            .private_callback_demands
            .iter()
            .find(|demand| demand.slot_identity == terminal.slot_identity.as_ref())
            .unwrap();
        assert_eq!(terminal.slot_application, typed_demand.slot_application);
        assert_eq!(entry.formal_ordinal(), 0);
        assert_eq!(entry.native_ordinal(), 0);
        assert_eq!(entry.root_layout(), &path.root_layout);
        assert_eq!(entry.root_layout().data_symbol, recorded_root.data_symbol);
        assert_eq!(entry.root_layout().physical.size, 32);
        assert_eq!(entry.root_layout().physical.alignment, 8);
        assert_eq!(field.symbol(), path.field_symbol);
        assert_eq!(field.identity(), path.field_identity.as_ref());
        assert_eq!(field.offset(), 8);
        assert_eq!(field.extent(), 24);
        assert_eq!(field.alignment(), 8);
        assert_eq!(field.child_layout(), &path.child_layout);
        assert_eq!(field.child_layout().data_symbol, recorded_child.data_symbol);
        assert_eq!(field.child_layout().physical.size, 24);
        assert_eq!(field.child_layout().physical.alignment, 8);
        assert_eq!(terminal, &path.terminal_demand);
        assert_eq!(entry.composed_offset(), field.offset() + terminal.offset);
        assert_eq!(entry.composed_offset(), path.composed_offset);
        let calling_conventions::NativePlace::Field {
            parameter,
            layout,
            field_path,
        } = entry.destination()
        else {
            panic!("inline child callback must retain its named field path")
        };
        assert_eq!(*layout, path.root_layout.layout);
        assert_eq!(field_path, &[path.field_slot, terminal.slot]);
        let demand = path.native_demand(*parameter);
        assert_eq!(entry.destination(), &demand.destination);
        assert_eq!(
            realization
                .callback_demands
                .iter()
                .filter(|row| **row == demand)
                .count(),
            1,
        );
        assert_eq!(
            realization
                .boundary_entry_plan
                .call
                .callback_materializations
                .iter()
                .filter(|row| &row.destination == entry.destination())
                .count(),
            1,
        );
    }
    let mut offsets = catalog
        .iter()
        .map(|entry| entry.composed_offset())
        .collect::<Vec<_>>();
    offsets.sort_unstable();
    assert_eq!(offsets, [16, 24]);
    let (_, report, commitment) = realization.replayed_validated_application().unwrap();
    assert_eq!(report, realization.report_fingerprint);
    assert_eq!(commitment, realization.commitment);
}

#[test]
fn direct_callback_parameter_is_interleaved_without_a_source_runtime_argument() {
    let source = callback_fixture_source("direct_callback_parameter.omg");
    let (main_path, package_inputs) = write_callback_package("direct-callback", &source);
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&main_path, Some("windows_x86_64"))
    })
    .expect("target closure should place the declared direct callback parameter");
    let registrar = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str().ends_with("HookRegistrar"))
        .expect("HookRegistrar boundary trait");
    let install = checked
        .typed
        .trait_machine_signatures(registrar)
        .iter()
        .find(|signature| signature.name.as_str() == "install")
        .expect("HookRegistrar::install requirement");
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| realization.requirement_machine == install.symbol)
        .expect("direct callback registrar realization");
    assert!(
        realization
            .materialized_signature()
            .callback_layout_catalog()
            .is_empty(),
        "a native-only callback parameter cannot invent a plan-laid root or slot",
    );
    assert_eq!(realization.callback_demands.len(), 1);
    assert_eq!(
        checked.typed.state_signature_parameters(install).len(),
        2,
        "the native-only callback must not become a source runtime parameter"
    );
    let [callback] = install.native_callback_parameters.as_slice() else {
        panic!("one exact native-only callback declaration must survive typed lowering");
    };
    assert_eq!(callback.name.as_str(), "procedure");
    assert_eq!(callback.binder.as_str(), "Handler");
    assert_eq!(callback.native_ordinal, 1);
    let [nominal_use] = checked.facts.nominal_machine_uses.uses.as_slice() else {
        panic!("the actual registrar occurrence must retain one selected callback use");
    };
    assert!(matches!(
        nominal_use.site,
        checked_trees::NominalMachineUseSite::Statement(_)
    ));
    let [placement] = checked.callback_placements() else {
        panic!("the actual registrar occurrence must bind one callback placement");
    };
    assert_eq!(placement.site, nominal_use.site);
    assert_eq!(placement.registration_operation, install.symbol);
    let materialization = placement
        .private_materialization
        .as_ref()
        .expect("the direct callback must retain its target-closed native parameter");
    assert!(matches!(
        materialization.destination,
        calling_conventions::NativePlace::Parameter(_)
    ));
    assert_eq!(
        placement.boundary_entry_plan.call.parameters.len(),
        1,
        "the selected callback's inbound ABI stays distinct from the registrar ABI",
    );
    assert_eq!(
        materialization
            .registrar_boundary_entry_plan
            .call
            .parameters
            .len(),
        3
    );
    let application = materialization
        .direct_registrar_parameter_application
        .as_ref()
        .expect("direct callback retains one exact target-closed telescope row");
    let calling_conventions::NativePlace::Parameter(destination) = &materialization.destination
    else {
        unreachable!();
    };
    assert_eq!(application.parameter, *destination);
    assert_eq!(application.native_ordinal, 1);
    assert_eq!(
        application.shape,
        calling_conventions::ValueShape::integer(8, 8)
    );
    assert_eq!(
        application.placement,
        materialization
            .registrar_boundary_entry_plan
            .call
            .parameters[1]
    );

    let mutations: [(
        &str,
        fn(&mut calling_conventions::NativeParameterApplication),
    ); 3] = [
        (
            "identity",
            |application: &mut calling_conventions::NativeParameterApplication| {
                application.parameter =
                    calling_conventions::NativeParameterId::new(0xdead).unwrap();
            },
        ),
        ("ordinal", |application| application.native_ordinal = 0),
        ("shape", |application| {
            application.shape = calling_conventions::ValueShape::float(8);
        }),
    ];
    for (label, mutate) in mutations {
        let mut drifted = placement.clone();
        let application = drifted
            .private_materialization
            .as_mut()
            .unwrap()
            .direct_registrar_parameter_application
            .as_mut()
            .unwrap();
        mutate(application);
        assert!(
            backend_plan::validate_bound_nominal_callback_placement(&drifted).is_err(),
            "{label} drift must reject independently",
        );
    }
    let mut missing_application = placement.clone();
    missing_application
        .private_materialization
        .as_mut()
        .unwrap()
        .direct_registrar_parameter_application = None;
    assert!(
        backend_plan::validate_bound_nominal_callback_placement(&missing_application).is_err(),
        "a direct destination cannot lose its target-closed application",
    );
    let mut placement_drift = placement.clone();
    placement_drift
        .private_materialization
        .as_mut()
        .unwrap()
        .direct_registrar_parameter_application
        .as_mut()
        .unwrap()
        .placement = materialization
        .registrar_boundary_entry_plan
        .call
        .parameters[0]
        .clone();
    assert!(
        backend_plan::validate_bound_nominal_callback_placement(&placement_drift).is_err(),
        "physical placement substitution must reject independently",
    );

    let requirement_identity = checked
        .typed
        .normalized_hermetic_symbol_identity(install.symbol)
        .expect("exact registrar requirement identity");
    let mut stale_v1_parameter = placement.clone();
    stale_v1_parameter
        .private_materialization
        .as_mut()
        .unwrap()
        .direct_registrar_parameter_application
        .as_mut()
        .unwrap()
        .parameter = calling_conventions::callback_native_parameter_id(&requirement_identity, 1);
    assert!(
        backend_plan::validate_bound_nominal_callback_placement(&stale_v1_parameter).is_err(),
        "the retired ordinal-derived v1 parameter identity must not substitute for v2",
    );

    let mut invented_parameter = placement.clone();
    let invented = calling_conventions::NativeParameterId::new(0xbeef).unwrap();
    let materialization = invented_parameter.private_materialization.as_mut().unwrap();
    materialization.destination = calling_conventions::NativePlace::Parameter(invented);
    materialization
        .direct_registrar_parameter_application
        .as_mut()
        .unwrap()
        .parameter = invented;
    assert!(
        backend_plan::validate_bound_nominal_callback_placement(&invented_parameter).is_err(),
        "a locally coherent policy-created parameter must not enter the declared telescope",
    );

    let mut wrong_binder = placement.clone();
    wrong_binder
        .private_materialization
        .as_mut()
        .unwrap()
        .binder = calling_conventions::StaticMachineBinderId::new(0xcafe).unwrap();
    assert!(
        backend_plan::validate_bound_nominal_callback_placement(&wrong_binder).is_err(),
        "a different binder identity must not retain the direct application",
    );

    let mut wrong_requirement = placement.clone();
    wrong_requirement
        .private_materialization
        .as_mut()
        .unwrap()
        .requirement = calling_conventions::CallbackRequirementId::new(0xfade).unwrap();
    assert!(
        backend_plan::validate_bound_nominal_callback_placement(&wrong_requirement).is_err(),
        "a different callback requirement must not retain the direct application",
    );
}

#[test]
fn opaque_movement_retains_native_ordinal_after_direct_callback_insertion() {
    let source = callback_fixture_source("direct_callback_parameter.omg")
        .replace(
            "use calling;",
            "use calling;\nuse omega::language::core::representation;\n\npub boundary data CallbackToken;\npub data CallbackTokenCarrier { value: u64; }\npub CallbackTokenRepresentation:\n    CallbackTokenCarrier satisfies OpaqueRepresentation<CallbackToken>;",
        )
        .replace("module: u64", "module: CallbackToken")
        .replace(
            "data Main { }\nmachine Main::main(&mut self) {\n    HookRegistrar::install<HookProvider::call>(1u64, 2u64);\n}",
            "data Main { }\nmachine Main::register(&mut self, module: CallbackToken) {\n    HookRegistrar::install<HookProvider::call>(1u64, module);\n}\nmachine Main::main(&mut self) { }",
        );
    let (main_path, package_inputs) =
        write_callback_package("opaque-direct-callback-ordinal", &source);
    fs::write(
        main_path
            .parent()
            .expect("callback package directory")
            .join("build.omg"),
        r#"machine build(builder: &mut Build) {
    builder.package("opaque-direct-callback-ordinal");
    builder.select_representation<CallbackToken, CallbackTokenRepresentation>();
}
"#,
    )
    .expect("write callback opaque-representation selection");
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&main_path, Some("windows_x86_64"))
    })
    .expect("opaque registrar parameter should close around the direct callback");
    let opaque = checked
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "CallbackToken")
        .expect("exact callback-token opaque declaration");
    let realization = checked
        .boundary_calling_plan_realizations()
        .iter()
        .find(|realization| {
            realization
                .materialized_signature()
                .opaque_representation_uses()
                .iter()
                .any(|representation| representation.opaque() == opaque.symbol)
                && realization
                    .replayed_validated_application()
                    .is_ok_and(|(validated, _, _)| validated.plan().call.parameters.len() == 3)
        })
        .expect("one closed registrar signature with its compiler-inserted callback");
    let representation = realization
        .materialized_signature()
        .opaque_representation_uses()
        .iter()
        .find(|representation| representation.opaque() == opaque.symbol)
        .expect("one exact callback-token occurrence");
    let (validated, _, _) = realization
        .replayed_validated_application()
        .expect("callback-interleaved opaque plan should replay exactly");
    let movement = realization
        .materialized_signature()
        .opaque_representation_movement(representation, &validated)
        .expect("opaque occurrence must rejoin after direct callback insertion");
    assert!(matches!(
        movement.role(),
        BoundaryOpaqueRepresentationMovementRole::Parameter {
            formal_ordinal: 1,
            native_ordinal: 2,
        }
    ));
    assert!(movement.path().is_empty());
    assert_eq!(movement.placement(), &validated.plan().call.parameters[2]);
    assert!(matches!(
        movement.placement().locations.as_slice(),
        [calling_conventions::ValueLocation::Register {
            register: calling_conventions::MachineRegister::X86R8,
            ..
        }]
    ));
    let _ = fs::remove_dir_all(
        main_path
            .parent()
            .expect("temporary callback package directory"),
    );
}

#[test]
fn direct_callback_parameter_requires_a_bodyless_boundary_requirement() {
    let source = callback_fixture_source("direct_callback_parameter.omg")
        .replace("boundary trait HookRegistrar", "trait HookRegistrar");
    let (main_path, package_inputs) =
        write_callback_package("direct-callback-nonboundary", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&main_path, Some("windows_x86_64"))
    })
    .expect_err("a non-boundary trait cannot declare a native callback parameter");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("permitted only on a bodyless boundary-trait requirement")
    }));
}

#[test]
fn direct_callback_parameter_requires_its_exact_nominal_binder() {
    let source = callback_fixture_source("direct_callback_parameter.omg").replace(
        "native callback procedure from Handler",
        "native callback procedure from Missing",
    );
    let (main_path, package_inputs) =
        write_callback_package("direct-callback-missing-binder", &source);
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(package_inputs),
        ..CheckedCompileRequest::new(&main_path, Some("windows_x86_64"))
    })
    .expect_err("a direct callback cannot infer or invent its binder");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unknown machine binder `Missing`")
    }));
}

#[test]
fn direct_callback_parameter_rejects_inferred_duplicate_and_unconstrained_binders() {
    let source = callback_fixture_source("direct_callback_parameter.omg");
    for (name, mutated, expected) in [
        (
            "inferred-binder",
            source.replace(
                "native callback procedure from Handler",
                "native callback procedure",
            ),
            "from",
        ),
        (
            "duplicate-name",
            source.replace(
                "native callback procedure from Handler,\n        module",
                "native callback procedure from Handler,\n        native callback procedure from Handler,\n        module",
            ),
            "native callback parameter `procedure` is declared more than once",
        ),
        (
            "duplicate-binder",
            source.replace(
                "native callback procedure from Handler,\n        module",
                "native callback procedure from Handler,\n        native callback backup from Handler,\n        module",
            ),
            "is assigned to more than one declared native callback parameter",
        ),
        (
            "unconstrained-binder",
            source.replace(
                "    )\n    where machine Handler satisfies HookProcedure::call;",
                "    );",
            ),
            "requires an authored declaration-site contract",
        ),
    ] {
        let rendered = compile_std_negative(name, &mutated);
        assert!(
            rendered.contains(expected),
            "{name} produced unexpected diagnostics:\n{rendered}",
        );
    }
}

#[test]
fn authored_addr_parameter_cannot_substitute_for_a_native_callback_declaration() {
    let source = callback_fixture_source("direct_callback_parameter.omg")
        .replace("native callback procedure from Handler", "procedure: addr");
    let rendered = compile_std_negative("authored-addr", &source);
    assert!(
        rendered.contains("invalid direct callback telescope"),
        "authored addr substitution produced unexpected diagnostics:\n{rendered}",
    );
}

#[test]
fn callback_private_materialization_requires_an_explicit_cited_demand() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "    let placed: Plan =\n        Plan::place_private<WndClassWindowProcedureSlot>(plan, 8);\n    Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 16)",
        "    plan",
    );
    let rendered = compile_std_negative("callback-uncited-demand", &source);

    assert!(
        rendered.contains("omits a nominal callback binder"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_a_foreign_layout_subject() {
    let source = CALLBACK_MATERIALIZATION_POLICY
        .replace("data Spread {}", "data Spread {}\n\ndata OtherSpread {}")
        .replace(
            "Spread satisfies PrivateCallbackSlot<WindowProcedure::call>;",
            "OtherSpread satisfies PrivateCallbackSlot<WindowProcedure::call>;",
        );
    let rendered = compile_std_negative("callback-wrong-layout", &source);

    assert!(
        rendered.contains("active layout producer") && rendered.contains("OtherSpread"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_an_ambiguous_requirement_path() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "boundary trait WindowProcedure {\n    machine call(message: u64) -> u64;\n}",
        "boundary trait WindowProcedure {\n    machine call(message: u64) -> u64;\n    machine call(message: i64) -> u64;\n}",
    );
    let rendered = compile_std_negative("callback-ambiguous-requirement", &source);

    assert!(
        rendered.contains("overloads requirement `call`")
            && rendered.contains("WindowProcedure::call"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_the_wrong_callback_requirement() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "WndClassWindowProcedureSlot:\n    Spread satisfies PrivateCallbackSlot<WindowProcedure::call>;",
        "WndClassWindowProcedureSlot:\n    Spread satisfies PrivateCallbackSlot<UnusedProcedure::call>;",
    );
    let rendered = compile_std_negative("wrong-requirement", &source);

    assert!(
        rendered.contains(
            "callback materialization binder and native-place demand require different callback contracts"
        ),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_a_raw_physical_offset_as_identity() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "        bound.call.callback_materializations[0].destination =\n            signature.callback_demands[0].destination;",
        "        let mut invented_path: [u64; 16];\n        invented_path[0] = 8;\n        bound.call.callback_materializations[0].destination = NativePlace::Field {\n            parameter: 8,\n            layout: 8,\n            field_path: invented_path,\n            field_path_count: 1,\n        };",
    );
    let rendered = compile_std_negative("raw-offset", &source);

    assert!(
        rendered.contains("does not name a declared private native-place demand"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_duplicate_source_placement() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 16)",
        "Plan::place_private<WndClassWindowProcedureSlot>(placed, 16)",
    );
    let rendered = compile_std_negative("duplicate-placement", &source);

    assert!(
        rendered.contains("duplicate private callback placement")
            || rendered.contains("more than once"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_overlapping_named_slots() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 16)",
        "Plan::place_private<SecondaryWndClassWindowProcedureSlot>(placed, 8)",
    );
    let rendered = compile_std_negative("overlapping-slots", &source);

    assert!(
        rendered.contains("private callback slots") && rendered.contains("overlap"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_is_absent_from_semantic_projection() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "data ForeignRecord {",
        "machine Spread::read_private_slot(&mut self) -> u64 {\n    self.WndClassWindowProcedureSlot\n}\n\ndata ForeignRecord {",
    );
    let rendered = compile_std_negative("semantic-projection", &source);

    assert!(
        rendered.contains("Spread")
            && rendered.contains("has no field")
            && rendered.contains("WndClassWindowProcedureSlot"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_is_absent_from_semantic_assignment() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "data ForeignRecord {",
        "machine Spread::write_private_slot(&mut self) {\n    self.WndClassWindowProcedureSlot = 0;\n}\n\ndata ForeignRecord {",
    );
    let rendered = compile_std_negative("semantic-assignment", &source);

    assert!(
        rendered.contains("Spread")
            && rendered.contains("has no field")
            && rendered.contains("WndClassWindowProcedureSlot"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn callback_private_materialization_rejects_a_machine_as_slot_identity() {
    let source = CALLBACK_MATERIALIZATION_POLICY.replace(
        "Plan::place_private<WndClassWindowProcedureSlot>(plan, 8)",
        "Plan::place_private<WindowProcedure::call>(plan, 8)",
    );
    let rendered = compile_std_negative("machine-as-slot", &source);

    assert!(
        rendered.contains(
            "static argument to `Plan::place_private` must resolve exactly to one named conformance"
        ),
        "unexpected diagnostics:\n{rendered}"
    );
}
