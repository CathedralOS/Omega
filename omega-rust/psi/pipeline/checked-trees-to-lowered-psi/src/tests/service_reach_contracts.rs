use super::*;

#[test]
fn direct_boundary_calls_transfer_both_owned_claims() {
    let checked = checked_source(
        r#"
        pub data Extent [linear] { value: u64; }
        pub boundary trait Sink { machine take(first: Extent, second: Extent); }
        pub data Root {}
        pub machine Root::enter(first: Extent, second: Extent)
        reaches Sink invokes Sink;
        { Sink::take(first, second); }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
        .produce_artifact()
        .expect("publish direct boundary claim transfer");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload claims");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    assert_eq!(entry.entry_claims.len(), 2);
    let OperationKind::BoundaryCall {
        completion_receipts,
        ..
    } = &entry.blocks[0].operations[0].kind
    else {
        panic!("boundary call");
    };
    assert_eq!(completion_receipts.len(), 2);
    for mutation in 0..4 {
        let mut invalid = module.clone();
        let entry = invalid
            .machines
            .iter_mut()
            .find(|machine| machine.id == invalid.entry)
            .expect("entry");
        let OperationKind::BoundaryCall {
            completion_receipts,
            ..
        } = &mut entry.blocks[0].operations[0].kind
        else {
            panic!("boundary call");
        };
        match mutation {
            0 => {
                completion_receipts.pop();
            }
            1 => completion_receipts[1] = completion_receipts[0],
            2 => completion_receipts[0].argument_index = 1,
            _ => completion_receipts.swap(0, 1),
        }
        assert!(
            terminal_verifier::validate_module(&invalid).is_err(),
            "missing, duplicate, wrong-position and reordered receipts reject: {mutation}"
        );
    }
}

#[test]
fn nominal_unit_callbacks_require_a_closed_executable_selection() {
    let source = r#"
        pub boundary trait Sink { machine emit(first: i32, second: i32); }
        pub data Root {}
        pub machine Root::unselected<machine Emit>(left: i32, right: i32)
        where machine Emit satisfies Sink::emit;
        { Emit(left, right); }
        machine quiet(first: i32, second: i32) satisfies Sink::emit {}
        pub machine Root::selected() { Root::unselected<quiet>(3, 7); }
    "#;
    let checked = checked_source(source);
    assert!(
        terminal_production::TerminalProductionRequest::new(&checked, "Root::unselected")
            .produce_artifact()
            .is_err(),
        "an unresolved binder must not become a boundary execution choice"
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::selected")
        .produce_artifact()
        .expect("publish the closed quiet selection");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload");
    assert!(
        module.boundary_machines.is_empty(),
        "quiet checked selection is not a host boundary"
    );
    drop(checked);
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
        )
        .expect("interpret the selected body without host effects"),
        terminal_interpreter::TerminalExecutionResult::Unit
    );
}

#[test]
fn closed_nominal_callback_transfers_both_claims_to_its_selected_body() {
    let checked = checked_source(
        r#"
        pub data Extent [linear] { value: u64; }
        pub boundary trait Sink { machine take(first: Extent, second: Extent); }
        boundary machine Extent::settle(self) ensures true;
        pub data Root {}
        machine Root::forward<machine Take>(first: Extent, second: Extent)
        where machine Take satisfies Sink::take;
        { Take(first, second); }
        machine selected(first: Extent, second: Extent) satisfies Sink::take
        { first.settle(); second.settle(); }
        pub machine Root::enter(first: Extent, second: Extent)
        { Root::forward<selected>(first, second); }
    "#,
    );
    let produced = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
        .produce_program_entry([0xa5; 32])
        .expect("closed generic ProgramEntry publishes");
    let artifact = produced.artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload");
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry");
    assert_eq!(entry.entry_claims.len(), 2);
    assert_eq!(
        module.boundary_machines.len(),
        1,
        "only the selected body's settlement is a host boundary"
    );
    let arguments = entry
        .structural_parameters
        .iter()
        .enumerate()
        .map(
            |(index, parameter)| terminal_interpreter::TerminalStructuralValue {
                opaque_identity: u64::try_from(index).expect("argument index") + 1,
                structural_type: parameter.structural_type,
                qualifications: parameter.qualifications.clone(),
                path: Vec::new(),
            },
        )
        .collect::<Vec<_>>();
    drop(checked);
    let mut execution =
        terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            &arguments,
        )
        .expect("source-free closed selection starts");
    assert_eq!(
        execution
            .resume(&mut terminal_fuel::TerminalFuelMeter::default())
            .expect("execute selected body"),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        )
    );
    assert!(matches!(execution.effects(), [
        terminal_interpreter::TerminalEffect::BoundaryCall { structural_arguments: first, completion_receipts: first_receipts, .. },
        terminal_interpreter::TerminalEffect::BoundaryCall { structural_arguments: second, completion_receipts: second_receipts, .. },
    ] if first == &arguments[..1] && second == &arguments[1..]
        && first_receipts.len() == 1 && second_receipts.len() == 1));
}

#[test]
fn nominal_callback_selected_reach_survives_terminal_publication() {
    let traversal = include_str!(
        "../../../../../../tests/omega/pass/effects/nominal_callback_dependency/main.omg"
    );
    for (callback_reach, callback_body, helper, expected_services) in [
        ("", "value", "", 0),
        ("reaches Console", "value", "", 1),
        (
            "",
            "helper(value)",
            "machine helper(value: u64) -> u64 reaches Console { value }",
            1,
        ),
    ] {
        let source = format!(
            "{traversal}\n {helper}\n machine selected(value: u64) -> u64 satisfies StepContract::step {callback_reach} {{ {callback_body} }}\n pub machine enter(value: u64) -> u64 {{ traverse<selected>(value) }}"
        );
        let checked = checked_source(&source);
        let lowered = lower_machine(&checked, "enter")
            .unwrap_or_else(|error| panic!("selected callback {callback_reach:?}: {error:?}"));
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
            .produce_artifact()
            .expect("publish traversal");
        let module =
            terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode traversal");
        assert_eq!(module, lowered.semantic_module);
        assert_eq!(module.root_service_reach.concrete.len(), expected_services);
        assert_eq!(module.services.len(), expected_services);
        assert!(
            module
                .root_service_reach
                .installation_dependencies
                .is_empty()
        );
        for machine in &module.machines {
            assert_eq!(
                machine.published_service_ceiling,
                module.root_service_reach.concrete
            );
        }
        assert_eq!(
            module
                .machines
                .iter()
                .filter(|machine| !machine.declared_service_reach.is_empty())
                .count(),
            expected_services
        );
        terminal_verifier::verify_module(
            &module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("independently replay concrete reach and calls");
        let argument = terminal_interpreter::TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            value: IntegerValue::Unsigned(7),
        };
        assert_eq!(
            terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[argument],
            )
            .expect("interpret source-free scalar traversal"),
            terminal_interpreter::TerminalExecutionResult::Scalar(argument),
        );
        if expected_services != 0 {
            assert_eq!(module.services[0].identity, "Console");
            let mut stale_root = module.clone();
            stale_root.root_service_reach.concrete.clear();
            assert!(matches!(
                terminal_verifier::validate_module(&stale_root),
                Err(terminal_verifier::ModuleError::RootConcreteServiceReachMismatch { .. })
            ));
            let mut missing_declaration = module;
            for machine in &mut missing_declaration.machines {
                machine.declared_service_reach.clear();
            }
            assert!(matches!(
                terminal_verifier::validate_module(&missing_declaration),
                Err(terminal_verifier::ModuleError::RootConcreteServiceReachMismatch { .. })
            ));
        }
    }
}

#[test]
fn authored_unit_reach_survives_ordinary_helper_publication() {
    let checked = checked_source(
        r#"
        pub boundary trait Console {}
        machine note() reaches Console {}
        pub machine enter() { note(); }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("publish inert Unit helper");
    let module =
        terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode Unit helper");
    assert_eq!(module.services[0].identity, "Console");
    assert_eq!(
        module.root_service_reach.concrete,
        vec![module.services[0].id]
    );
    assert_eq!(
        module
            .machines
            .iter()
            .filter(|machine| !machine.declared_service_reach.is_empty())
            .count(),
        1
    );
}

struct ReachFixture {
    rows: language_semantics::ServiceReachRowTable,
    console: ServiceReachId,
    console_row: ServiceReachRowId,
    console_and_network_row: ServiceReachRowId,
    terminal_services: Vec<(ServiceReachId, ServiceId)>,
}

#[test]
fn direct_installation_boundary_keeps_its_required_declaration() {
    for additional_reach in ["", "+ Console"] {
        let source = format!(
            r#"
            pub boundary trait Console {{}}
            pub boundary trait Installer {{ machine step() reaches <= Console; }}
            pub data Root {{}}
            pub machine Root::enter() reaches Installer {additional_reach} invokes Installer; {{ Installer::step(); }}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let result = lower_typed_trees(typed);
        if additional_reach.is_empty() {
            let diagnostics =
                result.expect_err("installation bounds do not waive direct declarations");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("undeclared service `Console`"))
            );
            continue;
        }
        let checked = result.expect("complete direct reach declaration checks");
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
            .produce_artifact()
            .expect("publish complete direct installation-bound declaration");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload");
        assert_eq!(
            service_names(&module, &module.boundary_machines[0].fixed_service_reach),
            ["Installer"]
        );
        assert_eq!(
            service_names(
                &module,
                &module.root_service_reach.installation_dependencies[0].upper_bound
            ),
            ["Console"]
        );
        assert_eq!(
            service_names(&module, &module.root_service_reach.concrete),
            ["Console", "Installer"]
        );
    }
}

fn service_names<'module>(module: &'module TerminalModule, row: &[ServiceId]) -> Vec<&'module str> {
    row.iter()
        .map(|service| {
            module
                .services
                .iter()
                .find(|declaration| declaration.id == *service)
                .expect("declared service")
                .identity
                .as_str()
        })
        .collect()
}

#[test]
fn top_level_bounded_boundary_keeps_fixed_invocation_reach() {
    for bound in ["Console", "Storage"] {
        let source = format!(
            r#"
        pub boundary trait Audit {{}}
        pub boundary trait Console: Audit {{}}
        pub boundary trait Storage {{}}
        pub data Endpoint {{}}
        pub boundary requirement Endpoint::step() invokes Console; reaches <= {bound};
    "#
        );
        let checked = checked_source(&source);
        let requirement = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Endpoint::step")
            .expect("top-level requirement");
        let service_ids = ["Audit", "Console", "Storage"]
            .iter()
            .enumerate()
            .map(|(position, name)| {
                (
                    checked
                        .facts
                        .service_reaches
                        .services
                        .id_for_name(name)
                        .expect("service"),
                    service_id(u64::try_from(position).expect("service position") + 1),
                )
            })
            .collect::<Vec<_>>();
        let root = lower_root_service_reach(&checked, requirement.symbol, &service_ids)
            .expect("lower exact top-level requirement closure");
        assert_eq!(
            root.concrete,
            service_ids[..2]
                .iter()
                .map(|(_, terminal)| *terminal)
                .collect::<Vec<_>>()
        );
        assert_eq!(root.installation_dependencies.len(), 1);
        let expected_bound = if bound == "Console" {
            &service_ids[..2]
        } else {
            &service_ids[..]
        };
        assert_eq!(
            root.installation_dependencies[0].upper_bound,
            expected_bound
                .iter()
                .map(|(_, terminal)| *terminal)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            root.installation_dependencies[0].requirement_identity,
            checked
                .typed
                .normalized_machine_overload_identity(requirement)
                .expect("normalized requirement")
                .identity()
        );
        let envelope = checked
            .facts
            .contract_plans
            .realized_envelope(requirement.symbol)
            .expect("checked requirement envelope");
        assert_eq!(envelope.concrete_service_reach, ["Audit", "Console"]);
        assert_eq!(envelope.unresolved_installation_reaches.len(), 1);
        assert_eq!(
            envelope.unresolved_installation_reaches[0].requirement,
            requirement.symbol
        );
        assert_eq!(
            envelope.unresolved_installation_reaches[0].upper_bound,
            requirement.service_reach_row
        );
        let call_source = format!(
            "{source}\n pub data Root {{}}\n machine helper() reaches Console + Storage invokes Console; {{ Endpoint::step(); }}\n pub machine Root::enter() invokes Console; {{ helper(); helper(); }}"
        );
        let caller = checked_source(&call_source);
        let artifact = terminal_production::TerminalProductionRequest::new(&caller, "Root::enter")
            .produce_artifact()
            .expect("publish explicit top-level boundary calls");
        let module =
            terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload calls");
        assert_eq!(
            service_names(&module, &module.boundary_machines[0].fixed_service_reach),
            ["Audit", "Console"]
        );
        assert_eq!(
            module.boundary_machines[0].identity,
            root.installation_dependencies[0].requirement_identity
        );
        assert_eq!(
            module.root_service_reach.installation_dependencies,
            root.installation_dependencies
        );
        assert_eq!(
            service_names(&module, &module.root_service_reach.concrete),
            ["Audit", "Console", "Storage"]
        );
        let mut stale = module.clone();
        stale.boundary_machines[0].identity = "Endpoint::step".into();
        assert!(
            terminal_verifier::validate_module(&stale).is_err(),
            "display name cannot replace normalized requirement identity"
        );
        drop(caller);
        drop(checked);
        struct Host;
        impl terminal_interpreter::TerminalEffectHandler for Host {
            fn handle_effect(
                &mut self,
                effect: &terminal_interpreter::TerminalEffect,
            ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
                assert!(matches!(
                    effect,
                    terminal_interpreter::TerminalEffect::BoundaryCall { .. }
                ));
                Ok(())
            }
        }
        let execution =
            terminal_interpreter::interpret_terminal_artifact_with_effect_handler_measured(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                &[],
                &mut Host,
            )
            .expect("interpret source-free top-level boundary helpers");
        assert_eq!(
            execution.value(),
            terminal_interpreter::TerminalExecutionResult::Unit
        );
        assert_eq!(execution.effects().len(), 2);
        if bound == "Storage" {
            let invalid = call_source.replace("reaches Console + Storage invokes", "invokes");
            let tokens = Lexer::new(&invalid).tokenize().expect("tokens");
            let syntax = parse_syntax_trees(&tokens).expect("parse");
            let resolved = lower_syntax_trees(&syntax).expect("resolve");
            let typed = lower_symbol_resolved_trees(&resolved).expect("type");
            let diagnostics =
                lower_typed_trees(typed).expect_err("bound does not waive direct declaration");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("undeclared service `Storage`")),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn bounded_boundary_helpers_replay_fixed_parent_and_invocation_reach() {
    for (bound, invocations, expected_fixed, expected_bound) in [
        ("Console", "", vec!["Audit", "Installer"], vec!["Console"]),
        (
            "Console + Installer",
            "",
            vec!["Audit", "Installer"],
            vec!["Audit", "Console", "Installer"],
        ),
        (
            "Console",
            "invokes Console;",
            vec!["Audit", "Console", "Installer"],
            vec!["Console"],
        ),
    ] {
        let source = format!(
            r#"
            pub boundary trait Audit {{}}
            pub boundary trait Console {{}}
            pub boundary trait Installer: Audit {{
                machine step() {invocations} reaches <= {bound};
            }}
            machine helper() reaches Installer + Console invokes Installer; {invocations} {{ Installer::step(); }}
            pub data Root {{}}
            pub machine Root::enter() invokes Installer; {invocations} {{ helper(); helper(); }}
        "#
        );
        let checked = checked_source(&source);
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
            .produce_artifact()
            .expect("publish helper closure");
        let module = terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("reload helper closure");
        assert_eq!(module.boundary_machines.len(), 1);
        assert_eq!(module.root_service_reach.installation_dependencies.len(), 1);
        assert_eq!(
            service_names(&module, &module.boundary_machines[0].fixed_service_reach),
            expected_fixed
        );
        assert_eq!(
            service_names(
                &module,
                &module.root_service_reach.installation_dependencies[0].upper_bound
            ),
            expected_bound
        );
        assert_eq!(
            service_names(&module, &module.root_service_reach.concrete),
            ["Audit", "Console", "Installer"]
        );
        drop(checked);
        struct Host;
        impl terminal_interpreter::TerminalEffectHandler for Host {
            fn handle_effect(
                &mut self,
                effect: &terminal_interpreter::TerminalEffect,
            ) -> Result<(), terminal_interpreter::TerminalEffectRejection> {
                assert!(matches!(
                    effect,
                    terminal_interpreter::TerminalEffect::BoundaryCall { .. }
                ));
                Ok(())
            }
        }
        let execution =
            terminal_interpreter::interpret_terminal_artifact_with_effect_handler_measured(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
                &[],
                &mut Host,
            )
            .expect("interpret decoded helpers with receiving host authority");
        assert_eq!(
            execution.value(),
            terminal_interpreter::TerminalExecutionResult::Unit
        );
        assert_eq!(execution.effects().len(), 2);
    }
}

#[test]
fn standalone_scalar_helpers_reject_reachful_or_missing_contracts() {
    let mut checked = checked_source(
        r#"
        pub boundary trait Console {}
        pub machine selected(value: u64) -> u64 reaches Console { value }
    "#,
    );
    let selected = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "selected")
        .expect("selected callback")
        .symbol;
    assert!(
        crate::scalar_call_closure::embedded::EmbeddedScalarCalls::prepare_targets(
            &checked,
            &[selected],
            &[],
            0,
        )
        .is_err(),
        "standalone helpers cannot erase a retained Console contract"
    );
    checked.facts.service_reaches.root_machines = arena::HandleSpan::empty();
    assert!(
        crate::scalar_call_closure::embedded::EmbeddedScalarCalls::prepare_targets(
            &checked,
            &[selected],
            &[],
            0,
        )
        .is_err(),
        "absence is not an empty checked service contract"
    );
    assert!(crate::scalar_call_closure::requires_shared_catalog(&checked, selected).is_err());
}

fn reach_fixture() -> ReachFixture {
    let mut services = language_semantics::ServiceReachTable::default();
    let console = services.intern(SymbolHandle::from_arena_index(1), "Console");
    let network = services.intern(SymbolHandle::from_arena_index(2), "Network");
    let mut rows = language_semantics::ServiceReachRowTable::default();
    rows.intern(Vec::new());
    let console_row = rows.intern(vec![console]);
    let console_and_network_row = rows.intern(vec![console, network]);
    ReachFixture {
        rows,
        console,
        console_row,
        console_and_network_row,
        terminal_services: vec![(console, service_id(1)), (network, service_id(2))],
    }
}

fn summary(row: ServiceReachRowId) -> ServiceReachSummary {
    ServiceReachSummary {
        direct: row,
        transitive: row,
    }
}

#[test]
fn internal_inferred_reach_is_retained_for_executable_lowering() {
    let fixture = reach_fixture();
    let contract = ServiceReachPlan {
        interface: ServiceReachInterface::InternalInferred,
        checked_inferred: fixture.console_row,
    };
    let mut selected = Vec::new();

    collect_contract_services(
        &fixture.rows,
        contract,
        summary(fixture.console_row),
        &mut selected,
    )
    .expect("private inferred reach remains executable");
    assert_eq!(selected, vec![fixture.console]);
    assert_eq!(
        lower_contract_service_ceiling(
            &fixture.rows,
            contract,
            summary(fixture.console_row),
            &fixture.terminal_services,
        )
        .expect("private inferred reach lowers into the executable contract"),
        vec![service_id(1)],
    );
}

#[test]
fn published_contract_rejects_inferred_reach_outside_published_ceiling() {
    let fixture = reach_fixture();
    let contract = ServiceReachPlan {
        interface: ServiceReachInterface::PublishedCeiling(fixture.console_row),
        checked_inferred: fixture.console_and_network_row,
    };
    let expected =
        LoweringError::Unsupported("checked Unit service reach exceeds its published ceiling");
    let mut selected = Vec::new();

    assert_eq!(
        collect_contract_services(
            &fixture.rows,
            contract,
            summary(fixture.console_and_network_row),
            &mut selected,
        ),
        Err(expected.clone()),
    );
    assert_eq!(
        lower_contract_service_ceiling(
            &fixture.rows,
            contract,
            summary(fixture.console_and_network_row),
            &fixture.terminal_services,
        ),
        Err(expected),
    );
}

fn checked_public_reach_wrapper() -> checked_trees::CheckedTrees {
    checked_source(
        r#"
            pub boundary trait Audit { machine record() reaches Audit; }
            pub boundary trait Host { machine ping() reaches Host + Audit; }
            pub data Root {}
            machine Root::helper() reaches Host + Audit { Host::ping(); }
            pub machine Root::enter() invokes Host; { Root::helper(); }
        "#,
    )
}

#[test]
fn public_wrapper_publishes_propagated_reach_and_pinned_boundary_ceiling() {
    let checked = checked_public_reach_wrapper();
    let wrapper = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .expect("public wrapper");
    assert!(wrapper.is_public);
    assert!(
        checked
            .authored_service_reach_rows_for(wrapper.symbol)
            .next()
            .is_none(),
        "the wrapper does not author its callee's reaches clause",
    );
    let lowered = lower_machine(&checked, "Root::enter").expect("public wrapper lowers");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Root::enter")
        .produce_artifact()
        .expect("public propagated contract publishes");
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("published Terminal module decodes");
    assert_eq!(module, lowered.semantic_module);
    terminal_verifier::verify_module(
        &module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent replay accepts the propagated public contract");
    let mut identities = module
        .services
        .iter()
        .map(|service| service.identity.as_str())
        .collect::<Vec<_>>();
    identities.sort_unstable();
    assert_eq!(identities, ["Audit", "Host"]);
    let mut services = module
        .services
        .iter()
        .map(|service| service.id)
        .collect::<Vec<_>>();
    services.sort_unstable();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("published wrapper entry");
    assert_eq!(entry.published_service_ceiling, services);
    assert_eq!(module.boundary_machines.len(), 1);
    assert_eq!(
        module.boundary_machines[0].published_service_ceiling,
        services
    );
    assert_eq!(module.root_service_reach.concrete, services);
    assert!(
        module
            .root_service_reach
            .installation_dependencies
            .is_empty()
    );
}

#[test]
fn public_wrapper_replay_rejects_removed_propagated_service() {
    let checked = checked_public_reach_wrapper();
    let lowered = lower_machine(&checked, "Root::enter").expect("public wrapper lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("untampered wrapper verifies");
    let mut module = lowered.semantic_module;
    let entry = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .expect("published wrapper entry");
    assert_eq!(entry.published_service_ceiling.len(), 2);
    entry.published_service_ceiling.pop();
    let result = terminal_verifier::verify_module(
        &module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    );
    assert!(
        matches!(
            result,
            Err(terminal_verifier::VerificationError::Module(
                terminal_verifier::ModuleError::DeclaredServiceOutsidePublishedCeiling { .. },
            )),
        ),
        "the wrapper's declared invokes Host contribution cannot fit its narrowed row: {result:?}",
    );
}

#[test]
fn public_contract_lowering_rejects_internal_inference() {
    let fixture = reach_fixture();
    let contract = ServiceReachPlan {
        interface: ServiceReachInterface::InternalInferred,
        checked_inferred: fixture.console_row,
    };
    let expected =
        LoweringError::Unsupported("public Unit contract has no published service ceiling");
    let mut selected = Vec::new();

    assert_eq!(
        collect_published_contract_services(
            &fixture.rows,
            contract,
            summary(fixture.console_row),
            &mut selected,
        ),
        Err(expected.clone()),
    );
    assert_eq!(
        lower_published_service_ceiling(
            &fixture.rows,
            contract,
            summary(fixture.console_row),
            &fixture.terminal_services,
        ),
        Err(expected),
    );
}
