use super::typed_source;
use crate::CheckingRequest;
use crate::tests::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use checked_trees::{ContractProofFactKind, ContractProofFactOwner};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};

#[test]
fn exact_requirement_lifetime_application_retains_raw_machine_ordinals() {
    let source = r#"
        trait Pair<'left, 'right> {
            machine choose(first: &'left [u8], second: &'right [u8]) -> &'left [u8];
        }

        machine choose<'unused, 'x, 'y>(
            first: &'x [u8],
            second: &'y [u8]
        ) -> &'x [u8]
            satisfies Pair<'x, 'y>::choose
        {
            first
        }
    "#;
    let typed = typed_source(source).expect("typed exact requirement application");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .expect("realizing machine");
    let [realization] = typed.machine_trait_conformances(machine) else {
        panic!("one exact requirement realization")
    };
    assert_eq!(realization.trait_lifetime_arguments, [1, 2]);
    assert_eq!(
        typed_trees::machine::normalize_requirement_lifetime_partition(
            &realization.trait_lifetime_arguments,
        ),
        [0, 1],
    );
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("declared lifetime substitution should validate");
}

#[test]
fn exact_requirement_lifetime_application_accepts_repeated_realizer_binder() {
    let source = r#"
        trait Pair<'left, 'right> {
            machine consume(first: &'left [u8], second: &'right [u8]);
        }

        machine consume<'x>(first: &'x [u8], second: &'x [u8])
            satisfies Pair<'x, 'x>::consume
        {
        }
    "#;
    let typed = typed_source(source).expect("typed repeated lifetime application");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("realizing machine");
    let [realization] = typed.machine_trait_conformances(machine) else {
        panic!("one exact requirement realization")
    };
    assert_eq!(realization.trait_lifetime_arguments, [0, 0]);
    assert_eq!(
        typed_trees::machine::normalize_requirement_lifetime_partition(
            &realization.trait_lifetime_arguments,
        ),
        [0, 0],
    );
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("repeated lifetime substitution should validate");
}

#[test]
fn exact_requirement_lifetime_application_rejects_signature_substitution_drift() {
    let source = r#"
        trait Pair<'left, 'right> {
            machine choose(first: &'left [u8], second: &'right [u8]) -> &'left [u8];
        }

        machine choose<'x, 'y>(first: &'x [u8], second: &'y [u8]) -> &'x [u8]
            satisfies Pair<'y, 'x>::choose
        {
            first
        }
    "#;
    let typed = typed_source(source).expect("typed mismatched lifetime application");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("lifetime drift must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("lifetime application does not match the declared `satisfies` edge")
    }));
}

#[test]
fn exact_requirement_lifetime_application_requires_complete_in_scope_arguments() {
    let missing = r#"
        trait Reads<'view> {
            machine read(value: &'view [u8]) -> &'view [u8];
        }
        machine read<'scope>(value: &'scope [u8]) -> &'scope [u8]
            satisfies Reads::read
        {
            value
        }
    "#;
    let typed = typed_source(missing).expect("typing retains missing application for validation");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("missing lifetime must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("expects 1 target-trait lifetime argument(s), got 0")
    }));

    let foreign = r#"
        trait Reads<'view> {
            machine read(value: &'view [u8]) -> &'view [u8];
        }
        machine read<'scope>(value: &'scope [u8]) -> &'scope [u8]
            satisfies Reads<'foreign>::read
        {
            value
        }
    "#;
    let tokens = Lexer::new(foreign).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let error = lower_symbol_resolved_trees(&resolved)
        .expect_err("foreign lifetime argument must reject during typed lowering");
    assert!(error.message.contains("outside its lifetime telescope"));
}

#[test]
fn concrete_subjectless_conformance_checks_as_carrierless_evidence() {
    let source = r#"
        trait Evidence {
            machine witness(value: i32);
        }

        ConcreteEvidence: satisfies Evidence {
            machine witness(value: i32) { }
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("subjectless evidence rows should validate");
}

/// MP1: the machine-parameter requirement is semantic tree data. It is
/// populated once from the declaration and copied through the resolved tree
/// into the typed tree; later rungs consume it for modular checking and
/// specialization.
#[test]
fn machine_parameter_contract_survives_resolved_and_typed_trees() {
    let source = r#"
        data Deck {}

        machine Deck::best<T, machine Key>(&self) -> u64
        where machine Key(value: &T) -> u64
        {
            0
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");

    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let typed_machine = typed
        .machines()
        .iter()
        .find(|machine| !typed.machine_type_parameters(machine).is_empty())
        .expect("typed generic machine");
    let typed_parameters = typed.machine_type_parameters(typed_machine);
    assert_eq!(typed_parameters.len(), 2);
    let typed_trees::data::TypeParameterKind::Machine { contract } = &typed_parameters[1].kind
    else {
        panic!("typed Key should remain a machine parameter");
    };
    let contract = typed
        .machine_parameter_contract_view(contract)
        .expect("structural Key contract")
        .signature();
    assert_eq!(contract.name.as_str(), "Key");
    assert_eq!(typed.state_signature_parameters(contract).len(), 1);
    assert!(contract.return_type.is_valid());
}

#[test]
fn nested_structural_machine_parameter_emits_exact_checked_evidence() {
    let source = r#"
        machine outer<machine Schema>()
        where machine Schema<machine Nested>()
        where machine Nested(value: bool)
            requires value
            crashes Abort
                value;
        {
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let outer = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "outer")
        .expect("outer machine");
    let schema = typed
        .machine_type_parameters(outer)
        .first()
        .expect("Schema parameter");
    let typed_trees::data::TypeParameterKind::Machine {
        contract: typed_trees::data::MachineParameterContract::Structural(schema_signature),
    } = &schema.kind
    else {
        panic!("Schema should retain a structural signature")
    };
    let nested = typed
        .state_signature_type_parameters(schema_signature)
        .first()
        .expect("Nested parameter");
    let typed_trees::data::TypeParameterKind::Machine {
        contract: typed_trees::data::MachineParameterContract::Structural(nested_signature),
    } = &nested.kind
    else {
        panic!("Nested should retain a structural signature")
    };
    let nested_owner = nested.symbol;
    let nested_state = nested_signature.symbol;

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("nested structural evidence should check");

    assert!(checked.facts.proof.contract_facts.iter().any(|(_, fact)| {
        fact.kind == ContractProofFactKind::Requires
            && fact.owner
                == ContractProofFactOwner::StateSignature {
                    owner_symbol: nested_owner,
                    state_symbol: nested_state,
                }
    }));
    assert!(
        checked
            .facts
            .contract_plans
            .crash_capsule(nested_owner, nested_state)
            .is_some(),
        "the nested binder should own a capsule keyed by its exact signature state"
    );
}

#[test]
fn nested_nominal_machine_parameter_uses_trait_evidence_without_binder_expansion() {
    let source = r#"
        trait Handler {
            machine call(value: bool)
                requires value
                crashes Abort
                    value;
        }

        machine outer<machine Schema>()
        where machine Schema<machine Nested>()
        where machine Nested satisfies Handler::call;
        {
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");

    let outer = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "outer")
        .expect("outer machine");
    let schema = typed
        .machine_type_parameters(outer)
        .first()
        .expect("Schema parameter");
    let typed_trees::data::TypeParameterKind::Machine {
        contract: typed_trees::data::MachineParameterContract::Structural(schema_signature),
    } = &schema.kind
    else {
        panic!("Schema should retain a structural signature")
    };
    let nested = typed
        .state_signature_type_parameters(schema_signature)
        .first()
        .expect("Nested parameter");
    assert!(matches!(
        nested.kind,
        typed_trees::data::TypeParameterKind::Machine {
            contract: typed_trees::data::MachineParameterContract::Nominal { .. }
        }
    ));
    let nested_owner = nested.symbol;

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("nested nominal reference should check");

    assert!(!checked.facts.proof.contract_facts.iter().any(|(_, fact)| {
        matches!(
            fact.owner,
            ContractProofFactOwner::StateSignature { owner_symbol, .. }
                if owner_symbol == nested_owner
        )
    }));
    assert!(
        checked
            .facts
            .contract_plans
            .crash_capsules
            .iter()
            .all(|capsule| capsule.target_machine() != nested_owner),
        "a nested nominal binder must not duplicate its trait requirement capsule"
    );
}

#[test]
fn nominal_machine_parameter_accepts_one_explicit_exact_satisfaction_row() {
    let source = r#"
        trait Handler {
            machine call(value: i32) -> i32;
        }

        machine chosen(value: i32) -> i32
        satisfies Handler::call
        {
            value
        }

        machine register<machine Selected>(value: i32) -> i32
        where machine Selected satisfies Handler::call;
        {
            Selected(value)
        }

        machine caller(value: i32) -> i32 {
            register<chosen>(value)
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let register = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "register")
        .expect("register template");
    let selected = typed
        .machine_type_parameters(register)
        .first()
        .expect("Selected parameter");
    let typed_trees::data::TypeParameterKind::Machine { contract } = &selected.kind else {
        panic!("Selected should be a machine parameter")
    };
    assert!(matches!(
        typed.machine_parameter_contract_view(contract),
        Some(typed_trees::data::MachineParameterContractView::Nominal {
            trait_definition,
            requirement,
        }) if trait_definition.name.as_str() == "Handler"
            && requirement.name.as_str() == "call"
    ));
    let register_symbol = register.symbol;
    let register_entry = typed.machine_states(register)[0].symbol;
    let chosen = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "chosen")
        .expect("chosen machine");
    let chosen_symbol = chosen.symbol;
    let chosen_entry = typed.machine_states(chosen)[0].symbol;
    let (satisfaction_trait, satisfaction_requirement, canonical_requirement_overload) =
        match typed.machine_parameter_contract_view(contract) {
            Some(typed_trees::data::MachineParameterContractView::Nominal {
                trait_definition,
                requirement,
            }) => (
                trait_definition.symbol,
                requirement.symbol,
                typed
                    .normalized_trait_requirement_overload_identity(trait_definition, requirement)
                    .identity(),
            ),
            _ => unreachable!("Selected has a nominal contract"),
        };

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("an explicitly satisfied exact nominal requirement should specialize");
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| specialization.template == register_symbol
                && specialization.machine_arguments.len() == 1)
    );
    assert_eq!(checked.facts.nominal_machine_uses.uses.len(), 1);
    let nominal_use = &checked.facts.nominal_machine_uses.uses[0];
    assert_eq!(nominal_use.registration_operation, register_entry);
    assert_eq!(nominal_use.static_machine_ordinal, 0);
    assert_eq!(nominal_use.selected_machine, chosen_symbol);
    assert_eq!(nominal_use.selected_entry, chosen_entry);
    assert_eq!(nominal_use.satisfaction_trait, satisfaction_trait);
    assert_eq!(
        nominal_use.satisfaction_requirement,
        satisfaction_requirement
    );
    assert_eq!(
        nominal_use.canonical_requirement_overload,
        canonical_requirement_overload
    );
    let published_fingerprint = checked
        .facts
        .contract_plans
        .crash_capsule(satisfaction_trait, satisfaction_requirement)
        .expect("published nominal requirement capsule")
        .target_contract_report_fingerprint();
    let actual_fingerprint = checked
        .facts
        .contract_plans
        .for_machine(chosen_symbol)
        .expect("selected machine contract plan")
        .report_fingerprint;
    assert_eq!(
        nominal_use
            .published_requirement_envelope
            .contract_report_fingerprint,
        published_fingerprint
    );
    assert_eq!(
        nominal_use
            .selected_actual_envelope
            .contract_report_fingerprint,
        actual_fingerprint
    );
    assert_eq!(
        nominal_use
            .refinement
            .published_requirement_report_fingerprint,
        published_fingerprint
    );
    assert_eq!(
        nominal_use.refinement.selected_actual_report_fingerprint,
        actual_fingerprint
    );
    assert_eq!(nominal_use.callback_placement, None);
}

#[test]
fn public_installation_wrapper_keeps_upper_bound_separate_from_concrete_reach() {
    for authored_reach in ["", "reaches PortIo"] {
        let source = format!(
            r#"
            pub boundary trait MachineControl {{}}
            pub boundary trait PortIo {{}}
            pub boundary trait InterruptCompletion {{
                machine complete() -> u64
                reaches <= MachineControl + PortIo;
            }}
            machine invoke<machine Completion>() -> u64
            where machine Completion satisfies InterruptCompletion::complete;
            {{ Completion() }}
            pub machine outer<machine Completion>() -> u64
            where machine Completion satisfies InterruptCompletion::complete;
            {authored_reach}
            {{ invoke<Completion>() }}
            "#,
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize public installation wrapper");
        let syntax = parse_syntax_trees(&tokens).expect("parse public installation wrapper");
        let resolved =
            resolve(ResolutionRequest::new(&syntax)).expect("resolve public installation wrapper");
        let typed =
            lower_symbol_resolved_trees(&resolved).expect("type public installation wrapper");
        let checked = lower_typed_trees(typed, &CheckingRequest::settled())
            .expect("check public installation wrapper");
        let wrapper = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "outer")
            .expect("public wrapper");
        let requirement = checked
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "InterruptCompletion")
            .and_then(|definition| checked.trait_machine_signatures(definition).first())
            .expect("exact installation requirement");
        let reaches = &checked.facts.service_reaches;
        let summary = reaches
            .for_machine(wrapper.symbol)
            .expect("public reach summary");
        assert_eq!(
            summary.interface,
            language_semantics::ServiceReachInterface::PublishedCeiling(summary.published_ceiling),
        );
        assert_eq!(
            summary.unresolved_installation_reaches,
            [flow_effects::InstallationReachRequirement {
                requirement: requirement.symbol,
                upper_bound: requirement.service_reach_row,
            }],
        );
        let names = |row| {
            reaches
                .rows
                .services(row)
                .iter()
                .map(|service| {
                    reaches
                        .services
                        .definition(*service)
                        .expect("canonical service")
                        .name
                        .as_str()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(names(summary.effective), ["MachineControl", "PortIo"]);
        assert_eq!(
            names(summary.published_ceiling),
            ["MachineControl", "PortIo"]
        );
        assert!(names(summary.concrete_transitive).is_empty());
        let expected_concrete = if authored_reach.is_empty() {
            vec![]
        } else {
            vec!["PortIo"]
        };
        assert_eq!(names(summary.concrete_effective), expected_concrete);
        let envelope = checked
            .facts
            .contract_plans
            .realized_envelope(wrapper.symbol)
            .expect("public realized envelope");
        assert_eq!(envelope.concrete_service_reach, expected_concrete);
        assert_eq!(
            envelope.unresolved_installation_reaches,
            summary.unresolved_installation_reaches
        );
    }
}

#[test]
fn bounded_installation_reach_retains_exact_unresolved_requirement_through_checked_facts() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}

        boundary trait InterruptCompletion {
            machine complete() -> u64
            reaches <= MachineControl + PortIo;
        }

        machine pic_complete() -> u64
        satisfies InterruptCompletion::complete
        reaches PortIo
        {
            0
        }

        machine invoke<machine Completion>() -> u64
        where machine Completion satisfies InterruptCompletion::complete;
        {
            Completion()
        }

        machine outer<machine Completion>() -> u64
        where machine Completion satisfies InterruptCompletion::complete;
        {
            invoke<Completion>()
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let requirement = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "InterruptCompletion")
        .and_then(|definition| typed.trait_machine_signatures(definition).first())
        .expect("InterruptCompletion::complete");
    let requirement_symbol = requirement.symbol;
    let upper_bound = requirement.service_reach_row;
    let invoke_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "invoke")
        .expect("invoke machine")
        .symbol;
    let outer_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "outer")
        .expect("outer machine")
        .symbol;

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("bounded reach closure should check");
    let reach = checked
        .facts
        .service_reaches
        .for_machine(invoke_symbol)
        .expect("invoke reach facts");

    assert_eq!(
        reach.unresolved_installation_reaches,
        [flow_effects::InstallationReachRequirement {
            requirement: requirement_symbol,
            upper_bound,
        }]
    );
    let names = checked
        .facts
        .service_reaches
        .rows
        .services(reach.effective)
        .iter()
        .filter_map(|service| checked.facts.service_reaches.services.definition(*service))
        .map(|definition| definition.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, ["MachineControl", "PortIo"]);
    assert!(
        checked
            .facts
            .service_reaches
            .rows
            .services(reach.concrete_effective)
            .is_empty(),
        "the abstract upper bound must not enter concrete reach"
    );
    assert!(
        checked
            .facts
            .service_reaches
            .rows
            .services(reach.concrete_transitive)
            .is_empty(),
        "the checked body's preselection reach must exclude the abstract upper bound"
    );
    let outer_reach = checked
        .facts
        .service_reaches
        .for_machine(outer_symbol)
        .expect("outer reach facts");
    assert_eq!(
        outer_reach.unresolved_installation_reaches,
        reach.unresolved_installation_reaches
    );
    assert!(
        checked
            .facts
            .service_reaches
            .rows
            .services(outer_reach.concrete_transitive)
            .is_empty(),
        "a wrapper must preserve the callee's unresolved upper bound distinction"
    );
    assert_eq!(
        checked
            .facts
            .contract_plans
            .realized_envelope(outer_symbol)
            .expect("outer realized envelope")
            .unresolved_installation_reaches,
        reach.unresolved_installation_reaches
    );
    assert!(
        checked
            .facts
            .contract_plans
            .realized_envelope(outer_symbol)
            .expect("outer realized envelope")
            .concrete_service_reach
            .is_empty()
    );
}

#[test]
fn top_level_bounded_reach_is_unresolved_not_concrete() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}
        pub data Completion {}

        pub boundary requirement Completion::complete() -> u64
        reaches <= MachineControl + PortIo;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    assert!(
        resolved
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "Completion::complete")
            .expect("resolved complete requirement")
            .service_reach_is_installation_bound
    );
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let complete = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Completion::complete")
        .expect("complete requirement");
    assert_eq!(
        complete.supply_mode,
        language_semantics::MachineSupplyMode::TopLevelRequirement
    );
    let complete_symbol = complete.symbol;
    let upper_bound = complete.service_reach_row;
    assert!(complete.service_reach_is_installation_bound);

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("bounded reach closure should check");
    let complete_reach = checked
        .facts
        .service_reaches
        .for_machine(complete_symbol)
        .expect("complete reach facts");
    let expected = [flow_effects::InstallationReachRequirement {
        requirement: complete_symbol,
        upper_bound,
    }];
    assert_eq!(complete_reach.unresolved_installation_reaches, expected);
    assert!(
        checked
            .facts
            .service_reaches
            .rows
            .services(complete_reach.concrete_effective)
            .is_empty(),
        "the abstract upper bound must not enter the requirement's concrete reach"
    );
}

#[test]
fn bounded_installation_reach_rejects_provider_outside_upper_bound() {
    let source = r#"
        boundary trait MachineControl {}
        boundary trait PortIo {}
        boundary trait FilesystemHost {}

        boundary trait InterruptCompletion {
            machine complete() -> u64
            reaches <= MachineControl + PortIo;
        }

        machine invalid_complete() -> u64
        satisfies InterruptCompletion::complete
        reaches FilesystemHost
        {
            0
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("provider reach outside an installation bound must reject");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("service `FilesystemHost`")
            && diagnostic
                .message
                .contains("is not allowed by the trait requirement")
    }));
}

#[test]
fn nominal_callback_use_retains_exact_evaluated_placement_identity() {
    let source = r#"
        pub boundary trait Handler {
            machine call(value: i32) -> i32;
        }

        boundary machine chosen(value: i32) -> i32
        satisfies Handler::call
        {
            value
        }

        boundary machine register<machine Selected>(value: i32) -> i32
        where machine Selected satisfies Handler::call;
        {
            Selected(value)
        }

        machine caller(value: i32) -> i32 {
            register<chosen>(value)
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let handler = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Handler")
        .expect("Handler boundary trait");
    let handler_symbol = handler.symbol;
    let requirement_symbol = typed
        .trait_machine_signatures(handler)
        .first()
        .expect("Handler::call")
        .symbol;
    let expected_fingerprint = 0x2a7c_6b19_d331_85e1;
    typed.record_boundary_calling_plan(typed_trees::typed_trees::BoundaryCallingPlanIdentity {
        boundary_trait: handler_symbol,
        boundary_arguments: Vec::new(),
        requirement_machine: requirement_symbol,
        report_fingerprint: expected_fingerprint,
        commitment: typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest(
            [0x2a; 32],
        ),
    });

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("nominal callback selection should check");
    let [nominal_use] = checked.facts.nominal_machine_uses.uses.as_slice() else {
        panic!("one nominal callback use")
    };

    let callback_placement = nominal_use
        .callback_placement
        .expect("nominal callback placement identity");
    assert_eq!(
        callback_placement.boundary_calling_plan_report_fingerprint,
        expected_fingerprint
    );
    let published = checked
        .facts
        .contract_plans
        .crash_capsule(
            nominal_use.satisfaction_trait,
            nominal_use.satisfaction_requirement,
        )
        .expect("callback requirement envelope");
    assert_eq!(
        published.target_contract_report_fingerprint(),
        nominal_use
            .published_requirement_envelope
            .contract_report_fingerprint
    );
    assert!(published.published_service_reach().is_empty());
    assert!(published.published_synchronous_invocations().is_empty());
    assert!(!published.published_may_suspend());
    assert!(!published.published_may_block());
    assert_eq!(
        published.published_termination(),
        &language_semantics::TerminationGuarantee::NoGuarantee
    );

    let actual = checked
        .facts
        .contract_plans
        .realized_envelope(nominal_use.selected_machine)
        .expect("selected callback actual envelope");
    assert_eq!(
        actual.contract_report_fingerprint,
        nominal_use
            .selected_actual_envelope
            .contract_report_fingerprint
    );
    assert!(actual.effective_service_reach.is_empty());
    assert!(actual.effective_synchronous_invocations.is_empty());
    assert!(!actual.checked_may_suspend);
    assert!(!actual.checked_may_block);
    assert!(actual.capabilities.is_empty());
    let resources = checked
        .facts
        .contract_plans
        .resource_envelope(nominal_use.selected_machine, nominal_use.selected_entry)
        .expect("selected callback checked resource anchor");
    assert_eq!(resources.machine(), nominal_use.selected_machine);
    assert_eq!(resources.entry(), nominal_use.selected_entry);
    assert_eq!(
        resources.contract_report_fingerprint(),
        nominal_use
            .selected_actual_envelope
            .contract_report_fingerprint
    );
    assert_eq!(
        resources.stack().derivation_obligation(),
        checked_trees::CheckedResourceDerivationObligation::TerminalAndTargetStackClosure
    );
    assert_eq!(
        resources.logical_structural_work().derivation_obligation(),
        checked_trees::CheckedResourceDerivationObligation::TerminalControlAndFuelSchedule
    );
    assert_eq!(
        resources.machine_state().derivation_obligation(),
        checked_trees::CheckedResourceDerivationObligation::SelectedInstructionMachineStateFootprint
    );
    callback_placement
        .resource_receipt
        .validate_against(resources)
        .expect("callback placement must retain the exact selected entry resource anchor");
    checked
        .facts
        .contract_plans
        .validate_resource_envelopes()
        .expect("resource anchors must independently replay from exact contracts");
}

#[test]
fn checked_resource_envelopes_cover_entries_in_declaration_order() {
    let source = r#"
        data Token { value: i32; }

        machine route(first: Token, second: Token, choose_first: bool) -> i32
        {
            transition choose_first {
                true -> keep_first(first)
                _ -> keep_second(second)
            }

            state keep_first(first: Token) -> i32 { 1 }
            state keep_second(second: Token) -> i32 { 2 }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "route")
        .expect("route machine");
    let machine_symbol = machine.symbol;
    let entries = typed
        .machine_states(machine)
        .iter()
        .map(|entry| entry.symbol)
        .collect::<Vec<_>>();

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("multi-entry resource anchors should check");
    let realized = checked
        .facts
        .contract_plans
        .realized_envelope(machine_symbol)
        .expect("route realized envelope");
    assert_eq!(
        realized
            .resources
            .iter()
            .map(checked_trees::CheckedEntryResourceEnvelope::entry)
            .collect::<Vec<_>>(),
        entries
    );
    for entry in entries {
        let resource = checked
            .facts
            .contract_plans
            .resource_envelope(machine_symbol, entry)
            .expect("every owned entry has one resource anchor");
        assert_eq!(resource.machine(), machine_symbol);
        assert_eq!(resource.entry(), entry);
        resource.validate().expect("entry anchor replays");
    }
}

#[test]
fn nominal_machine_use_identity_survives_forwarded_specialization_rounds() {
    let source = r#"
        trait Handler {
            machine call(value: i32) -> i32;
        }

        machine chosen(value: i32) -> i32
        satisfies Handler::call
        {
            value
        }

        machine inner<machine Selected>(value: i32) -> i32
        where machine Selected satisfies Handler::call;
        {
            Selected(value)
        }

        machine outer<machine Forwarded>(value: i32) -> i32
        where machine Forwarded satisfies Handler::call;
        {
            inner<Forwarded>(value)
        }

        machine caller(value: i32) -> i32 {
            outer<chosen>(value)
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let chosen_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "chosen")
        .expect("chosen machine")
        .symbol;

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("a forwarded exact nominal requirement should specialize transitively");
    let selected_uses = checked
        .facts
        .nominal_machine_uses
        .uses
        .iter()
        .filter(|nominal_use| nominal_use.selected_machine == chosen_symbol)
        .collect::<Vec<_>>();

    assert_eq!(selected_uses.len(), 2);
    assert_ne!(
        selected_uses[0].registration_operation,
        selected_uses[1].registration_operation
    );
    assert!(selected_uses.iter().all(|nominal_use| {
        nominal_use
            .published_requirement_envelope
            .contract_report_fingerprint
            == nominal_use
                .refinement
                .published_requirement_report_fingerprint
            && nominal_use
                .selected_actual_envelope
                .contract_report_fingerprint
                == nominal_use.refinement.selected_actual_report_fingerprint
    }));
    assert_eq!(
        selected_uses[0].published_requirement_envelope,
        selected_uses[1].published_requirement_envelope
    );
    assert_eq!(
        selected_uses[0].selected_actual_envelope,
        selected_uses[1].selected_actual_envelope
    );
}

#[test]
fn nominal_machine_uses_keep_distinct_authored_call_sites() {
    let source = r#"
        trait Handler {
            machine call(value: i32) -> i32;
        }

        machine chosen(value: i32) -> i32
        satisfies Handler::call
        {
            value
        }

        machine register<machine Selected>(value: i32) -> i32
        where machine Selected satisfies Handler::call;
        {
            Selected(value)
        }

        machine caller(value: i32) -> i32 {
            let first: i32 = register<chosen>(value);
            register<chosen>(first)
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("both nominal uses should specialize");
    let uses = &checked.facts.nominal_machine_uses.uses;

    assert_eq!(uses.len(), 2);
    assert_ne!(uses[0].site, uses[1].site);
    assert_eq!(
        uses[0].registration_operation,
        uses[1].registration_operation
    );
}

#[test]
fn structural_machine_selection_publishes_no_nominal_use_row() {
    let source = r#"
        machine chosen(value: i32) -> i32 {
            value
        }

        machine register<machine Selected>(value: i32) -> i32
        where machine Selected(value: i32) -> i32;
        {
            Selected(value)
        }

        machine caller(value: i32) -> i32 {
            register<chosen>(value)
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let checked = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("the structural use should specialize");

    assert!(checked.facts.nominal_machine_uses.uses.is_empty());
}

#[test]
fn nominal_machine_parameter_rejects_structural_coincidence() {
    let source = r#"
        trait Handler {
            machine call(value: i32) -> i32;
        }

        machine coincidental(value: i32) -> i32 {
            value
        }

        machine register<machine Selected>(value: i32) -> i32
        where machine Selected satisfies Handler::call;
        {
            Selected(value)
        }

        machine caller(value: i32) -> i32 {
            register<coincidental>(value)
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("structural coincidence must reject");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("authored satisfaction row(s)")
            && rendered.contains("structural coincidence establishes none"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn nominal_machine_parameter_rejects_a_different_authored_requirement() {
    let source = r#"
        trait Handler { machine call(value: i32) -> i32; }
        trait Other { machine call(value: i32) -> i32; }

        machine wrong(value: i32) -> i32
        satisfies Other::call
        {
            value
        }

        machine register<machine Selected>(value: i32) -> i32
        where machine Selected satisfies Handler::call;
        {
            Selected(value)
        }

        machine caller(value: i32) -> i32 {
            register<wrong>(value)
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed, &CheckingRequest::settled())
        .expect_err("wrong satisfaction row must reject");
    let rendered = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        rendered.contains("exact requirement `Handler::call`")
            && rendered.contains("exactly one is required"),
        "unexpected diagnostics:\n{rendered}"
    );
}

#[test]
fn call_site_machine_argument_resolves_to_static_entry_symbol() {
    let source = r#"
        data Card {}

        machine Card::power(value: &Card) {
        }

        machine map<T, machine F>(value: &T)
        where machine F(value: &T)
        {
        }

        machine caller(card: &Card) {
            map<Card::power>(card);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let call = typed
        .machines()
        .iter()
        .flat_map(|machine| typed.machine_states(machine))
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Call(call)
                if !call.machine_arguments.is_empty() =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("call carrying a static machine argument");

    assert_eq!(call.machine_arguments.len(), 1);
    assert!(call.machine_arguments[0].symbol.is_valid());
    assert_eq!(
        call.machine_arguments[0]
            .path
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        vec!["Card", "power"]
    );
}

#[test]
fn generic_body_call_resolves_to_machine_parameter_contract() {
    let source = r#"
        data Card {}

        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        {
            F(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("generic machine");
    let machine_parameter = typed
        .machine_type_parameters(machine)
        .iter()
        .find(|parameter| parameter.name.as_str() == "F")
        .expect("machine parameter");
    let typed_trees::data::TypeParameterKind::Machine { contract } = &machine_parameter.kind else {
        panic!("F should be a machine parameter");
    };
    let contract = typed
        .machine_parameter_contract_view(contract)
        .expect("structural F contract")
        .signature();
    assert!(
        typed
            .state_signature_parameters(contract)
            .iter()
            .all(|parameter| parameter.symbol.is_valid())
    );

    let call = typed
        .machine_states(machine)
        .iter()
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            typed_trees::statement::StatementNode::Call(call) if call.target.as_str() == "F" => {
                Some(call)
            }
            _ => None,
        })
        .expect("generic body call");
    assert_eq!(call.target_symbol, machine_parameter.symbol);
}

#[test]
fn generic_body_call_is_accepted_modularly_by_checked_lowering() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        {
            F(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved =
        resolve(ResolutionRequest::new(&syntax)).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed, &CheckingRequest::settled())
        .expect("generic body should check from F's authored contract");
}
