use super::{
    bind_selected_provider_plan_facts_for_test, derive_provider_fixture,
    normalized_machine_identity, selected_operator_binding_fixture, selection_plan,
};
use crate::provider_planning::{
    Arc, ProviderBinding, ProviderPlanDerivation, ProviderPlanRow, ProviderSchemaDeclaration,
    SelectedTargetMachineOrigin, bind_selected_provider_plan_facts, derive_satisfies_plans,
    select_derived_provider_plans, select_provider_plans, selected_provider_plan_facts,
    validate_derived_provider_plan_candidates, validate_provider_plan_candidates,
};

#[test]
fn exact_requirement_lifetime_partition_is_stable_across_checked_and_external_supply() {
    let checked_source = r#"
        boundary trait Pair<'left, 'right> {
            machine consume(first: &'left u64, second: &'right u64) reaches Pair;
        }

        machine consume<'unused, 'x, 'y>(first: &'x u64, second: &'y u64)
            satisfies Pair<'x, 'y>::consume
        {
        }
    "#;
    let external_source = r#"
        boundary trait Pair<'left, 'right> {
            machine consume(first: &'left u64, second: &'right u64) reaches Pair;
        }

        machine consume<'y, 'unused, 'x>(first: &'x u64, second: &'y u64)
            satisfies Pair<'x, 'y>::consume
            via Binding::Syscall(60);
    "#;
    let (checked_typed, checked_plan) = derive_provider_fixture(checked_source);
    let (external_typed, external_plan) = derive_provider_fixture(external_source);
    typed_trees_to_checked_trees::lower_typed_trees(checked_typed.clone())
        .expect("checked exact realization");
    typed_trees_to_checked_trees::lower_typed_trees(external_typed.clone())
        .expect("external exact realization");

    assert_eq!(checked_plan.rows[0].requirement_lifetime_partition, [0, 1]);
    assert_eq!(
        checked_plan.rows[0].requirement_lifetime_partition,
        external_plan.rows[0].requirement_lifetime_partition,
        "private realizer binder order and supply mode are outside edge identity",
    );

    let mut tampered = checked_plan;
    tampered.rows[0].requirement_lifetime_partition = vec![0, 0];
    let diagnostics = validate_provider_plan_candidates(&checked_typed, &[tampered]);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("requirement lifetime partition differs from its exact realization edge")
    }));
}

#[test]
fn derives_and_selects_checked_top_level_boundary_requirement_provider() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        pub data InterruptAcknowledgement [copy] { token: u64; }
        pub domain InterruptAcknowledgement::Pending;
        pub data LapicCompletion {}

        pub boundary requirement InterruptAcknowledgement::complete(self)
        reaches <= MachineControl + PortIo
        requires self in InterruptAcknowledgement::Pending;

        machine LapicCompletion::complete(
            acknowledgement: InterruptAcknowledgement in Pending
        )
        satisfies InterruptAcknowledgement::complete
        reaches MachineControl
        {
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize top-level requirement fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse top-level requirement fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve top-level requirement fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type top-level requirement fixture");
    let requirement = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("typed top-level requirement");
    let provider = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "LapicCompletion::complete")
        .expect("typed checked provider");
    let provider_type = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "LapicCompletion")
        .expect("nominal provider type");

    let derived = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None));
    let [derived] = derived.as_slice() else {
        panic!("one exact top-level provider plan, got {}", derived.len())
    };
    assert_eq!(
        derived.provenance.schema,
        ProviderSchemaDeclaration::BoundaryRequirement(requirement.symbol)
    );
    assert_eq!(derived.provenance.provider_type, Some(provider_type.symbol));
    assert_eq!(derived.provenance.row_requirements, [requirement.symbol]);
    assert_eq!(derived.provenance.row_realizations, [provider.symbol]);
    let requirement_identity = typed
        .normalized_machine_overload_identity(requirement)
        .expect("normalized requirement identity")
        .identity();
    assert_eq!(
        derived.plan.rows[0].requirement_identity,
        requirement_identity
    );
    assert!(matches!(
        &derived.plan.rows[0].binding,
        ProviderBinding::CheckedAdapter { machine_identity, .. }
            if machine_identity == &normalized_machine_identity(&typed, "LapicCompletion::complete")
    ));
    let validation = validate_provider_plan_candidates(&typed, std::slice::from_ref(&derived.plan));
    assert!(
        validation.is_empty(),
        "the derived exact plan must replay against typed declarations: {validation:?}"
    );

    let selection = crate::ProviderSelection {
        subject: crate::ProviderSelectionSubject::BoundaryRequirement(
            crate::ProviderSelectionIdentity {
                symbol: requirement.symbol,
                package: typed.symbols.symbol_package_identity(requirement.symbol),
                canonical_path: derived.plan.schema.trait_name.clone(),
                authored_path: "InterruptAcknowledgement::complete".to_owned(),
            },
        ),
        provider_type: crate::ProviderSelectionIdentity {
            symbol: provider_type.symbol,
            package: typed.symbols.symbol_package_identity(provider_type.symbol),
            canonical_path: derived.plan.provider_type.clone(),
            authored_path: "LapicCompletion".to_owned(),
        },
        composition_mode: crate::CompositionMode::Fused,
        selecting_machine: symbols::SymbolHandle::invalid(),
        source_span: source::SourceSpan::default(),
    };
    let selected = select_provider_plans(
        std::slice::from_ref(&derived.plan),
        target::NativeTarget::host(),
        &[],
        &[selection],
    )
    .expect("an explicit boundary-requirement selection chooses its declared candidate");
    assert_eq!(selected, std::slice::from_ref(&derived.plan));
}

#[test]
fn derives_and_selects_external_top_level_boundary_requirement_provider() {
    let source = r#"
        pub data InterruptAcknowledgement [copy] { token: u64; }
        pub data LinuxCompletion {}

        pub boundary requirement InterruptAcknowledgement::complete(self);

        machine LinuxCompletion::complete(
            acknowledgement: InterruptAcknowledgement
        )
        satisfies InterruptAcknowledgement::complete
        via Binding::Syscall(60);
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize external top-level requirement fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse external top-level requirement fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve external top-level requirement fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type external top-level requirement fixture");
    typed_trees_to_checked_trees::lower_typed_trees(typed.clone())
        .expect("the exact external satisfier should pass conformance validation");
    let evaluated_bindings = crate::evaluated_via_bindings::evaluate_via_bindings(
        &typed,
        Some(target::TargetProfile::LinuxX64),
        None,
    )
    .expect("legacy-only fixture has an exact empty evaluated-via table");

    let requirement = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "InterruptAcknowledgement::complete")
        .expect("typed top-level requirement");
    let provider = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "LinuxCompletion::complete")
        .expect("typed external provider");
    let provider_type = typed
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == "LinuxCompletion")
        .expect("nominal external provider type");
    let derived = derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(Some("linux_x86_64")),
    );
    let [derived] = derived.as_slice() else {
        panic!(
            "one exact external top-level provider plan, got {}",
            derived.len()
        )
    };

    assert_eq!(
        derived.provenance.schema,
        ProviderSchemaDeclaration::BoundaryRequirement(requirement.symbol)
    );
    assert_eq!(derived.provenance.provider_type, Some(provider_type.symbol));
    assert_eq!(derived.provenance.row_requirements, [requirement.symbol]);
    assert_eq!(derived.provenance.row_realizations, [provider.symbol]);
    assert_eq!(derived.plan.target, "linux_x86_64");
    assert!(matches!(
        derived.plan.rows.as_slice(),
        [ProviderPlanRow {
            binding: ProviderBinding::Syscall { number: 60 },
            ..
        }]
    ));
    let validation = validate_provider_plan_candidates(&typed, std::slice::from_ref(&derived.plan));
    assert!(
        validation.is_empty(),
        "the external plan must replay against its exact typed binding: {validation:?}"
    );

    let selection = crate::ProviderSelection {
        subject: crate::ProviderSelectionSubject::BoundaryRequirement(
            crate::ProviderSelectionIdentity {
                symbol: requirement.symbol,
                package: typed.symbols.symbol_package_identity(requirement.symbol),
                canonical_path: derived.plan.schema.trait_name.clone(),
                authored_path: "InterruptAcknowledgement::complete".to_owned(),
            },
        ),
        provider_type: crate::ProviderSelectionIdentity {
            symbol: provider_type.symbol,
            package: typed.symbols.symbol_package_identity(provider_type.symbol),
            canonical_path: derived.plan.provider_type.clone(),
            authored_path: "LinuxCompletion".to_owned(),
        },
        composition_mode: crate::CompositionMode::Fused,
        selecting_machine: symbols::SymbolHandle::invalid(),
        source_span: source::SourceSpan::default(),
    };
    let selected = select_provider_plans(
        std::slice::from_ref(&derived.plan),
        target::NativeTarget::linux_x64(),
        &[],
        &[selection],
    )
    .expect("an explicit selection chooses the declared external candidate");
    assert_eq!(selected, std::slice::from_ref(&derived.plan));

    let selected_with_provenance = select_derived_provider_plans(
        std::slice::from_ref(derived),
        target::NativeTarget::linux_x64(),
        &[],
        &[],
    )
    .expect("the unique external candidate retains exact selection provenance");
    selected_provider_plan_facts(
        &typed,
        &evaluated_bindings,
        selected_with_provenance.clone(),
    )
    .expect("selected facts must replay the external realization and binding");

    let mut drifted = derived.plan.clone();
    drifted.rows[0].binding = ProviderBinding::Syscall { number: 61 };
    let diagnostics = validate_provider_plan_candidates(&typed, &[drifted]);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("resolves to 0 exact typed external realizations")
        }),
        "a substituted external binding must not replay against source: {diagnostics:?}"
    );

    let mut drifted_binding_provenance = selected_with_provenance.clone();
    drifted_binding_provenance[0].derived.plan.rows[0].binding =
        ProviderBinding::Syscall { number: 61 };
    let diagnostics =
        selected_provider_plan_facts(&typed, &evaluated_bindings, drifted_binding_provenance)
            .expect_err("selected provenance must reject substituted external binding identity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("binding does not equal its exact typed realization replay")
    }));

    let mut drifted_realization_provenance = selected_with_provenance.clone();
    drifted_realization_provenance[0]
        .derived
        .provenance
        .row_realizations[0] = requirement.symbol;
    let diagnostics =
        selected_provider_plan_facts(&typed, &evaluated_bindings, drifted_realization_provenance)
            .expect_err("selected provenance must reject a substituted realization symbol");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not rejoin its exact nominal provider provenance")
    }));

    let mut drifted_requirement_provenance = selected_with_provenance;
    drifted_requirement_provenance[0]
        .derived
        .provenance
        .row_requirements[0] = provider.symbol;
    let diagnostics =
        selected_provider_plan_facts(&typed, &evaluated_bindings, drifted_requirement_provenance)
            .expect_err("selected provenance must reject a substituted requirement symbol");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("to 0 exact boundary declarations")
    }));
}

#[test]
fn provider_derivation_consumes_typed_external_binding_identity() {
    // The authored `Binding::DllImport("module", "symbol")` spelling is retired,
    // so `ExternalBindingIdentity::Import` no longer has a source producer. The
    // remaining bootstrap spellings still exercise the typed id/table join.
    let source = |number: i64| {
        format!(
            r#"
                boundary trait Process {{
                    machine exit(code: i32);
                }}

                machine exit_leaf(code: i32)
                satisfies Process::exit
                via Binding::Syscall({number});
            "#
        )
    };
    let retained_source = source(60);
    let retained_tokens = source_files_to_tokens::Lexer::new(&retained_source)
        .tokenize()
        .expect("tokenize retained binding");
    let retained_syntax = tokens_to_syntax_trees::parse_syntax_trees(&retained_tokens)
        .expect("parse retained binding");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&retained_syntax),
    )
    .expect("resolve retained binding");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type retained binding");

    // Derivation accepts no syntax tree: the exact typed id/table is its
    // only external-binding authority.
    let plans = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        panic!("one external provider plan")
    };

    assert_eq!(
        plan.rows[0].binding,
        ProviderBinding::Syscall { number: 60 }
    );

    // A typed program that still interns the retired string-backed import
    // identity is rejected where typed identities enter provider planning,
    // with the diagnostic that names the typed replacement.
    let mut retired = typed;
    retired
        .external_bindings
        .intern(language_semantics::ExternalBindingIdentity::Import {
            library: "kernel32.dll".to_owned(),
            symbol: "ExitProcess".to_owned(),
        });
    let evaluated_bindings =
        crate::evaluated_via_bindings::evaluate_via_bindings(&retired, None, None)
            .expect("no ordinary via leaf to evaluate");
    let diagnostics = ProviderPlanDerivation::evaluated(&retired, None, &evaluated_bindings, &[])
        .err()
        .expect("retired string-backed import identity must reject");
    assert_eq!(diagnostics.len(), 1);
    assert!(diagnostics[0].message.contains(
        "string-backed import bootstrap `kernel32.dll`/`ExitProcess` is retired; declare a typed locator"
    ));
}

#[test]
fn linux_console_exit_intrinsic_requires_selected_target_machine_origin() {
    let source = r#"
        boundary trait Console {
            machine write_byte(byte: i32);
            machine exit_process(return_code: i32);
        }

        data ConsoleNativeProvider {}
        boundary machine ConsoleNativeProvider::write_byte(byte: i32)
            satisfies Console::write_byte;
        boundary machine ConsoleNativeProvider::exit_process(return_code: i32)
            satisfies Console::exit_process;
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize source-inferred catalog leaf");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse source-inferred catalog leaf");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve source-inferred catalog leaf");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type source-inferred catalog leaf");

    assert!(
        derive_satisfies_plans(
            &typed,
            ProviderPlanDerivation::unevaluated(Some("linux_x86_64"))
        )
        .is_empty(),
        "an unscoped same-shaped boundary machine is not compiler-catalog authority",
    );
    let realization = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ConsoleNativeProvider::exit_process")
        .expect("typed exit realization");

    for target in ["linux_x86_64", "linux_arm64"] {
        let profile = match target {
            "linux_x86_64" => target::TargetProfile::LinuxX64,
            "linux_arm64" => target::TargetProfile::LinuxArm64,
            _ => unreachable!(),
        };
        let evaluated =
            crate::evaluated_via_bindings::evaluate_via_bindings(&typed, Some(profile), None)
                .expect("claim-free fixture has an empty evaluated-via table");
        let origin = SelectedTargetMachineOrigin {
            machine: realization.symbol,
            target: target.to_owned(),
        };
        let derived = ProviderPlanDerivation::evaluated(
            &typed,
            Some(target),
            &evaluated,
            std::slice::from_ref(&origin),
        )
        .map(|derivation| derive_satisfies_plans(&typed, derivation))
        .expect("exact selected target-machine origin derives");
        let [derived] = derived.as_slice() else {
            panic!("one exact inferred Linux Console plan for {target}")
        };
        assert_eq!(derived.plan.target, target);
        let [row] = derived.plan.rows.as_slice() else {
            panic!("only the migrated exit row may be inferred")
        };
        assert_eq!(row.method, "exit_process");
        assert!(matches!(
            row.binding,
            ProviderBinding::CompilerIntrinsic { .. }
        ));
        let retained_realization = typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == derived.provenance.row_realizations[0])
            .expect("retained inferred realization");
        assert_eq!(
            retained_realization.supply_mode,
            language_semantics::MachineSupplyMode::Boundary,
        );
        assert_eq!(
            derived.provenance.row_target_machine_origins,
            [Some(origin.clone())],
        );
        let [conformance] = typed.machine_trait_conformances(retained_realization) else {
            panic!("one inferred satisfies edge")
        };
        assert!(conformance.external_binding.is_none());
        assert!(!conformance.via_expression.is_valid());
        assert!(conformance.external_binding_source_span.is_none());

        let wrong_origin = SelectedTargetMachineOrigin {
            machine: realization.symbol,
            target: "windows_x86_64".to_owned(),
        };
        assert!(
            ProviderPlanDerivation::evaluated(&typed, Some(target), &evaluated, &[wrong_origin])
                .map(|derivation| derive_satisfies_plans(&typed, derivation))
                .expect("wrong origin is a closed candidate set")
                .is_empty(),
            "wrong-target declaration provenance must not infer the Linux row",
        );

        let mut tampered = derived.clone();
        tampered.provenance.row_target_machine_origins[0]
            .as_mut()
            .expect("inferred origin")
            .target = "windows_x86_64".to_owned();
        assert!(
            !validate_derived_provider_plan_candidates(&typed, &evaluated, &[tampered]).is_empty(),
            "retained target-machine origin substitution must reject during replay",
        );
    }
    let targetless_evaluated =
        crate::evaluated_via_bindings::evaluate_via_bindings(&typed, None, None)
            .expect("claim-free fixture has an empty targetless evaluated-via table");
    let linux_host_origin = SelectedTargetMachineOrigin {
        machine: realization.symbol,
        target: "linux_x86_64".to_owned(),
    };
    let targetless = ProviderPlanDerivation::evaluated(
        &typed,
        None,
        &targetless_evaluated,
        &[linux_host_origin],
    )
    .map(|derivation| derive_satisfies_plans(&typed, derivation))
    .expect("targetless Linux-host origin derives without inventing a plan target");
    let [targetless] = targetless.as_slice() else {
        panic!("one exact inferred targetless Linux Console plan")
    };
    assert_eq!(targetless.plan.target, "");
    assert!(
        derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
            .into_iter()
            .map(|derived| derived.plan)
            .collect::<Vec<_>>()
            .is_empty()
    );
    assert!(
        derive_satisfies_plans(
            &typed,
            ProviderPlanDerivation::unevaluated(Some("macos_arm64"))
        )
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>()
        .is_empty()
    );
}

#[test]
fn provider_derivation_rejects_incomplete_or_inconsistent_external_supply() {
    let source = r#"
        boundary trait Process {
            machine exit(code: i32);
        }

        machine exit_leaf(code: i32)
        satisfies Process::exit
        via Binding::Syscall(60);
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize external binding");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse external binding");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve external binding");
    let mut typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type external binding");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exit_leaf")
        .expect("external leaf");
    let language_semantics::MachineSupplyMode::ExternalRealization {
        binding: Some(binding),
        mechanism: Some(language_semantics::ExternalBindingMechanism::Syscall),
    } = machine.supply_mode
    else {
        panic!("legacy external binding must begin fully installed")
    };

    typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "exit_leaf")
        .expect("external leaf")
        .supply_mode = language_semantics::MachineSupplyMode::ExternalRealization {
        binding: Some(binding),
        mechanism: None,
    };
    assert!(
        derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
            .into_iter()
            .map(|derived| derived.plan)
            .collect::<Vec<_>>()
            .is_empty(),
        "an installed binding without its mechanism must not derive a provider plan",
    );

    typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "exit_leaf")
        .expect("external leaf")
        .supply_mode = language_semantics::MachineSupplyMode::ExternalRealization {
        binding: Some(binding),
        mechanism: Some(language_semantics::ExternalBindingMechanism::Import),
    };
    assert!(
        derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
            .into_iter()
            .map(|derived| derived.plan)
            .collect::<Vec<_>>()
            .is_empty(),
        "a mechanism that disagrees with the interned binding must not derive a provider plan",
    );
}

#[test]
fn provider_derivation_retains_every_exact_external_realization_symbol() {
    let source = r#"
        boundary trait Pair {
            machine first();
            machine second();
        }

        machine first_leaf()
        satisfies Pair::first
        via Binding::Syscall(60);

        machine second_leaf()
        satisfies Pair::second
        via Binding::Syscall(93);
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize two-row provider fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse two-row provider fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve two-row provider fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type two-row provider fixture");
    let derived = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None));
    let [derived] = derived.as_slice() else {
        panic!("one two-row external provider plan")
    };
    assert_eq!(derived.plan.rows.len(), 2);
    assert_eq!(derived.provenance.row_requirements.len(), 2);
    assert_eq!(derived.provenance.row_realizations.len(), 2);
    let pair = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Pair")
        .expect("Pair boundary trait");
    let expected_requirements = typed
        .trait_machine_signatures(pair)
        .iter()
        .map(|signature| signature.symbol)
        .collect::<Vec<_>>();
    assert_eq!(
        derived.provenance.row_requirements, expected_requirements,
        "provider rows retain their exact requirement declarations in schema order",
    );
    let expected = ["first_leaf", "second_leaf"]
        .into_iter()
        .map(|name| {
            typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == name)
                .unwrap_or_else(|| panic!("missing `{name}` machine"))
                .symbol
        })
        .collect::<Vec<_>>();
    assert_ne!(expected[0], expected[1]);
    assert!(
        expected
            .iter()
            .all(|symbol| derived.provenance.row_realizations.contains(symbol))
    );
}

#[test]
fn selected_provider_binds_actual_reach_for_bounded_requirement() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary trait InterruptCompletion {
            machine complete() -> u64
            reaches <= MachineControl + PortIo;
        }

        data Pic {}

        machine Pic::complete() -> u64
        satisfies InterruptCompletion::complete
        reaches PortIo
        {
            0
        }
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("bounded provider should check");
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("one selected PIC plan");
    let selected = bind_selected_provider_plan_facts_for_test(
        &mut checked,
        std::slice::from_ref(&plan),
        selected,
        &[],
    )
    .expect("selected PIC reach should resolve");
    let requirement_identity = plan.rows[0].requirement_identity.as_str();
    let resolution = selected
        .installation_reach_resolution(requirement_identity)
        .expect("bounded requirement resolution");

    assert_eq!(resolution.upper_bound, ["MachineControl", "PortIo"]);
    assert_eq!(resolution.resolved_row, ["PortIo"]);
    assert_eq!(
        resolution.provider_plan_report_identity,
        plan.report_fingerprint()
    );
}

#[test]
fn selected_top_level_provider_binds_its_exact_actual_reach() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        pub data InterruptAcknowledgement [copy] { token: u64; }
        pub data Pic {}

        pub boundary requirement InterruptAcknowledgement::complete(self) -> u64
        reaches <= MachineControl + PortIo;

        machine Pic::complete(acknowledgement: InterruptAcknowledgement) -> u64
        satisfies InterruptAcknowledgement::complete
        reaches PortIo
        {
            0
        }
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("bounded top-level provider should check");
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("one selected PIC completion plan");
    let selected = bind_selected_provider_plan_facts_for_test(
        &mut checked,
        std::slice::from_ref(&plan),
        selected,
        &[],
    )
    .expect("selected top-level completion reach should resolve");
    let requirement_identity = plan.rows[0].requirement_identity.as_str();
    let resolution = selected
        .installation_reach_resolution(requirement_identity)
        .expect("top-level bounded requirement resolution");

    assert_eq!(resolution.upper_bound, ["MachineControl", "PortIo"]);
    assert_eq!(resolution.resolved_row, ["PortIo"]);
    assert_eq!(
        resolution.provider_plan_report_identity,
        plan.report_fingerprint()
    );
}

#[test]
fn selected_top_level_provider_with_unresolved_installation_reach_is_not_a_resolved_row() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}
        boundary trait Storage {}

        pub data InterruptAcknowledgement [copy] { token: u64; }
        pub data Endpoint {}
        pub data Pic {}

        pub boundary requirement InterruptAcknowledgement::complete(self) -> u64
        reaches <= MachineControl + PortIo + Storage;

        pub boundary requirement Endpoint::step() reaches <= Storage;

        machine Pic::complete(acknowledgement: InterruptAcknowledgement) -> u64
        satisfies InterruptAcknowledgement::complete
        reaches PortIo + Storage
        {
            Endpoint::step();
            0
        }
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("provider calling an unresolved bounded requirement should check");
    let realization = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Pic::complete")
        .expect("realization machine");
    let envelope = checked
        .facts
        .contract_plans
        .realized_envelope(realization.symbol)
        .expect("realization envelope");
    assert_eq!(envelope.unresolved_installation_reaches.len(), 1);
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("one selected PIC completion plan");
    let diagnostics = bind_selected_provider_plan_facts_for_test(
        &mut checked,
        std::slice::from_ref(&plan),
        selected,
        &[],
    )
    .expect_err(
        "a realization with an unresolved installation reach must not publish a resolved row",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("retains 1 unresolved installation-bound requirement")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn selected_trait_provider_with_unresolved_installation_reach_is_not_a_resolved_row() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}
        boundary trait Storage {}

        pub data Endpoint {}
        pub boundary requirement Endpoint::step() reaches <= Storage;

        boundary trait InterruptCompletion {
            machine complete() -> u64
            reaches <= MachineControl + PortIo + Storage;
        }

        data Pic {}

        machine Pic::complete() -> u64
        satisfies InterruptCompletion::complete
        reaches PortIo + Storage
        {
            Endpoint::step();
            0
        }
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("trait provider calling an unresolved bounded requirement should check");
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("one selected PIC plan");
    let diagnostics = bind_selected_provider_plan_facts_for_test(
        &mut checked,
        std::slice::from_ref(&plan),
        selected,
        &[],
    )
    .expect_err(
        "a realization with an unresolved installation reach must not publish a resolved row",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("retains 1 unresolved installation-bound requirement")
        }),
        "{diagnostics:?}"
    );
}

#[test]
fn selected_boundary_operator_does_not_enter_trait_installation_reach_resolution() {
    let source = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        {
            transition { _ -> (input) }
        }
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    let mut checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("boundary operator provider should check");
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("one selected boundary operator plan");
    let selected = bind_selected_provider_plan_facts_for_test(
        &mut checked,
        std::slice::from_ref(&plan),
        selected,
        &[],
    )
    .expect("boundary operator selection must not require a trait installation row");

    assert!(selected.installation_reach_resolutions().is_empty());
}

#[test]
fn shared_provider_binding_publishes_exact_operator_identity_without_mutating_retained_custody() {
    let (checked, plan) = selected_operator_binding_fixture();
    let (use_handle, use_before) = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .next()
        .expect("one named boundary-operator use");
    assert_eq!(use_before.provider_plan_report_fingerprint, 0);
    assert!(use_before.provider_plan_commitment.is_empty());
    let original_contents = checked.clone();
    let original = Arc::new(checked);
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("select exact operator provider");

    let binding = bind_selected_provider_plan_facts(
        &original,
        std::slice::from_ref(&plan),
        selected,
        &[],
        &[],
    )
    .expect("exact provider binding succeeds");
    let (bound, selected, _) = binding.into_parts();

    assert!(!Arc::ptr_eq(&bound, &original));
    assert_eq!(original.as_ref(), &original_contents);
    assert_eq!(
        original
            .facts
            .operators
            .named_uses
            .get(use_handle)
            .provider_plan_report_fingerprint,
        0
    );
    assert_eq!(
        bound
            .facts
            .operators
            .named_uses
            .get(use_handle)
            .provider_plan_report_fingerprint,
        plan.report_fingerprint()
    );
    assert_eq!(
        bound
            .facts
            .operators
            .named_uses
            .get(use_handle)
            .provider_plan_commitment
            .as_bytes(),
        plan.identity_digest().as_bytes(),
    );
    assert!(selected.installation_reach_resolutions().is_empty());
}

#[test]
fn late_reach_rejection_publishes_no_staged_operator_updates() {
    let (checked, operator_plan) = selected_operator_binding_fixture();
    let use_handle = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, _)| handle)
        .next()
        .expect("one named boundary-operator use");
    let missing_trait_plan = selection_plan("MissingProvider", &["missing"], &["missing"]);
    let candidates = [operator_plan.clone(), missing_trait_plan.clone()];
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        &candidates,
        &[operator_plan.name.clone(), missing_trait_plan.name.clone()],
    )
    .expect("select exact operator and missing-trait fixtures");
    let original_contents = checked.clone();
    let original = Arc::new(checked);

    let diagnostics = bind_selected_provider_plan_facts(&original, &candidates, selected, &[], &[])
        .expect_err("missing typed requirement must reject after operator staging");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "selected provider row `Pair::missing` resolves to 0 exact typed requirements",
        )
    }));
    assert_eq!(original.as_ref(), &original_contents);
    assert_eq!(
        original
            .facts
            .operators
            .named_uses
            .get(use_handle)
            .provider_plan_report_fingerprint,
        0
    );
    assert!(
        original
            .facts
            .operators
            .named_uses
            .get(use_handle)
            .provider_plan_commitment
            .is_empty()
    );
}

#[test]
fn empty_provider_binding_preserves_exact_arc_identity_and_contents() {
    let original = Arc::new(checked_trees::CheckedTrees::default());
    let original_contents = original.as_ref().clone();

    let binding = bind_selected_provider_plan_facts(
        &original,
        &[],
        effects::SelectedProviderPlanFacts::default(),
        &[],
        &[],
    )
    .expect("empty provider binding is already settled");
    let (bound, selected, _) = binding.into_parts();

    assert!(Arc::ptr_eq(&bound, &original));
    assert_eq!(bound.as_ref(), &original_contents);
    assert_eq!(selected, effects::SelectedProviderPlanFacts::default());
}
