use super::{checked_public_reach_wrapper, reach_fixture, service_names, summary};
use crate::TerminalMachineSelection;
use crate::terminal_identities::service_id;
use crate::tests::{
    Lexer, LoweringError, ResolutionRequest, checked_source, lower_machine,
    lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees, resolve,
};
use crate::unit::attached_unit::{
    collect_contract_services, collect_published_contract_services, lower_contract_service_ceiling,
    lower_published_service_ceiling, lower_root_service_reach,
};
use language_semantics::{ServiceReachInterface, ServiceReachPlan};
use terminal_interpreter::TerminalStructuralInputs;
use typed_trees_to_checked_trees::CheckingRequest;

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
        let execution = terminal_interpreter::interpret_terminal_artifact_measured(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
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
            let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
            let typed = lower_symbol_resolved_trees(&resolved).expect("type");
            let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
                .expect_err("bound does not waive direct declaration");
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
        let execution = terminal_interpreter::interpret_terminal_artifact_measured(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
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
fn unresolved_installation_selection_keeps_closed_reach_application() {
    let checked = checked_source(
        r#"
        pub boundary trait Console {}
        pub boundary trait Installer { machine install() reaches <= Console; }
        pub trait StepContract {
            machine step() reaches Installer + Console invokes Installer;
        }
        machine installing() satisfies StepContract::step reaches Installer + Console invokes Installer; {
            Installer::install();
        }
        machine traverse<machine Step>()
        where machine Step satisfies StepContract::step;
        {
            Step();
        }
        pub machine enter() reaches Installer + Console invokes Installer; { traverse<installing>(); }
    "#,
    );
    let lowered =
        lower_machine(&checked, TerminalMachineSelection::Name("enter")).expect("lower traverse");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("publish installation-bound selection");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload");
    assert_eq!(module, lowered.semantic_module);
    drop(checked);
    assert_eq!(module.boundary_machines.len(), 1);
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
    let traverse = module
        .machines
        .iter()
        .find(|machine| machine.closed_reach_application.is_some())
        .expect("selected closure retains the closed application");
    let application = traverse.closed_reach_application.as_ref().unwrap();
    assert!(
        application.fixed.is_empty(),
        "the original template publishes no fixed row"
    );
    assert_eq!(application.dependencies, [0]);
    assert_eq!(application.calls.len(), 1);
    let terminal_psi::ClosedReachParameter::Machine(binding) = &application.telescope[0] else {
        panic!("selected telescope position is a machine binding");
    };
    // The selected public contract remains the whole effective row: provider
    // bounds stay conservative while the requirement axis is replayed
    // separately through installation dependencies.
    assert_eq!(
        service_names(&module, &binding.selected_reach),
        ["Console", "Installer"]
    );
    assert_eq!(
        service_names(&module, &binding.upper_bound),
        ["Console", "Installer"]
    );
    let installing = module
        .machines
        .iter()
        .find(|machine| Some(machine.id) == binding.callee)
        .expect("selected callback is emitted");
    assert_eq!(installing.published_service_ceiling, binding.selected_reach);
    assert_eq!(application.calls[0].binder, 0);
    let consumer = traverse
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| operation.id == application.calls[0].operation)
        .expect("binder consumer operation is retained");
    assert_eq!(consumer.static_reach_binding, Some(0));
    terminal_verifier::verify_module(
        &module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independently replay unresolved installation selection");
    // A shrunken selected row is stale evidence: it no longer matches the
    // emitted callable's public contract or the owner's substituted ceiling.
    let mut stale = module.clone();
    let stale_application = stale
        .machines
        .iter_mut()
        .find_map(|machine| machine.closed_reach_application.as_mut())
        .expect("stale application");
    let terminal_psi::ClosedReachParameter::Machine(stale_binding) =
        &mut stale_application.telescope[0]
    else {
        panic!("stale machine binding");
    };
    stale_binding.selected_reach.clear();
    assert!(matches!(
        terminal_verifier::validate_module(&stale),
        Err(terminal_verifier::ModuleError::InvalidClosedReachApplication { .. })
    ));
    // Clearing the operation marker detaches the consumer half of the join.
    let mut unmarked = module.clone();
    let consumer_id = application.calls[0].operation;
    unmarked
        .machines
        .iter_mut()
        .flat_map(|machine| machine.blocks.iter_mut())
        .flat_map(|block| block.operations.iter_mut())
        .find(|operation| operation.id == consumer_id)
        .expect("unmarked consumer")
        .static_reach_binding = None;
    assert!(matches!(
        terminal_verifier::validate_module(&unmarked),
        Err(terminal_verifier::ModuleError::InvalidClosedReachApplication { .. })
    ));
    // The installation dependency axis is enforced independently: shrinking
    // its bound mismatches the boundary's published ceiling.
    let mut shrunken = module;
    shrunken.root_service_reach.installation_dependencies[0]
        .upper_bound
        .clear();
    assert!(matches!(
        terminal_verifier::validate_module(&shrunken),
        Err(terminal_verifier::ModuleError::InstallationReachBoundaryMismatch(_))
    ));
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
        crate::scalar_graph::scalar_call_closure::embedded::EmbeddedScalarCalls::prepare_targets(
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
        crate::scalar_graph::scalar_call_closure::embedded::EmbeddedScalarCalls::prepare_targets(
            &checked,
            &[selected],
            &[],
            0,
        )
        .is_err(),
        "absence is not an empty checked service contract"
    );
    assert!(
        crate::scalar_graph::scalar_call_closure::requires_shared_catalog(&checked, selected)
            .is_err()
    );
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
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("public wrapper lowers");
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
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Root::enter"))
        .expect("public wrapper lowers");
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
