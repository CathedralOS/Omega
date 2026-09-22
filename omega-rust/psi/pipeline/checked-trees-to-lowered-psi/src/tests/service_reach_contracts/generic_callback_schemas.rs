use super::{nominal_schema_forwarding_module, service_names};
use crate::lower_bounded_callback_identity_machine;
use crate::tests::checked_source;
use lowered_psi_to_lowered_psi::run_psi_optimization;
use lowered_psi_to_terminal_psi::finalize_terminal_artifact;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;

#[test]
fn generic_callback_schema_retains_both_closed_callees_after_reload() {
    let checked = checked_source(include_str!(
        "../../../../../../../tests/omega/pass/effects/generic_callback_schema_reach/main.omg"
    ));
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("two closed applications of one selected schema")
    .into_artifact();
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("source-free schema reload");
    let outer = module
        .machines
        .iter()
        .find(|machine| {
            machine
                .closed_reach_application
                .as_ref()
                .is_some_and(|application| {
                    application.telescope.iter().any(|parameter| {
                        matches!(parameter, terminal_psi::ClosedReachParameter::Machine(_))
                    })
                })
        })
        .expect("outer schema selection must survive source discard");
    let application = outer.closed_reach_application.as_ref().unwrap();
    assert_eq!(application.calls.len(), 2);
    assert_eq!(
        application.telescope.len(),
        2,
        "unused schema stays in the full telescope"
    );
    let terminal_psi::ClosedReachParameter::Machine(unused) = &application.telescope[1] else {
        panic!("unused schema binder");
    };
    assert!(unused.callee.is_none());
    assert!(
        unused.schema.is_none(),
        "no closed application is manufactured for an unused schema"
    );
    assert_eq!(service_names(&module, &unused.selected_reach), ["Console"]);
    assert_eq!(service_names(&module, &application.fixed), ["Console"]);
    let first = application.calls[0]
        .application
        .as_ref()
        .expect("first closed tuple");
    let second = application.calls[1]
        .application
        .as_ref()
        .expect("second closed tuple");
    assert_ne!(first.callee, second.callee);
    assert_ne!(first.arguments, second.arguments);
    assert_ne!(
        first.specialization_commitment,
        second.specialization_commitment
    );
    let input = terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
        value: IntegerValue::Unsigned(7),
    };
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[input],
        )
        .expect("source-free schema execution"),
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                value: IntegerValue::Unsigned(3),
            }
        )
    );
    for mutation in 0..7 {
        let mut invalid = module.clone();
        let owner = invalid
            .machines
            .iter_mut()
            .find(|machine| machine.id == outer.id)
            .unwrap();
        let application = owner.closed_reach_application.as_mut().unwrap();
        let call = &mut application.calls[0];
        match mutation {
            0 => call.application = None,
            1 => call.application.as_mut().unwrap().arguments.clear(),
            2 => call.application.as_mut().unwrap().arguments = second.arguments.clone(),
            3 => {
                let operation = owner
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find(|operation| operation.id == call.operation)
                    .unwrap();
                let OperationKind::Call { callee, .. } = &mut operation.kind else {
                    panic!("ordinary scalar call");
                };
                *callee = second.callee;
                let receipt = call.application.as_mut().unwrap();
                receipt.callee = second.callee;
                receipt.specialization_commitment = second.specialization_commitment;
                // Even coherent target/commitment redirection retains the wrong tuple.
            }
            4 => {
                let target = invalid
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == first.callee)
                    .unwrap();
                target.closed_reach_application.as_mut().unwrap().telescope[0] =
                    terminal_psi::ClosedReachParameter::Const {
                        argument: "different-constant".into(),
                    };
            }
            5 => {
                let target = invalid
                    .machines
                    .iter_mut()
                    .find(|machine| machine.id == first.callee)
                    .unwrap();
                target.closed_reach_application = None;
            }
            _ => {
                let terminal_psi::ClosedReachParameter::Machine(binding) =
                    &mut application.telescope[0]
                else {
                    panic!("schema binder");
                };
                binding.schema.as_mut().unwrap().template_identity = "unrelated-template".into();
                for target in &mut invalid.machines {
                    if let Some(target_application) = target.closed_reach_application.as_mut()
                        && target.id != outer.id
                    {
                        target_application.template_identity = "unrelated-template".into();
                    }
                }
            }
        }
        assert!(
            terminal_verifier::validate_module_representation(&invalid).is_err(),
            "schema call mutation {mutation} must reject"
        );
    }
}

#[test]
fn unused_schema_selection_reuses_the_same_retained_application_header() {
    let source = include_str!(
        "../../../../../../../tests/omega/pass/effects/generic_callback_schema_reach/main.omg"
    )
    .replace("selected, unused", "selected, selected");
    let checked = checked_source(&source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("used and unused bindings select one family")
    .into_artifact();
    drop(checked);
    let mut module = terminal_codec::decode_module(artifact.semantic_bytes())
        .expect("source-free mixed-use reload");
    let owner_position = module
        .machines
        .iter()
        .position(|machine| {
            machine
                .closed_reach_application
                .as_ref()
                .is_some_and(|application| application.telescope.len() == 2)
        })
        .expect("full outer telescope");
    let application = module.machines[owner_position]
        .closed_reach_application
        .as_ref()
        .unwrap();
    let [
        terminal_psi::ClosedReachParameter::Machine(used),
        terminal_psi::ClosedReachParameter::Machine(unused),
    ] = application.telescope.as_slice()
    else {
        panic!("two selected family arguments");
    };
    assert!(used.schema.is_some());
    assert_eq!(
        used, unused,
        "selection metadata is module-wide, not call-site dependent"
    );
    assert_eq!(application.calls.len(), 2);
    assert!(application.calls.iter().all(|call| call.binder == 0));
    let application = module.machines[owner_position]
        .closed_reach_application
        .as_mut()
        .unwrap();
    let terminal_psi::ClosedReachParameter::Machine(unused) = &mut application.telescope[1] else {
        panic!("unused family argument");
    };
    unused.schema = None;
    assert!(
        terminal_verifier::validate_module_representation(&module).is_err(),
        "unused occurrence cannot contradict the same selection's retained schema"
    );
}

#[test]
fn generic_callback_schema_keeps_each_nested_selected_reach() {
    let checked = checked_source(
        r#"
        boundary trait Console { machine ping(); }
        boundary trait Callback { machine call(value: u64) -> u64 reaches Console; }
        machine schema<machine Step>(value: u64) -> u64
        where machine Step satisfies Callback::call;
        { Step(value) }
        machine outer<machine Schema>(value: u64) -> u64
        where machine Schema<machine Inner>(value: u64) -> u64
        where machine Inner satisfies Callback::call;
        reaches Console;
        { let first: u64 = Schema<quiet>(value); Schema<loud>(first) }
        machine quiet(value: u64) -> u64 satisfies Callback::call { value }
        machine loud(value: u64) -> u64 satisfies Callback::call reaches Console { 9 }
        pub machine enter(value: u64) -> u64 { outer<schema>(value) }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("nested schema applications")
    .into_artifact();
    drop(checked);
    let module =
        terminal_codec::decode_module(artifact.semantic_bytes()).expect("nested schema reload");
    let application = module
        .machines
        .iter()
        .filter_map(|machine| machine.closed_reach_application.as_ref())
        .find(|application| {
            application
                .calls
                .iter()
                .any(|call| call.application.is_some())
        })
        .expect("outer schema application");
    let calls = application
        .calls
        .iter()
        .map(|call| call.application.as_ref().expect("closed schema call"))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    let rows = calls
        .iter()
        .map(|call| {
            let callee = module
                .machines
                .iter()
                .find(|machine| machine.id == call.callee)
                .unwrap();
            service_names(&module, &callee.published_service_ceiling)
        })
        .collect::<Vec<_>>();
    assert_eq!(rows, [Vec::<&str>::new(), vec!["Console"]]);
    assert_ne!(calls[0].arguments, calls[1].arguments);
    assert_eq!(service_names(&module, &application.fixed), ["Console"]);
    let input = terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
        value: IntegerValue::Unsigned(7),
    };
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[input],
        )
        .expect("nested schema execution"),
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                value: IntegerValue::Unsigned(9),
            }
        )
    );
}

#[test]
fn generic_callback_schema_dependency_survives_private_forwarding() {
    let module = nominal_schema_forwarding_module();
    let forwarded = module
        .machines
        .iter()
        .filter_map(|machine| machine.closed_reach_application.as_ref())
        .find(|application| application.dependencies == [0] && application.calls.is_empty())
        .expect("forwarding retains dependency without fabricating a direct call");
    let terminal_psi::ClosedReachParameter::Machine(binding) = &forwarded.telescope[0] else {
        panic!("schema binder");
    };
    assert!(binding.schema.is_some());
    assert_eq!(service_names(&module, &binding.selected_reach), ["Console"]);
    terminal_verifier::validate_module_representation(&module)
        .expect("actual helper call closure covers dependency");
}

#[test]
fn incomplete_schema_projection_prunes_forwarded_dependencies_to_a_fixed_point() {
    let original = nominal_schema_forwarding_module();
    let mut unchanged = original.clone();
    crate::retention::closed_reach_applications::prune_incomplete_closed_reach_applications(
        &mut unchanged,
    );
    assert_eq!(unchanged, original, "complete coverage is preserved");

    let forwarded = original
        .machines
        .iter()
        .find(|machine| {
            machine
                .closed_reach_application
                .as_ref()
                .is_some_and(|application| {
                    application.dependencies == [0] && application.calls.is_empty()
                })
        })
        .expect("forwarding owner")
        .id;
    let selected = original
        .machines
        .iter()
        .find_map(|machine| {
            machine
                .closed_reach_application
                .as_ref()?
                .calls
                .iter()
                .find_map(|call| {
                    call.application
                        .as_ref()
                        .map(|application| application.callee)
                })
        })
        .expect("selected schema application");
    let mut partial = original.clone();
    // Model the producer's explicit absence of an inner projection. This is
    // not a claim that this valid source currently takes an unsupported route.
    let selected_owner = partial
        .machines
        .iter_mut()
        .find(|machine| machine.id == selected)
        .unwrap();
    selected_owner.closed_reach_application = None;
    for operation in selected_owner
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        operation.static_reach_binding = None;
    }
    assert!(
        matches!(
            terminal_verifier::validate_module_representation(&partial),
            Err(terminal_verifier::ModuleError::InvalidClosedReachApplication { .. })
        ),
        "partial coverage is not a valid published relation"
    );

    crate::retention::closed_reach_applications::prune_incomplete_closed_reach_applications(
        &mut partial,
    );
    assert!(
        partial
            .machines
            .iter()
            .find(|machine| machine.id == forwarded)
            .unwrap()
            .closed_reach_application
            .is_none(),
        "forwarding owner must also lose coverage"
    );
    terminal_verifier::validate_module_representation(&partial)
        .expect("ordinary semantics remain valid after incomplete projections are removed");
    let pruned = partial.clone();
    crate::retention::closed_reach_applications::prune_incomplete_closed_reach_applications(
        &mut partial,
    );
    assert_eq!(partial, pruned, "pruning reaches a stable fixed point");
    // Only annotations changed: restore them from the complete module to
    // compare all executable operations, service rows, and other contracts.
    for (machine, original_machine) in partial.machines.iter_mut().zip(&original.machines) {
        machine.closed_reach_application = original_machine.closed_reach_application.clone();
        for (block, original_block) in machine.blocks.iter_mut().zip(&original_machine.blocks) {
            for (operation, original_operation) in
                block.operations.iter_mut().zip(&original_block.operations)
            {
                operation.static_reach_binding = original_operation.static_reach_binding;
            }
        }
    }
    assert_eq!(partial, original);
}

#[test]
fn closed_callback_dependency_survives_source_discard() {
    let checked = checked_source(
        r#"
        boundary trait Console { machine ping(); }
        boundary trait Callback { machine call() reaches Console; }
        machine forward<machine Selected>()
        where machine Selected satisfies Callback::call;
        { Selected(); }
        machine selected() satisfies Callback::call reaches Console {}
        pub machine enter() { forward<selected>(); }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("closed callback product")
    .into_artifact();
    drop(checked);
    let module =
        terminal_codec::decode_module(artifact.semantic_bytes()).expect("source-free reload");
    let owner = module
        .machines
        .iter()
        .find(|machine| machine.closed_reach_application.is_some())
        .expect("original dependency must survive publication");
    let application = owner.closed_reach_application.as_ref().unwrap();
    assert_eq!(application.dependencies, [0]);
    assert!(application.fixed.is_empty());
    assert_eq!(application.calls.len(), 1);
    let terminal_psi::ClosedReachParameter::Machine(binding) = &application.telescope[0] else {
        panic!("selected machine binder");
    };
    assert_eq!(binding.selected_reach, owner.published_service_ceiling);
    assert_eq!(service_names(&module, &binding.selected_reach), ["Console"]);
    assert!(terminal_verifier::validate_module_representation(&module).is_ok());
}

#[test]
fn generic_template_commitment_retains_private_helper_reach_dependency() {
    let source = |helper_reach: &str, selected: &str| {
        format!(
            r#"
            boundary trait Console {{ machine ping(); }}
            boundary trait Callback {{ machine call() reaches Console; }}
            machine helper() {helper_reach} {{}}
            machine forward<machine Selected>()
            where machine Selected satisfies Callback::call;
            {{ helper(); Selected(); }}
            machine quiet() satisfies Callback::call {{}}
            machine loud() satisfies Callback::call reaches Console {{}}
            pub machine enter() {{ forward<{selected}>(); }}
            "#
        )
    };
    let quiet = checked_source(&source("", "quiet"));
    let additive = checked_source(&source("reaches Console", "quiet"));
    let selected = checked_source(&source("", "loud"));
    for checked in [&quiet, &additive, &selected] {
        let _artifact = terminal_production::TerminalProductionRequest::new(
            checked,
            terminal_production::TerminalMachineSelection::Name("enter"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("closed callback publication")
        .into_artifact();
    }
    let original = &quiet.machine_specializations[0];
    let changed = &additive.machine_specializations[0];
    assert_ne!(
        original.template_contract_commitment, changed.template_contract_commitment,
        "a private helper's additive reach belongs to the original generic contract"
    );
    assert_eq!(
        original.template_contract_commitment,
        selected.machine_specializations[0].template_contract_commitment,
        "a concrete callback selection must not change the original dependency"
    );

    let mut stale = additive.clone();
    let receipt = &mut stale.typed.machine_specializations[0];
    receipt.canonical_template_contract_bytes = original.canonical_template_contract_bytes.clone();
    receipt.template_contract_commitment = original.template_contract_commitment;
    receipt.template_contract_report_fingerprint = original.template_contract_report_fingerprint;
    let instance = receipt.instance;
    assert!(
        validation::recompute_checked_machine_specialization_commitment(&stale, instance).is_err(),
        "re-hashing a stale dependency cannot authorize the changed original graph"
    );
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &stale,
            terminal_production::TerminalMachineSelection::Name("enter")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
        .is_err()
    );
}

#[test]
fn generic_dependency_identity_ignores_call_order_and_helper_extraction() {
    let mut commitments = Vec::new();
    for body in [
        "helper(); First(); Second();",
        "Second(); helper(); First(); First();",
        "relay<First, Second>(); helper();",
    ] {
        for entries in [
            "pub machine enter() { forward<u64, quiet, 2, loud>(); } machine other() { forward<u64, loud, 2, quiet>(); }",
            "machine other() { forward<u64, loud, 2, quiet>(); } pub machine enter() { forward<u64, quiet, 2, loud>(); }",
        ] {
            let checked = checked_source(&format!(
                r#"
                boundary trait Console {{ machine ping(); }}
                boundary trait Callback {{ machine call() reaches Console; }}
                machine helper() reaches Console {{}}
                machine relay<machine Left, machine Right>()
                where machine Left satisfies Callback::call;
                where machine Right satisfies Callback::call;
                {{ Left(); Right(); }}
                machine forward<T [copy], machine First, const Count: u64, machine Second>()
                where machine First satisfies Callback::call;
                where machine Second satisfies Callback::call;
                {{ {body} }}
                machine quiet() satisfies Callback::call {{}}
                machine loud() satisfies Callback::call reaches Console {{}}
                {entries}
                "#
            ));
            let artifact = terminal_production::TerminalProductionRequest::new(
                &checked,
                terminal_production::TerminalMachineSelection::Name("enter"),
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default(),
            ))
            .expect("nested callback dependency publication")
            .into_artifact();
            let template = checked
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "forward")
                .expect("original forward");
            for specialization in checked
                .machine_specializations
                .iter()
                .filter(|specialization| specialization.template == template.symbol)
            {
                commitments.push(specialization.template_contract_commitment);
            }
            drop(checked);
            let module = terminal_codec::decode_module(artifact.semantic_bytes())
                .expect("source-free concrete dependency product");
            assert!(!module.root_service_reach.concrete.is_empty());
            let application = module
                .machines
                .iter()
                .filter_map(|machine| machine.closed_reach_application.as_ref())
                .find(|application| application.telescope.len() == 4)
                .expect("outer full telescope survives nested helper publication");
            assert_eq!(application.dependencies, [1, 3]);
            assert_eq!(service_names(&module, &application.fixed), ["Console"]);
            assert!(matches!(
                application.telescope[0],
                terminal_psi::ClosedReachParameter::Type { .. }
            ));
            assert!(matches!(
                application.telescope[2],
                terminal_psi::ClosedReachParameter::Const { .. }
            ));
            if body.starts_with("relay") {
                assert!(application.calls.is_empty());
                assert!(
                    module
                        .machines
                        .iter()
                        .filter_map(|machine| machine.closed_reach_application.as_ref())
                        .any(|application| application.telescope.len() == 2
                            && application.calls.len() == 2)
                );
            }
        }
    }
    assert_eq!(commitments.len(), 12);
    assert!(
        commitments
            .iter()
            .all(|commitment| *commitment == commitments[0])
    );
}

#[test]
fn generic_dependency_preserves_the_referenced_telescope_position() {
    let source = |selected: &str| {
        format!(
            r#"
        boundary trait Console {{ machine ping(); }}
        boundary trait Callback {{ machine call() reaches Console; }}
        machine forward<T [copy], machine First, const Count: u64, machine Second>()
        where machine First satisfies Callback::call;
        where machine Second satisfies Callback::call;
        {{ {selected}(); }}
        machine quiet() satisfies Callback::call {{}}
        pub machine enter() {{ forward<u64, quiet, 2, quiet>(); }}
        "#
        )
    };
    let first = checked_source(&source("First"));
    let second = checked_source(&source("Second"));
    for (checked, binder) in [(&first, 1), (&second, 3)] {
        let artifact = terminal_production::TerminalProductionRequest::new(
            checked,
            terminal_production::TerminalMachineSelection::Name("enter"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("one of two same-contract binders")
        .into_artifact();
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let application = module
            .machines
            .iter()
            .find_map(|machine| machine.closed_reach_application.as_ref())
            .unwrap();
        assert_eq!(application.telescope.len(), 4);
        assert_eq!(application.dependencies, [binder]);
        assert_eq!(application.calls[0].binder, binder);
    }
    let original = &first.machine_specializations[0];
    let mut stale = second.clone();
    let receipt = &mut stale.typed.machine_specializations[0];
    assert_ne!(
        original.template_contract_commitment,
        receipt.template_contract_commitment
    );
    receipt.canonical_template_contract_bytes = original.canonical_template_contract_bytes.clone();
    receipt.template_contract_commitment = original.template_contract_commitment;
    receipt.template_contract_report_fingerprint = original.template_contract_report_fingerprint;
    let instance = receipt.instance;
    assert!(
        validation::recompute_checked_machine_specialization_commitment(&stale, instance).is_err()
    );
}

#[test]
fn type_only_generic_template_retains_concrete_helper_dependency() {
    let mut commitments = Vec::new();
    for reach in ["", "reaches Console"] {
        let checked = checked_source(&format!(
            r#"
            boundary trait Console {{ machine ping(); }}
            machine helper(value: u64) -> u64 {reach} {{ value }}
            machine forward<T [copy]>(value: u64) -> u64 {{ helper(value) }}
            pub machine enter(value: u64) -> u64 {{ forward<u64>(value) }}
            "#
        ));
        let artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("enter"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("type-only generic helper dependency")
        .into_artifact();
        commitments.push(checked.machine_specializations[0].template_contract_commitment);
        drop(checked);
        let mut module = terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("source-free type-only application");
        let owner = module
            .machines
            .iter_mut()
            .find(|machine| machine.closed_reach_application.is_some())
            .expect("type-only applications retain their original concrete dependency");
        let application = owner.closed_reach_application.as_mut().unwrap();
        assert!(matches!(
            application.telescope.as_slice(),
            [terminal_psi::ClosedReachParameter::Type { .. }]
        ));
        assert!(application.dependencies.is_empty());
        assert!(application.calls.is_empty());
        assert_eq!(application.fixed, owner.published_service_ceiling);
        if !application.fixed.is_empty() {
            application.fixed.clear();
            assert!(terminal_verifier::validate_module_representation(&module).is_err());
        }
    }
    assert_ne!(commitments[0], commitments[1]);
}

#[test]
fn const_only_application_retains_its_fixed_dependency_after_reload() {
    let checked = checked_source(
        r#"
        boundary trait Console { machine ping(); }
        machine identity<const Count: u64>(value: u64) -> u64 reaches Console { value }
        pub machine enter(value: u64) -> u64 { identity<2>(value) }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("const-only generic product")
    .into_artifact();
    drop(checked);
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload constant");
    let owner = module
        .machines
        .iter()
        .find(|machine| machine.closed_reach_application.is_some())
        .expect("retained const application");
    let application = owner.closed_reach_application.as_ref().unwrap();
    assert!(matches!(
        application.telescope.as_slice(),
        [terminal_psi::ClosedReachParameter::Const { .. }]
    ));
    assert!(application.dependencies.is_empty());
    assert!(application.calls.is_empty());
    assert_eq!(service_names(&module, &application.fixed), ["Console"]);
    assert_eq!(application.fixed, owner.published_service_ceiling);
    terminal_verifier::validate_module_representation(&module).expect("fixed-only reach replay");
}

#[test]
fn ordinary_callback_publication_replays_its_exact_specialization() {
    for source in [
        r#"
            boundary trait Callback { machine call(); }
            machine forward<machine Selected>()
            where machine Selected satisfies Callback::call;
            { Selected(); }
            machine selected() satisfies Callback::call {}
            machine alternative() satisfies Callback::call {}
            pub machine enter() { forward<selected>(); }
        "#,
        r#"
            boundary trait Callback { machine call(value: u64) -> u64; }
            machine forward<machine Selected>(value: u64) -> u64
            where machine Selected satisfies Callback::call;
            { Selected(value) }
            machine selected(value: u64) -> u64 satisfies Callback::call { value }
            machine alternative(value: u64) -> u64 satisfies Callback::call { value }
            pub machine enter(value: u64) -> u64 { forward<selected>(value) }
        "#,
    ] {
        let checked = checked_source(source);
        let _artifact = terminal_production::TerminalProductionRequest::new(
            &checked,
            terminal_production::TerminalMachineSelection::Name("enter"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .expect("valid ordinary closed callback")
        .into_artifact();
        assert_eq!(checked.machine_specializations.len(), 1);
        let alternative = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "alternative")
            .expect("alternative");
        let alternative_state = checked.machine_states(alternative)[0].symbol;
        let instance = checked.machine_specializations[0].instance;
        for mutation in 0..8 {
            let mut invalid = checked.clone();
            let specialization = &mut invalid.typed.machine_specializations[0];
            match mutation {
                0 => specialization.commitment = Default::default(),
                1 => specialization.normalized_template_identity = "forged-template".into(),
                2 => specialization.template_parameters = Default::default(),
                3 => specialization.machine_arguments[0] = alternative_state,
                4 => specialization.machine_argument_contract_commitments[0] = [0; 32],
                5 => specialization.canonical_template_contract_bytes.push(0),
                6 => specialization
                    .type_argument_identities
                    .push("unselected-type".into()),
                _ => invalid.typed.machine_specializations.clear(),
            }
            if matches!(mutation, 1 | 2 | 4) {
                assert!(
                    validation::recompute_checked_machine_specialization_commitment(
                        &invalid, instance
                    )
                    .is_err(),
                    "re-hashing cannot authorize a different live template or recorded contract: {mutation}"
                );
            }
            assert!(
                terminal_production::TerminalProductionRequest::new(
                    &invalid,
                    terminal_production::TerminalMachineSelection::Name("enter")
                )
                .produce(TerminalProductionCustody::artifact_only(
                    &mut TerminalProductionTimings::default()
                ))
                .is_err(),
                "ordinary publication must reject stale specialization custody: {mutation}"
            );
        }
    }
}

#[test]
fn isolated_callback_publication_replays_its_specialization() {
    let checked = checked_source(
        r#"
        machine identity<T [copy]>(value: u64) -> u64 { value }
        pub machine enter(value: u64) -> u64 { identity<u64>(value) }
    "#,
    );
    let instance = checked.machine_specializations[0].instance;
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == instance)
        .expect("instance");
    let entry = checked.machine_states(machine)[0].symbol;
    let callback = lower_bounded_callback_identity_machine(&checked, instance, entry)
        .expect("valid isolated generic callback");
    assert!(
        callback.terminal.semantic_module.machines[0]
            .closed_reach_application
            .is_some(),
        "isolated callbacks retain the same closed application as ordinary publication"
    );
    let mut invalid = checked.clone();
    invalid.typed.machine_specializations[0].commitment = Default::default();
    assert!(
        lower_bounded_callback_identity_machine(&invalid, instance, entry).is_err(),
        "isolating a callback cannot bypass specialization custody"
    );
}

#[test]
fn isolated_callback_retains_unused_selections_without_emitting_their_bodies() {
    let source = r#"
        boundary trait Console { machine ping(); }
        boundary trait Callback { machine call() reaches Console; }
        machine identity<T [copy], machine Unused, const Count: u64>(value: u64) -> u64
        where machine Unused satisfies Callback::call;
        { value }
        machine selected() satisfies Callback::call reaches Console {}
        pub machine enter(value: u64) -> u64 { identity<u64, selected, 2>(value) }
    "#;
    for family_parameters in ["", "<const Number: u64>"] {
        let source = source
            .replace(
                "machine call()",
                &format!("machine call{family_parameters}()"),
            )
            .replace(
                "machine selected()",
                &format!("machine selected{family_parameters}()"),
            );
        let checked = checked_source(&source);
        let instance = checked.machine_specializations[0].instance;
        let source_machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == instance)
            .expect("closed identity owner");
        let entry = checked.machine_states(source_machine)[0].symbol;
        let lowered = lower_bounded_callback_identity_machine(&checked, instance, entry)
            .expect("isolated closed identity");
        drop(checked);
        let optimized = run_psi_optimization(lowered.terminal, Default::default())
            .expect("ordinary callback optimization boundary");
        let artifact = finalize_terminal_artifact(&optimized).expect("publish isolated callback");
        let mut module = terminal_codec::decode_module(artifact.semantic_bytes())
            .expect("reload without checked selection custody");
        assert_eq!(
            module.machines.len(),
            1,
            "unused selection does not manufacture a body"
        );
        let owner = &module.machines[0];
        let application = owner
            .closed_reach_application
            .as_ref()
            .expect("retained complete telescope");
        assert!(owner.published_service_ceiling.is_empty());
        assert!(application.fixed.is_empty());
        assert!(application.dependencies.is_empty());
        assert!(application.calls.is_empty());
        let [
            terminal_psi::ClosedReachParameter::Type { .. },
            terminal_psi::ClosedReachParameter::Machine(binding),
            terminal_psi::ClosedReachParameter::Const { .. },
        ] = application.telescope.as_slice()
        else {
            panic!("original ordered type, machine, const telescope");
        };
        assert_eq!(service_names(&module, &binding.upper_bound), ["Console"]);
        assert_eq!(service_names(&module, &binding.selected_reach), ["Console"]);
        assert_eq!(binding.callee, None);
        let input = terminal_interpreter::TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            value: IntegerValue::Unsigned(7),
        };
        assert_eq!(
            terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[input],
            )
            .expect("source-free isolated identity"),
            terminal_interpreter::TerminalExecutionResult::Scalar(input)
        );
        let application = module.machines[0]
            .closed_reach_application
            .as_mut()
            .unwrap();
        let terminal_psi::ClosedReachParameter::Machine(binding) = &mut application.telescope[1]
        else {
            panic!("machine binder");
        };
        binding.upper_bound.clear();
        assert!(
            terminal_verifier::validate_module_representation(&module).is_err(),
            "unused selection still has to satisfy its retained bound"
        );
    }
}

#[test]
fn nested_generic_callbacks_replay_interleaved_telescope_positions() {
    let checked = checked_source(
        r#"
        boundary trait Callback { machine call(value: u64) -> u64; }
        machine relay<T [copy], machine First, const Count: u64, machine Second>(value: u64) -> u64
        where machine First satisfies Callback::call;
        where machine Second satisfies Callback::call;
        { let intermediate: u64 = First(value); Second(intermediate) }
        machine wrapper<machine First, machine Second>(value: u64) -> u64
        where machine First satisfies Callback::call;
        where machine Second satisfies Callback::call;
        { relay<u64, First, 2, Second>(value) }
        machine first(value: u64) -> u64 satisfies Callback::call { 3 }
        machine second(value: u64) -> u64 satisfies Callback::call { value }
        pub machine enter(value: u64) -> u64 { wrapper<first, second>(value) }
    "#,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("nested closed callbacks")
    .into_artifact();
    assert_eq!(checked.machine_specializations.len(), 2);
    for specialization in &checked.machine_specializations {
        let mut invalid = checked.clone();
        invalid
            .typed
            .machine_specializations
            .iter_mut()
            .find(|candidate| candidate.instance == specialization.instance)
            .expect("instance")
            .machine_arguments
            .swap(0, 1);
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &invalid,
                terminal_production::TerminalMachineSelection::Name("enter")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "same-contract selections retain binder positions"
        );
    }
    drop(checked);
    assert_eq!(
        terminal_interpreter::interpret_terminal_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                value: IntegerValue::Unsigned(9),
            }],
        )
        .expect("source-free nested selection"),
        terminal_interpreter::TerminalExecutionResult::Scalar(
            terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
                value: IntegerValue::Unsigned(3),
            }
        )
    );
}
