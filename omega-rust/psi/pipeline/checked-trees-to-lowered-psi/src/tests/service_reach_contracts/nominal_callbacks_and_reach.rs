use super::service_names;
use crate::TerminalMachineSelection;
use crate::tests::{
    Lexer, ResolutionRequest, checked_source, lower_machine, lower_symbol_resolved_trees,
    lower_typed_trees, parse_syntax_trees, resolve,
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;
use typed_trees_to_checked_trees::CheckingRequest;

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
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Root::enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("publish direct boundary claim transfer")
    .into_artifact();
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
        terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("Root::unselected")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err(),
        "an unresolved binder must not become a boundary execution choice"
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Root::selected"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("publish the closed quiet selection")
    .into_artifact();
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
    let produced = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("Root::enter"),
    )
    .produce(TerminalProductionCustody {
        entry_identity: Some([0xa5; 32]),
        callback_custody: (),
        timings: &mut TerminalProductionTimings::default(),
    })
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
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("source-free closed selection starts");
    assert_eq!(
        execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::default(),
                &mut AcceptTerminalEffects
            )
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
        "../../../../../../../tests/omega/pass/effects/nominal_callback_dependency/main.omg"
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
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("enter"))
            .unwrap_or_else(|error| panic!("selected callback {callback_reach:?}: {error:?}"));
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("enter"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("publish traversal")
        .into_artifact();
        let module =
            terminal_codec::decode_module(artifact.semantic_bytes()).expect("decode traversal");
        assert_eq!(module, lowered.semantic_module);
        assert_eq!(module.root_service_reach.concrete.len(), expected_services);
        // The retained nominal bound names Console even when the selected
        // callback and root reach are quiet; metadata is not a root effect.
        assert_eq!(module.services.len(), 1);
        let binding = module
            .machines
            .iter()
            .filter_map(|machine| machine.closed_reach_application.as_ref())
            .flat_map(|application| &application.telescope)
            .find_map(|parameter| {
                let terminal_psi::ClosedReachParameter::Machine(binding) = parameter else {
                    return None;
                };
                binding.nominal_requirement.is_some().then_some(binding)
            })
            .expect("retained nominal callback bound");
        assert_eq!(service_names(&module, &binding.upper_bound), ["Console"]);
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
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("publish inert Unit helper")
    .into_artifact();
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
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let result = lower_typed_trees(typed, &CheckingRequest::settled());
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
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("Root::enter"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("publish complete direct installation-bound declaration")
        .into_artifact();
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
