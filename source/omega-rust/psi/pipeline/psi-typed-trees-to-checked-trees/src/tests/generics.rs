use super::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use psi_checked_trees::{ContractProofFactKind, ContractProofFactOwner};
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;

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
    let resolved = lower_syntax_trees(&syntax).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed).expect("subjectless evidence rows should validate");
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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");

    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let typed_machine = typed
        .machines()
        .iter()
        .find(|machine| !typed.machine_type_parameters(machine).is_empty())
        .expect("typed generic machine");
    let typed_parameters = typed.machine_type_parameters(typed_machine);
    assert_eq!(typed_parameters.len(), 2);
    let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &typed_parameters[1].kind
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let psi_typed_trees::data::TypeParameterKind::Machine {
        contract: psi_typed_trees::data::MachineParameterContract::Structural(schema_signature),
    } = &schema.kind
    else {
        panic!("Schema should retain a structural signature")
    };
    let nested = typed
        .state_signature_type_parameters(schema_signature)
        .first()
        .expect("Nested parameter");
    let psi_typed_trees::data::TypeParameterKind::Machine {
        contract: psi_typed_trees::data::MachineParameterContract::Structural(nested_signature),
    } = &nested.kind
    else {
        panic!("Nested should retain a structural signature")
    };
    let nested_owner = nested.symbol;
    let nested_state = nested_signature.symbol;

    let checked = lower_typed_trees(typed).expect("nested structural evidence should check");

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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    let psi_typed_trees::data::TypeParameterKind::Machine {
        contract: psi_typed_trees::data::MachineParameterContract::Structural(schema_signature),
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
        psi_typed_trees::data::TypeParameterKind::Machine {
            contract: psi_typed_trees::data::MachineParameterContract::Nominal { .. }
        }
    ));
    let nested_owner = nested.symbol;

    let checked = lower_typed_trees(typed).expect("nested nominal reference should check");

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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
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
    let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &selected.kind else {
        panic!("Selected should be a machine parameter")
    };
    assert!(matches!(
        typed.machine_parameter_contract_view(contract),
        Some(psi_typed_trees::data::MachineParameterContractView::Nominal {
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
            Some(psi_typed_trees::data::MachineParameterContractView::Nominal {
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

    let checked = lower_typed_trees(typed)
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
        .target_contract_fingerprint();
    let actual_fingerprint = checked
        .facts
        .contract_plans
        .for_machine(chosen_symbol)
        .expect("selected machine contract plan")
        .fingerprint;
    assert_eq!(
        nominal_use
            .published_requirement_envelope
            .contract_fingerprint,
        published_fingerprint
    );
    assert_eq!(
        nominal_use.selected_actual_envelope.contract_fingerprint,
        actual_fingerprint
    );
    assert_eq!(
        nominal_use.refinement.published_requirement_fingerprint,
        published_fingerprint
    );
    assert_eq!(
        nominal_use.refinement.selected_actual_fingerprint,
        actual_fingerprint
    );
    assert_eq!(nominal_use.callback_placement, None);
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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

    let checked = lower_typed_trees(typed).expect("bounded reach closure should check");
    let reach = checked
        .facts
        .service_reaches
        .for_machine(invoke_symbol)
        .expect("invoke reach facts");

    assert_eq!(
        reach.unresolved_installation_reaches,
        [psi_effects::InstallationReachRequirement {
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

        boundary machine complete() -> u64
        reaches <= MachineControl + PortIo;
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    assert!(
        resolved
            .machines
            .iter()
            .find(|machine| machine.name.as_str() == "complete")
            .expect("resolved complete requirement")
            .service_reach_is_installation_bound
    );
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let complete = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "complete")
        .expect("complete requirement");
    let complete_symbol = complete.symbol;
    let upper_bound = complete.service_reach_row;
    assert!(complete.service_reach_is_installation_bound);

    let checked = lower_typed_trees(typed).expect("bounded reach closure should check");
    let complete_reach = checked
        .facts
        .service_reaches
        .for_machine(complete_symbol)
        .expect("complete reach facts");
    let expected = [psi_effects::InstallationReachRequirement {
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed)
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
        boundary trait Handler {
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
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
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
    typed.record_boundary_calling_plan(psi_typed_trees::typed_trees::BoundaryCallingPlanIdentity {
        boundary_trait: handler_symbol,
        boundary_arguments: Vec::new(),
        requirement_machine: requirement_symbol,
        fingerprint: expected_fingerprint,
    });

    let checked = lower_typed_trees(typed).expect("nominal callback selection should check");
    let [nominal_use] = checked.facts.nominal_machine_uses.uses.as_slice() else {
        panic!("one nominal callback use")
    };

    assert_eq!(
        nominal_use.callback_placement,
        Some(psi_checked_trees::CheckedCallbackPlacementIdentity {
            boundary_calling_plan_fingerprint: expected_fingerprint,
        })
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
        published.target_contract_fingerprint(),
        nominal_use
            .published_requirement_envelope
            .contract_fingerprint
    );
    assert!(published.published_service_reach().is_empty());
    assert!(published.published_synchronous_invocations().is_empty());
    assert!(!published.published_may_suspend());
    assert!(!published.published_may_block());
    assert_eq!(
        published.published_termination(),
        &psi_language_semantics::TerminationGuarantee::NoGuarantee
    );

    let actual = checked
        .facts
        .contract_plans
        .realized_envelope(nominal_use.selected_machine)
        .expect("selected callback actual envelope");
    assert_eq!(
        actual.contract_fingerprint,
        nominal_use.selected_actual_envelope.contract_fingerprint
    );
    assert!(actual.effective_service_reach.is_empty());
    assert!(actual.effective_synchronous_invocations.is_empty());
    assert!(!actual.checked_may_suspend);
    assert!(!actual.checked_may_block);
    assert!(actual.capabilities.is_empty());
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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let chosen_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "chosen")
        .expect("chosen machine")
        .symbol;

    let checked = lower_typed_trees(typed)
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
            .contract_fingerprint
            == nominal_use.refinement.published_requirement_fingerprint
            && nominal_use.selected_actual_envelope.contract_fingerprint
                == nominal_use.refinement.selected_actual_fingerprint
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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let checked = lower_typed_trees(typed).expect("both nominal uses should specialize");
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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let checked = lower_typed_trees(typed).expect("the structural use should specialize");

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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("structural coincidence must reject");
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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed).expect_err("wrong satisfaction row must reject");
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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    let call = typed
        .machines()
        .iter()
        .flat_map(|machine| typed.machine_states(machine))
        .flat_map(|state| typed.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            psi_typed_trees::statement::StatementNode::Call(call)
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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
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
    let psi_typed_trees::data::TypeParameterKind::Machine { contract } = &machine_parameter.kind
    else {
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
            psi_typed_trees::statement::StatementNode::Call(call)
                if call.target.as_str() == "F" =>
            {
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
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed).expect("generic body should check from F's authored contract");
}

#[test]
fn higher_order_machine_schema_specializes_nested_selection_to_fixed_point() {
    let source = r#"
        data Index {
            case Zero;
            case Next(previous: Index);
        }

        data Stream<machine S>
        where machine S(index: Index) -> Index;
        {
            case Empty;
            case More(sample: Index, tail: Stream<S>);
        }

        boundary machine sample(index: Index) -> Index;

        machine identity_schema<machine Chosen>(value: Stream<Chosen>) -> Stream<Chosen>
        where machine Chosen(index: Index) -> Index;
        {
            value
        }

        machine forward_schema<machine Schema, machine Selected>(value: Stream<Selected>) -> Stream<Selected>
        where machine Schema<machine Inner>(value: Stream<Inner>) -> Stream<Inner>
        where machine Inner(index: Index) -> Index;
        where machine Selected(index: Index) -> Index;
        {
            Schema<Selected>(value)
        }

        machine accepts_concrete(value: Stream<sample>) -> Stream<sample> {
            forward_schema<identity_schema, sample>(value)
        }

        data Main {}
        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("higher-order schema should specialize");

    for name in ["forward_schema", "identity_schema"] {
        assert!(
            checked
                .machine_specializations
                .iter()
                .any(|specialization| {
                    checked.machines().iter().any(|machine| {
                        machine.symbol == specialization.template && machine.name.as_str() == name
                    })
                }),
            "{name} should have a concrete specialization"
        );
    }
    assert!(
        checked
            .expression_table
            .iter_expressions()
            .filter_map(|(_, expression)| match expression {
                psi_typed_trees::expression::ExpressionNode::Call(call) => Some(call),
                _ => None,
            })
            .all(|call| call.machine_arguments.is_empty() && call.target.as_str() != "Schema")
    );
}

#[test]
fn generic_body_must_discharge_machine_parameter_preconditions() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>(value: i32)
        where machine F(item: i32)
            requires item > 0
        {
            F(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("an unconstrained generic body must not assume F's precondition");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("item > 0")
            || diagnostic.message.contains("contract")
            || diagnostic.message.contains("proof")
    }));
}

#[test]
fn generic_body_can_discharge_machine_parameter_precondition_from_own_contract() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>(value: i32)
        where machine F(item: i32)
            requires item > 0;
        requires value > 0
        {
            F(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed)
        .expect("the generic body's own requires fact should discharge F's precondition");
}

#[test]
fn generic_body_can_discharge_machine_parameter_precondition_from_call_value() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>()
        where machine F(item: i32)
            requires item > 0
        {
            F(1);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed).expect("the call argument should discharge F's precondition");
}

#[test]
fn generic_body_inherits_machine_parameter_service_ceiling() {
    let source = r#"
        boundary trait DeviceIo {
            machine touch();
        }

        data Main {}
        machine Main::run(&mut self) {}

        machine apply<machine F>()
        where machine F()
            reaches DeviceIo
        {
            F();
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let apply = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("apply machine");
    let operations = psi_effects::infer_operational_may(&typed);
    let service_reaches = psi_effects::infer_service_reaches(&typed, &operations);
    let apply_reach = service_reaches
        .for_machine(apply.symbol)
        .expect("apply service-reach summary");
    let device_io = typed
        .service_reaches
        .id_for_name("DeviceIo")
        .expect("DeviceIo service identity");
    assert!(
        service_reaches
            .services(apply_reach.effective)
            .contains(&device_io)
    );
}

#[test]
fn static_machine_selection_respects_guarded_crash_ceiling() {
    let source = r#"
            machine selected(flag: bool)
            crashes Abort
                flag
            {}

            machine apply<machine Selected>(flag: bool)
            where machine Selected(value: bool)
                crashes Abort
                    value;
            {
                Selected(flag);
            }

            machine caller(flag: bool) {
                apply<selected>(flag);
            }
            "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");

    lower_typed_trees(typed).expect("an identical crash route should refine the machine slot");
}

#[test]
fn generic_body_can_consume_machine_parameter_ensures() {
    let source = r#"
        data Main {}
        machine Main::run(&mut self) {}
        domain i32::Positive
        requires
            self > 0

        machine pipeline<machine Establish, machine Consume>(value: &mut i32)
        where machine Establish(item: &mut i32)
            ensures item in i32::Positive;
        where machine Consume(item: &i32)
            requires item in i32::Positive
        {
            Establish(value);
            Consume(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed)
        .expect("Establish's authored ensures should discharge Consume's requires");
}

#[test]
fn static_machine_argument_specializes_body_calls_to_direct_symbols() {
    let source = r#"
        data Card {}
        data Main {}

        machine Card::power(value: &Card) -> u64 {
            7
        }

        machine apply<T, machine F>(value: &T) -> u64
        where machine F(item: &T) -> u64
        {
            F(value)
        }

        machine caller(card: &Card) {
            let score: u64 = apply<Card::power>(card);
        }

        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let power_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Card::power")
        .and_then(|machine| typed.machine_states(machine).first())
        .map(|state| state.symbol)
        .expect("power entry symbol");

    let checked = lower_typed_trees(typed).expect("static specialization should check");
    let apply = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("specialized apply machine");
    assert!(checked.machine_type_parameters(apply).is_empty());
    assert_eq!(checked.machine_specializations.len(), 1);
    assert_eq!(checked.machine_specializations[0].instance, apply.symbol);
    assert_eq!(
        checked.machine_specializations[0].machine_arguments,
        vec![power_symbol]
    );
    assert_eq!(
        checked.machine_specializations[0].type_arguments,
        vec!["Card"]
    );
    assert_ne!(checked.machine_specializations[0].fingerprint, 0);

    let direct_call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_typed_trees::expression::ExpressionNode::Call(call)
                if call.target_symbol == power_symbol =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("F(value) should become a direct Card::power call");
    assert_eq!(direct_call.target.as_str(), "power");
    assert!(direct_call.machine_arguments.is_empty());

    assert!(
        checked
            .expression_table
            .iter_expressions()
            .filter_map(|(_, expression)| match expression {
                psi_typed_trees::expression::ExpressionNode::Call(call) => Some(call),
                _ => None,
            })
            .all(|call| call.machine_arguments.is_empty())
    );
}

#[test]
fn free_static_machine_specialization_preserves_authored_target_name() {
    let source = r#"
        data Main {}

        machine chosen(value: u16) -> u16 {
            value
        }

        machine apply<machine F>(value: u16) -> u16
        where machine F(item: u16) -> u16
        {
            F(value)
        }

        machine caller() -> u16 {
            apply<chosen>(70)
        }

        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let chosen_symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "chosen")
        .and_then(|machine| typed.machine_states(machine).first())
        .map(|state| state.symbol)
        .expect("chosen entry symbol");

    let checked = lower_typed_trees(typed).expect("free static selection should specialize");
    let direct_call = checked
        .expression_table
        .iter_expressions()
        .find_map(|(_, expression)| match expression {
            psi_typed_trees::expression::ExpressionNode::Call(call)
                if call.target_symbol == chosen_symbol =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("F(value) should become a direct chosen call");
    assert_eq!(direct_call.target.as_str(), "chosen");
    assert_ne!(direct_call.target.as_str(), "entry");
    assert!(direct_call.machine_arguments.is_empty());
}

#[test]
fn static_machine_specialization_identity_is_reproducible() {
    fn fingerprint(source: &str) -> u64 {
        let tokens = Lexer::new(source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        lower_typed_trees(typed)
            .expect("specialization should check")
            .machine_specializations[0]
            .fingerprint
    }

    let source = r#"
        data Card {}
        data Main {}
        machine Card::power(value: &Card) {}
        machine apply<T, machine F>(value: &T)
        where machine F(item: &T)
        { F(value); }
        machine caller(card: &Card) {
            apply<Card::power>(card);
        }
        machine Main::run(&mut self) {}
    "#;
    assert_eq!(fingerprint(source), fingerprint(source));
}

#[test]
fn value_machine_type_parameter_is_inferred_through_a_borrowed_place() {
    let source = r#"
        data Light [copy] { weight: i32 in Wrapping; }
        data Main { light: Light; }

        machine Main::weigh<T [copy]>(&self, value: &T) -> i32 {
            70
        }

        machine Main::run(&mut self) {
            let result: i32 in Wrapping = self.weigh(&self.light);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("the borrowed place should select and materialize T := Light");

    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "Main::weigh"
            })
        })
        .expect("weigh specialization");
    assert_eq!(specialization.type_arguments, ["Light"]);
    assert_eq!(
        specialization.type_argument_identities,
        ["named(name(Light))"]
    );
}

#[test]
fn public_visibility_survives_value_type_specialization() {
    let source = r#"
        data Light [copy] { weight: i32 in Wrapping; }
        data Main { light: Light; }

        pub machine Main::weigh<T [copy]>(&self, value: &T) -> i32 {
            70
        }

        machine Main::run(&mut self) {
            let result: i32 in Wrapping = self.weigh(&self.light);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("public specialization should check");
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "Main::weigh"
            })
        })
        .expect("public weigh specialization");

    for symbol in [specialization.template, specialization.instance] {
        assert!(
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == symbol)
                .expect("public machine retained through specialization")
                .is_public
        );
    }
}

#[test]
fn distinct_static_machine_specializations_clone_the_template() {
    let source = r#"
        boundary trait Clock {}
        data Card {}
        data Main {}
        machine Card::power(value: &Card) {}
        machine Card::rank(value: &Card) {}
        machine apply<T, machine F>(value: &T)
        where machine F(item: &T);
        reaches Clock
        suspends;
        blocks;
        { F(value); }
        machine caller(card: &Card) {
            apply<Card::power>(card);
            apply<Card::rank>(card);
        }
        machine Main::run(&mut self) {}
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let typed_apply = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "apply")
        .expect("typed apply template");
    assert_eq!(
        typed
            .authored_service_reach_rows_for(typed_apply.symbol)
            .count(),
        1,
        "typed template retains authored reach before specialization"
    );
    assert_eq!(typed_apply.suspends_keyword_source_spans.len(), 1);
    assert_eq!(typed_apply.blocks_keyword_source_spans.len(), 1);
    let checked = lower_typed_trees(typed)
        .expect("each concrete machine tuple should receive its own specialization");
    let apply_specializations: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "apply"
            })
        })
        .collect();
    assert_eq!(apply_specializations.len(), 2);
    assert_ne!(
        apply_specializations[0].machine_arguments,
        apply_specializations[1].machine_arguments
    );
    assert_eq!(
        checked
            .machines()
            .iter()
            .filter(|machine| {
                machine.name.as_str() == "apply"
                    || machine.name.as_str().starts_with("apply$specialized$")
            })
            .count(),
        2
    );
    let clock = checked
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Clock")
        .expect("Clock boundary trait");
    for specialization in apply_specializations {
        let instance = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .expect("specialized machine");
        assert_eq!(instance.suspends_keyword_source_spans.len(), 1);
        assert_eq!(instance.blocks_keyword_source_spans.len(), 1);
        let rows = checked
            .authored_service_reach_rows_for(specialization.instance)
            .collect::<Vec<_>>();
        let [row] = rows.as_slice() else {
            panic!(
                "specialization {:?} from {:?} retains {} authored reach rows",
                specialization.instance,
                specialization.template,
                rows.len()
            )
        };
        let [target] = row.targets.as_slice() else {
            panic!("each concrete specialization retains the authored Clock occurrence")
        };
        assert_eq!(target.service, clock.symbol);
        assert!(target.source_span.span.end > target.source_span.span.start);
    }
}

#[test]
fn attached_machine_specialization_clones_inherited_field_symbols() {
    let source = r#"
        data Console {}
        data Light [copy] { weight: i32; }
        data Main { console: Console; light: Light; number: i32; }

        machine Main::pick<T [copy]>(&self, value: &T) -> i32 { 7 }
        machine Main::run(&mut self) {
            let from_light: i32 = self.pick(&self.light);
            let from_number: i32 = self.pick(&self.number);
        }
    "#;
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("each attached specialization should retain its inherited field coordinates");

    let pick = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::pick")
        .expect("pick template instance");
    let instances = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == pick.symbol)
        .map(|specialization| specialization.instance)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    let main_symbol = checked
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Main")
        .expect("Main data")
        .symbol;
    for instance in instances {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == instance)
            .expect("specialized pick machine");
        assert_eq!(
            machine.attached_data_symbol, main_symbol,
            "both the reused template and cloned specialization retain exact attached identity"
        );
        let fields = checked
            .symbols
            .child_handles(machine.symbol)
            .into_iter()
            .flatten()
            .filter(|symbol| checked.symbols.get(*symbol).kind == psi_symbols::SymbolKind::Field)
            .map(|symbol| checked.symbols.name(symbol))
            .collect::<Vec<_>>();
        assert_eq!(fields, ["console", "light", "number"]);
    }
}

#[test]
fn forwarded_generic_calls_specialize_after_their_caller() {
    let source = r#"
        data Light [copy] { weight: i32; }
        data Main { light: Light; number: i32; }

        machine Main::copy_it<T [copy]>(&self, value: &T) {}
        machine Main::wrap<U [copy]>(&self, value: &U) {
            self.copy_it(value);
        }
        machine Main::run(&mut self) {
            self.copy_it(&self.light);
            self.copy_it(&self.number);
            self.wrap(&self.light);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("specializing the generic caller should expose its forwarded concrete type");

    let specialization_count = |name: &str| {
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template && machine.name.as_str() == name
                })
            })
            .count()
    };
    assert_eq!(specialization_count("Main::copy_it"), 2);
    assert_eq!(specialization_count("Main::wrap"), 1);
}

#[test]
fn concrete_specialization_must_satisfy_nominal_conformance_bound() {
    let source = r#"
        trait Marker {}
        data Good {}
        data Bad {}
        GoodMarker: Good satisfies Marker;

        machine accept<T>(value: &T)
        where T satisfies Marker
        {}

        machine caller(good: &Good, bad: &Bad) {
            accept(good);
            accept(bad);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("the Bad specialization has no authored nominal conformance");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("binds `T` to `Bad`, which has no nominal conformance to `Marker`")
    }));
}

#[test]
fn bounded_generic_call_specializes_to_concrete_attached_state() {
    let source = r#"
        trait Incrementable {
            machine increment(&mut self);
        }
        data Counter { value: i32 in Wrapping; }
        machine Counter::increment(&mut self) satisfies Incrementable::increment {
            self.value = self.value + 1;
        }
        CounterIncrementable: Counter satisfies Incrementable;

        machine step<T>(subject: &mut T)
        where T satisfies Incrementable
        {
            subject.increment();
        }
        data Main { counter: Counter; }
        machine Main::run(&mut self) {
            step(&mut self.counter);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("the nominal bound should specialize");

    let step = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "step")
        .expect("specialized step machine");
    assert!(checked.machine_type_parameters(step).is_empty());
    assert!(step.conformance_bounds.is_empty());
    let state = checked
        .machine_states(step)
        .first()
        .expect("step entry state");
    let call = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| {
            let psi_typed_trees::statement::StatementNode::Call(call) = statement else {
                return None;
            };
            (call.target.as_str() == "increment").then_some(call)
        })
        .expect("bounded requirement call");
    assert_eq!(
        checked
            .statement_table
            .name_path_members(call.receiver)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>(),
        ["subject"]
    );

    let concrete_target = crate::lookup::resolve_state_call_target(
        &checked,
        step,
        state,
        call.receiver_symbol,
        call.target_symbol,
        crate::lookup::statement_call_receiver_members(&checked, call),
        &call.target,
    );
    let increment = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Counter::increment")
        .and_then(|machine| checked.machine_states(machine).first())
        .expect("Counter increment state");
    assert_eq!(call.target_symbol, increment.symbol);
    assert_eq!(concrete_target, increment.symbol);
}

#[test]
fn named_conformance_bound_rejects_a_different_concrete_carrier() {
    let source = r#"
        trait Marker {}
        data Good {}
        data Bad {}
        Primary: Good satisfies Marker;

        machine accept<T>(value: &T)
        where T satisfies Good::Primary
        {}

        machine caller(bad: &Bad) {
            accept(bad);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics =
        lower_typed_trees(typed).expect_err("the selected conformance belongs only to Good");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("binds `T` to `Bad`, but named conformance `Good::Primary` belongs to `Good`")
    }));
}

#[test]
fn named_conformance_bound_rejects_a_name_owned_by_another_carrier() {
    let source = r#"
        trait Marker {}
        data Good {}
        data Bad {}
        Primary: Good satisfies Marker;

        machine accept<T>(value: &T)
        where T satisfies Bad::Primary
        {}

        machine caller(bad: &Bad) {
            accept(bad);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("the package-scoped conformance name still retains its carrier");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("names conformance `Bad::Primary`, but that declaration belongs to `Good`")
    }));
}

#[test]
fn explicit_conformance_binder_selects_and_substitutes_one_closed_map() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card { rank: i32; }

        PowerOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                self.rank < other.rank
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, PowerOrder>(left, right)
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let selected = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "PowerOrder")
        })
        .expect("selected conformance")
        .symbol;
    let checked = lower_typed_trees(typed)
        .expect("an explicit binder should specialize through its selected closed map");

    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.conformance_arguments == [selected])
        .expect("specialization retains the exact conformance argument");
    assert!(specialization.machine_arguments.is_empty());
    assert_eq!(specialization.conformance_argument_fingerprints.len(), 1);
    assert_ne!(specialization.conformance_argument_fingerprints[0], 0);
    let selected_row = checked
        .conformances()
        .iter()
        .find(|conformance| conformance.symbol == selected)
        .and_then(|conformance| checked.closed_conformance_rows(conformance))
        .and_then(|rows| rows.first())
        .expect("selected closed row");
    assert!(checked.machines().iter().any(|machine| {
        checked.machine_states(machine).iter().any(|state| {
            checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        psi_typed_trees::statement::StatementNode::Expression(expression)
                            if matches!(
                                checked.expression_table.expression(*expression),
                                psi_typed_trees::expression::ExpressionNode::Call(call)
                                    if call.target_symbol == selected_row.realization_state
                            )
                    )
                })
        })
    }));
}

#[test]
fn explicit_conformance_binders_keep_distinct_closed_maps_as_distinct_instances() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card { rank: i32; }

        Ascending: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                self.rank < other.rank
            }
        }

        Descending: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                self.rank > other.rank
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine ascending(left: &Card, right: &Card) -> bool {
            choose<Card, Ascending>(left, right)
        }

        machine descending(left: &Card, right: &Card) -> bool {
            choose<Card, Descending>(left, right)
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed)
        .expect("each exact conformance argument should produce one specialization");

    let instances = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.conformance_arguments.len() == 1)
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);
    assert_ne!(instances[0].instance, instances[1].instance);
    assert_ne!(instances[0].fingerprint, instances[1].fingerprint);
    assert_ne!(
        instances[0].conformance_argument_fingerprints,
        instances[1].conformance_argument_fingerprints
    );
}

#[test]
fn nested_generic_conformance_application_closes_its_own_telescope() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine send<Element, Output, Encoding: Element satisfies Encodes<Output>>(
            bytes: &Element,
            message: &Output
        ) {}

        machine caller(bytes: &Bytes, message: &Message) {
            send<Bytes, Message, SequenceEncoding<Bytes, Message>>(bytes, message);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let selected = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "SequenceEncoding")
        })
        .expect("generic conformance")
        .symbol;
    let checked = lower_typed_trees(typed).expect("closed generic conformance application");
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| specialization.conformance_arguments == [selected])
        .expect("selected application specialization");
    let [application] = specialization.conformance_applications.as_slice() else {
        panic!("one closed application")
    };
    assert_eq!(application.declaration, selected);
    assert_eq!(application.type_arguments, ["Bytes", "Message"]);
    assert_eq!(application.subject_identity.as_deref(), Some("Bytes"));
    assert_eq!(application.trait_arguments.len(), 1);
    assert_ne!(application.fingerprint, 0);
    assert_eq!(
        specialization.conformance_argument_fingerprints,
        [application.fingerprint]
    );
}

#[test]
fn selected_generic_conformance_bound_closes_and_specializes_its_application() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine accept<Element>(value: &Element)
        where Element satisfies Bytes::SequenceEncoding<Bytes, Message>
        {}

        machine caller(value: &Bytes) {
            accept(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let selected = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "SequenceEncoding")
        })
        .expect("selected generic conformance")
        .symbol;
    let checked = lower_typed_trees(typed).expect("selected bound application closes");
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str() == "accept")
        })
        .expect("accept specialization");
    assert!(specialization.conformance_arguments.is_empty());
    let [application] = specialization.conformance_applications.as_slice() else {
        panic!("one selected bound application");
    };
    assert_eq!(application.declaration, selected);
    assert_eq!(application.type_arguments, ["Bytes", "Message"]);
    assert_eq!(application.subject_identity.as_deref(), Some("Bytes"));
    assert_eq!(application.trait_arguments, ["Message"]);
}

#[test]
fn selected_bound_application_substitutes_forwarded_type_const_and_machine_arguments() {
    let source = r#"
        trait Encodes<Output> {}
        data Card {}
        data Message {}
        machine rank(value: &Card) -> u64 { 0 }

        FullEncoding<Element, Output, const Rank: u64, machine TieBreak>:
            Element satisfies Encodes<Output>
        where machine TieBreak(value: &Element) -> u64;
        {}

        machine inspect<Element, const Rank: u64, machine TieBreak>(value: &Element)
        where machine TieBreak(value: &Element) -> u64;
        where Element satisfies Card::FullEncoding<Element, Message, Rank, TieBreak>
        {}

        machine caller(value: &Card) {
            inspect<Card, 7, rank>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let rank = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "rank")
        .and_then(|machine| typed.machine_states(machine).first())
        .expect("rank state")
        .symbol;
    let checked = lower_typed_trees(typed).expect("forwarded selected application closes");
    let specialization = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str() == "inspect")
        })
        .expect("inspect specialization");
    let [application] = specialization.conformance_applications.as_slice() else {
        panic!("one selected bound application");
    };
    assert_eq!(application.type_arguments, ["Card", "Message"]);
    assert_eq!(application.const_arguments, ["7"]);
    assert_eq!(application.machine_arguments, [rank]);
    assert_eq!(application.subject_identity.as_deref(), Some("Card"));
}

#[test]
fn unused_private_selected_conformance_bound_rejects_missing_application_arguments() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine private_accept<Element>(value: &Element)
        where Element satisfies Bytes::SequenceEncoding
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("private bound must close");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "generic conformance `SequenceEncoding` requires 2 explicit non-lifetime argument(s), got 0",
        )
    }));
}

#[test]
fn unused_private_trait_selected_conformance_bound_is_also_closed() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        trait Private<Element>
        where Element satisfies Bytes::SequenceEncoding
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("private trait bound must close");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "generic conformance `SequenceEncoding` requires 2 explicit non-lifetime argument(s), got 0",
        )
    }));
}

#[test]
fn unused_private_selected_conformance_bound_rejects_wrong_argument_category() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine private_accept<Element>(value: &Element)
        where Element satisfies Bytes::SequenceEncoding<7, Message>
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("wrong type category must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("parameter `Element` requires a type argument")
    }));
}

#[test]
fn private_selected_conformance_bound_closes_lifetime_const_and_machine_lanes() {
    let source = r#"
        trait Encodes<Output> {}
        data Card {}
        data Message {}
        machine rank(value: &Card) -> u64 { 0 }

        FullEncoding<'scope, Element, Output, const Rank: u64, machine TieBreak>:
            Element satisfies Encodes<Output>
        where machine TieBreak(value: &Element) -> u64;
        {}

        machine private_inspect<'view, Element>(value: &'view Element)
        where Element satisfies Card::FullEncoding<'view, Card, Message, 7, rank>
        {}
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    lower_typed_trees(typed).expect("all selected private bound lanes close");
}

#[test]
fn explicit_generic_conformance_lifetime_closes_its_trait_identity() {
    let source = r#"
        trait Borrows<Source> {}
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        Scoped<'scope, Element>:
            Element satisfies Borrows<Borrow<'scope, Element>>
        {}

        machine choose<'call, Element, Evidence: Element satisfies Borrows<Borrow<'call, Element>>>(
            value: &'call Element
        ) {}

        machine caller<'view>(value: &'view Card) {
            choose<Card, Scoped<'view, Card>>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("explicit conformance lifetime");
    let application = checked
        .machine_specializations
        .iter()
        .find_map(|specialization| specialization.conformance_applications.first())
        .expect("closed conformance application");
    assert_eq!(application.lifetime_arguments, ["view"]);
    assert_eq!(application.trait_arguments, ["Borrow<'view,Card>"]);
}

#[test]
fn generic_conformance_lifetime_elides_from_one_ordinary_borrow_constraint() {
    let source = r#"
        trait Borrows<Source> {}
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        Scoped<'scope, Element>:
            Element satisfies Borrows<Borrow<'scope, Element>>
        {}

        machine choose<'call, Element, Evidence: Element satisfies Borrows<Borrow<'call, Element>>>(
            value: &'call Element
        ) {}

        machine caller<'view>(value: &'view Card) {
            choose<Card, Scoped<Card>>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("uniquely elided conformance lifetime");
    let application = checked
        .machine_specializations
        .iter()
        .find_map(|specialization| specialization.conformance_applications.first())
        .expect("closed conformance application");
    assert_eq!(application.lifetime_arguments, ["view"]);
    assert_eq!(application.trait_arguments, ["Borrow<'view,Card>"]);
}

#[test]
fn explicit_generic_conformance_lifetime_must_match_the_ordinary_borrow_constraint() {
    let source = r#"
        trait Borrows<Source> {}
        data Card {}
        data Borrow<'scope, Element> { value: &'scope Element }

        Scoped<'scope, Element>:
            Element satisfies Borrows<Borrow<'scope, Element>>
        {}

        machine choose<'call, Element, Evidence: Element satisfies Borrows<Borrow<'call, Element>>>(
            value: &'call Element
        ) {}

        machine caller<'view, 'other>(value: &'view Card) {
            choose<Card, Scoped<'other, Card>>(value);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("mismatched explicit lifetime");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("disagree with the call's ordinary borrow constraints")
    }));
}

#[test]
fn generic_conformance_lifetime_elision_rejects_conflicting_borrow_constraints() {
    let source = r#"
        trait Relates<Context> {}
        data Card {}
        data Pair<'left, 'right, Element> {
            left: &'left Element;
            right: &'right Element;
        }

        SameScope<'scope, Element>:
            Element satisfies Relates<Pair<'scope, 'scope, Element>>
        {}

        machine choose<
            'left,
            'right,
            Element,
            Evidence: Element satisfies Relates<Pair<'left, 'right, Element>>
        >(
            left: &'left Element,
            right: &'right Element
        ) {}

        machine caller<'a, 'b>(left: &'a Card, right: &'b Card) {
            choose<Card, SameScope<Card>>(left, right);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("ambiguous elision must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no unique ordinary borrow constraint is available")
    }));
}

#[test]
fn bare_generic_conformance_name_does_not_infer_its_owned_arguments() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Message {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine send<Element, Output, Encoding: Element satisfies Encodes<Output>>(
            bytes: &Element,
            message: &Output
        ) {}

        machine caller(bytes: &Bytes, message: &Message) {
            send<Bytes, Message, SequenceEncoding>(bytes, message);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = lower_typed_trees(typed).expect_err("bare generic name must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "generic conformance `SequenceEncoding` requires 2 explicit non-lifetime argument(s), got 0",
        )
    }));
}

#[test]
fn members_of_one_generic_conformance_family_have_distinct_identity() {
    let source = r#"
        trait Encodes<Output> {}
        data Bytes {}
        data Text {}
        data Message {}
        data Notice {}

        SequenceEncoding<Element, Output>:
            Element satisfies Encodes<Output>
        {}

        machine send<Element, Output, Encoding: Element satisfies Encodes<Output>>(
            value: &Element,
            output: &Output
        ) {}

        machine first(value: &Bytes, output: &Message) {
            send<Bytes, Message, SequenceEncoding<Bytes, Message>>(value, output);
        }
        machine second(value: &Text, output: &Notice) {
            send<Text, Notice, SequenceEncoding<Text, Notice>>(value, output);
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("two closed family applications");
    let applications = checked
        .machine_specializations
        .iter()
        .filter_map(|specialization| specialization.conformance_applications.first())
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 2);
    assert_eq!(applications[0].declaration, applications[1].declaration);
    assert_ne!(applications[0].fingerprint, applications[1].fingerprint);
    assert_ne!(
        applications[0].type_arguments,
        applications[1].type_arguments
    );
}

#[test]
fn nested_generic_conformance_application_specializes_its_selected_row() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        FieldOrder<Element>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, FieldOrder<Card>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("instantiated selected row");
    let application = checked
        .machine_specializations
        .iter()
        .find_map(|specialization| specialization.conformance_applications.first())
        .expect("closed conformance application");
    assert_eq!(application.subject_identity.as_deref(), Some("Card"));
    assert_eq!(application.rows.len(), 1);
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| {
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == specialization.template)
                    .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
                    && specialization.type_arguments == ["Card"]
            })
    );
}

#[test]
fn distinct_generic_conformance_applications_specialize_distinct_selected_rows() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}
        data Token {}
        data Root {}

        FieldOrder<Element>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine cards(left: &Card, right: &Card) -> bool {
            choose<Card, FieldOrder<Card>>(left, right)
        }

        machine tokens(left: &Token, right: &Token) -> bool {
            choose<Token, FieldOrder<Token>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("two instantiated selected rows");
    let mut row_instances = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
        })
        .map(|specialization| {
            (
                specialization.instance,
                specialization.type_arguments.as_slice(),
            )
        })
        .collect::<Vec<_>>();
    row_instances.sort_by_key(|(_, arguments)| arguments[0].as_str());
    assert_eq!(row_instances.len(), 2);
    assert_eq!(row_instances[0].1, ["Card"]);
    assert_eq!(row_instances[1].1, ["Token"]);
    assert_ne!(row_instances[0].0, row_instances[1].0);
}

#[test]
fn generic_conformance_const_argument_specializes_its_selected_row() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        FieldOrder<Element, const Rank: u64>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, FieldOrder<Card, 7>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("const-instantiated selected row");
    let row = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
        })
        .expect("selected row specialization");
    assert_eq!(row.type_arguments, ["Card"]);
    assert_eq!(row.const_arguments, ["7"]);
}

#[test]
fn generic_conformance_static_machine_argument_specializes_its_selected_row() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        machine rank(value: &Card) -> bool { true }

        FieldOrder<Element, machine TieBreak>: Element satisfies Ranked
        where machine TieBreak(value: &Element) -> bool;
        {
            machine before(&self, other: &Element) -> bool {
                transition { _ -> TieBreak(self) }
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, FieldOrder<Card, rank>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let rank = typed
        .machines()
        .iter()
        .find_map(|machine| {
            (machine.name.as_str() == "rank")
                .then(|| {
                    typed
                        .machine_states(machine)
                        .first()
                        .map(|state| state.symbol)
                })
                .flatten()
        })
        .expect("rank state");
    let checked = lower_typed_trees(typed).expect("machine-instantiated selected row");
    let row = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
        })
        .expect("selected row specialization");
    assert_eq!(row.type_arguments, ["Card"]);
    assert_eq!(row.machine_arguments, [rank]);
}

#[test]
fn outer_generic_specialization_substitutes_nested_conformance_application() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        FieldOrder<Element>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine forward<Element>(left: &Element, right: &Element) -> bool {
            choose<Element, FieldOrder<Element>>(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            forward<Card>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("forwarded closed application");
    let application = checked
        .machine_specializations
        .iter()
        .filter_map(|specialization| specialization.conformance_applications.first())
        .find(|application| application.subject_identity.as_deref() == Some("Card"))
        .expect("concrete forwarded application");
    assert_eq!(application.type_arguments, ["Card"]);
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| {
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == specialization.template)
                    .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
                    && specialization.type_arguments == ["Card"]
            })
    );
}

#[test]
fn outer_generic_specialization_substitutes_all_nested_conformance_lanes() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}

        machine rank(value: &Card) -> bool { true }

        FieldOrder<Element, const Rank: u64, machine TieBreak>:
            Element satisfies Ranked
        where machine TieBreak(value: &Element) -> bool;
        {
            machine before(&self, other: &Element) -> bool {
                transition { _ -> TieBreak(self) }
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine forward<Element, const Rank: u64, machine TieBreak>(
            left: &Element,
            right: &Element
        ) -> bool
        where machine TieBreak(value: &Element) -> bool;
        {
            choose<Element, FieldOrder<Element, Rank, TieBreak>>(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            forward<Card, 7, rank>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let rank = typed
        .machines()
        .iter()
        .find_map(|machine| {
            (machine.name.as_str() == "rank")
                .then(|| {
                    typed
                        .machine_states(machine)
                        .first()
                        .map(|state| state.symbol)
                })
                .flatten()
        })
        .expect("rank state");
    let checked = lower_typed_trees(typed).expect("fully forwarded closed application");
    let row = checked
        .machine_specializations
        .iter()
        .find(|specialization| {
            checked
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.template)
                .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
        })
        .expect("selected row specialization");
    assert_eq!(row.type_arguments, ["Card"]);
    assert_eq!(row.const_arguments, ["7"]);
    assert_eq!(row.machine_arguments, [rank]);
}

#[test]
fn generic_carrier_conformance_application_specializes_its_selected_row() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }
        data Card {}
        data Box<Element> {}

        FieldOrder<Element>: Element satisfies Ranked {
            machine before(&self, other: &Element) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Box<Card>, right: &Box<Card>) -> bool {
            choose<Box<Card>, FieldOrder<Box<Card>>>(left, right)
        }
    "#;

    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("generic-carrier selected row");
    let application = checked
        .machine_specializations
        .iter()
        .filter_map(|specialization| specialization.conformance_applications.first())
        .find(|application| application.subject_identity.as_deref() == Some("Box<Card>"))
        .expect("closed generic-carrier application");
    assert_eq!(application.type_arguments, ["Box<Card>"]);
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| {
                checked
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == specialization.template)
                    .is_some_and(|machine| machine.name.as_str().contains("FieldOrder::before"))
                    && specialization.type_arguments == ["Box<Card>"]
            })
    );
}

#[test]
fn explicit_conformance_binder_rejects_a_map_for_the_wrong_subject() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card {}
        data Token {}

        TokenOrder: Token satisfies Ranked {
            machine before(&self, other: &Token) -> bool { true }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, TokenOrder>(left, right)
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("an exact evidence argument must belong to the instantiated subject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "cannot bind `Order` to conformance `TokenOrder`: expected a complete `Card satisfies Ranked` map",
        )
    }));
}

#[test]
fn explicit_conformance_binder_dispatches_an_inherited_requirement_row() {
    let source = r#"
        trait Comparable {
            machine Self::before(&self, other: &Self) -> bool;
        }

        trait Ranked: Comparable {}

        data Card { rank: i32; }

        CardOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                self.rank < other.rank
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            choose<Card, CardOrder>(left, right)
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("inherited binder lookup should resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let selected_row = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "CardOrder")
        })
        .and_then(|conformance| typed.closed_conformance_rows(conformance))
        .and_then(|rows| rows.first())
        .expect("selected inherited row")
        .realization_state;
    let checked = lower_typed_trees(typed)
        .expect("the inherited requirement should dispatch through the selected map");
    assert!(checked.machines().iter().any(|machine| {
        checked.machine_states(machine).iter().any(|state| {
            checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        psi_typed_trees::statement::StatementNode::Expression(expression)
                            if matches!(
                                checked.expression_table.expression(*expression),
                                psi_typed_trees::expression::ExpressionNode::Call(call)
                                    if call.target_symbol == selected_row
                            )
                    )
                })
        })
    }));
}

#[test]
fn explicit_conformance_binder_rewrites_a_procedure_requirement_call() {
    let source = r#"
        trait Resettable {
            machine Self::reset(&mut self);
        }

        data Counter { value: i32; }

        CounterReset: Counter satisfies Resettable {
            machine reset(&mut self) {
                self.value = 0;
            }
        }

        machine reset_one<Element, Reset: Element satisfies Resettable>(value: &mut Element) {
            Reset::reset(value);
        }

        machine caller(value: &mut Counter) {
            reset_one<Counter, CounterReset>(value);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let selected_row = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "CounterReset")
        })
        .and_then(|conformance| typed.closed_conformance_rows(conformance))
        .and_then(|rows| rows.first())
        .expect("selected reset row")
        .realization_state;
    let checked = lower_typed_trees(typed)
        .expect("a resultless requirement call should dispatch through the selected map");
    assert!(checked.machines().iter().any(|machine| {
        checked.machine_states(machine).iter().any(|state| {
            checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(
                        statement,
                        psi_typed_trees::statement::StatementNode::Call(call)
                            if call.target_symbol == selected_row
                                && checked.statement_table.name_path_members(call.receiver).len() == 1
                                && checked.statement_table.expression_handles(call.arguments).is_empty()
                    )
                })
        })
    }));
}

#[test]
fn static_named_witness_requirement_call_keeps_public_lanes_and_private_dispatch_separate() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;

        trait Producer {
            machine Self::produce(&self)
            requires public_in: ready()
            ensures public_out: ready();
        }

        data Token {}

        TokenProducer: Token satisfies Producer {
            machine produce(&self)
            requires local_in: ready()
            ensures public_out: ready()
            ensures private_out: ready()
            {
                public_out = local_in;
                private_out = local_in;
            }
        }

        machine Root::invoke<Element, Order: Element satisfies Producer>(
            &self,
            value: &Element
        )
        requires incoming: ready()
        {
            let (; public_out: result) = Order::produce(value; incoming);
        }

        machine Root::caller(&self, value: &Token)
        requires incoming: ready()
        {
            self.invoke<Token, TokenProducer>(value; incoming);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked =
        lower_typed_trees(typed).expect("one exact static requirement witness call should check");

    let invocations = checked
        .facts
        .proof
        .proof_output_calls
        .iter()
        .filter_map(|(_, invocation)| {
            invocation
                .static_requirement_dispatch
                .as_ref()
                .map(|dispatch| (invocation, dispatch))
        })
        .collect::<Vec<_>>();
    let [(invocation, dispatch)] = invocations.as_slice() else {
        panic!("one exact static requirement proof-output call")
    };
    assert_eq!(
        invocation.target_machine_symbol,
        dispatch.realization_machine
    );
    assert_eq!(invocation.target_state_symbol, dispatch.realization_state);
    assert_ne!(dispatch.application_fingerprint, 0);

    let [argument] = invocation.evidence_arguments.as_slice() else {
        panic!("one public requirement input")
    };
    let [output] = invocation.outputs.as_slice() else {
        panic!("private satisfier strengthening must not widen the requirement output lane")
    };
    let input_declaration = checked
        .facts
        .proof
        .evidence_terms
        .get(argument.callee_input);
    let output_declaration = checked.facts.proof.evidence_terms.get(output.callee_output);
    let public_owner = psi_checked_trees::ContractProofFactOwner::StateSignature {
        owner_symbol: dispatch.declaring_trait,
        state_symbol: dispatch.requirement,
    };
    assert_eq!(input_declaration.owner, public_owner);
    assert_eq!(output_declaration.owner, public_owner);
    assert_eq!(input_declaration.name, "public_in");
    assert_eq!(output_declaration.name, "public_out");
    let caller_output = output.output.expect("selected public output is retained");
    assert_ne!(caller_output, argument.source);
    assert_ne!(caller_output, output.callee_output);

    assert!(
        checked
            .facts
            .proof
            .evidence_forwardings
            .iter()
            .any(|(_, forwarding)| {
                forwarding.machine_symbol == dispatch.realization_machine
                    && matches!(
                        forwarding.source,
                        psi_checked_trees::EvidenceAssignmentSource::Forwarded { .. }
                    )
            })
    );

    let contract_calls = checked
        .facts
        .proof
        .contract_calls
        .iter()
        .filter_map(|(_, call)| {
            (call.target_state_symbol == dispatch.realization_state).then_some(call)
        })
        .collect::<Vec<_>>();
    let [contract_call] = contract_calls.as_slice() else {
        panic!("one exact ordinary call link for the static requirement dispatch")
    };
    for refs in [contract_call.requires, contract_call.ensures] {
        let [fact_ref] = checked.facts.proof.contract_fact_refs.span_or_empty(refs) else {
            panic!("one pinned public contract row per lane")
        };
        assert_eq!(
            checked.facts.proof.contract_facts.get(fact_ref.fact).owner,
            public_owner,
            "ordinary call facts must not expose satisfier strengthening",
        );
    }

    assert_eq!(
        invocation
            .runtime_call
            .expect("the static proof-output dispatch retains its Unit call")
            .statement_index,
        contract_call.statement_index,
    );
}

#[test]
fn static_named_witness_requirement_call_hides_satisfier_strengthening_selector() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Producer {
            machine Self::produce(&self)
            requires public_in: ready()
            ensures public_out: ready();
        }
        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self)
            requires local_in: ready()
            ensures public_out: ready()
            ensures private_out: ready()
            {
                public_out = local_in;
                private_out = local_in;
            }
        }
        machine invoke<Element, Order: Element satisfies Producer>(value: &Element)
        requires incoming: ready()
        {
            let (; private_out: leaked) = Order::produce(value; incoming);
        }
        machine caller(value: &Token)
        requires incoming: ready()
        {
            invoke<Token, TokenProducer>(value; incoming);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("a static requirement call must hide private satisfier strengthening");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("publishes no proof-output selector `private_out`")
    }));
}

#[test]
fn static_named_witness_requirement_call_fences_wider_public_lanes() {
    let source = r#"
        trait Evidence {}
        proposition ready() evidence Evidence;
        trait Producer {
            machine Self::produce(&self)
            requires first: ready()
            requires second: ready()
            ensures public_out: ready();
        }
        data Token {}
        TokenProducer: Token satisfies Producer {
            machine produce(&self)
            requires local_first: ready()
            requires local_second: ready()
            ensures public_out: ready()
            { public_out = local_first; }
        }
        machine invoke<Element, Order: Element satisfies Producer>(value: &Element)
        requires incoming_first: ready()
        requires incoming_second: ready()
        {
            let (; public_out: result) =
                Order::produce(value; incoming_first, incoming_second);
        }
        machine caller(value: &Token)
        requires incoming_first: ready()
        requires incoming_second: ready()
        {
            invoke<Token, TokenProducer>(value; incoming_first, incoming_second);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let diagnostics = lower_typed_trees(typed)
        .expect_err("the first static requirement rung must reject wider public lanes");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "must own exactly one named requires input and one unconditional named ensures output",
        )
    }));
}

#[test]
fn explicit_conformance_evidence_forwards_through_a_generic_caller() {
    let source = r#"
        trait Ranked {
            machine Self::before(&self, other: &Self) -> bool;
        }

        data Card { rank: i32; }

        CardOrder: Card satisfies Ranked {
            machine before(&self, other: &Card) -> bool {
                self.rank < other.rank
            }
        }

        machine choose<Element, Order: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            Order::before(left, right)
        }

        machine forward<Element, Evidence: Element satisfies Ranked>(
            left: &Element,
            right: &Element
        ) -> bool {
            choose<Element, Evidence>(left, right)
        }

        machine caller(left: &Card, right: &Card) -> bool {
            forward<Card, CardOrder>(left, right)
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let selected = typed
        .conformances()
        .iter()
        .find(|conformance| {
            conformance
                .alias
                .as_ref()
                .is_some_and(|name| name.as_str() == "CardOrder")
        })
        .expect("selected conformance")
        .symbol;
    let checked = lower_typed_trees(typed)
        .expect("concrete evidence should propagate through the specialized generic caller");

    let specialization_count = |name: &str| {
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template && machine.name.as_str() == name
                })
            })
            .count()
    };
    assert_eq!(specialization_count("forward"), 1);
    assert_eq!(specialization_count("choose"), 1);
    assert!(
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template
                        && matches!(machine.name.as_str(), "forward" | "choose")
                })
            })
            .all(|specialization| specialization.conformance_arguments == [selected])
    );
}

#[test]
fn accepted_template_instances_share_one_commitment_and_pin_argument_contracts() {
    let source = r#"
        data Light {}
        data Main { light: Light; number: i32; }

        machine Light::touch(value: &Light) {}
        machine touch_number(value: &i32) {}

        boundary machine admitted<T, machine F>(value: &T)
        where machine F(item: &T);
        ensures true;

        machine Main::run(&mut self) {
            admitted<Light::touch>(&self.light);
            admitted<touch_number>(&self.number);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("accepted generic instances should check");
    let instances: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "admitted"
            })
        })
        .collect();

    assert_eq!(instances.len(), 2);
    assert!(instances.iter().all(|instance| {
        instance.accepted_template_commitment.as_deref() == Some("admitted")
            && instance.template_contract_fingerprint != 0
            && instance.machine_argument_contract_fingerprints.len() == 1
            && instance.machine_argument_contract_fingerprints[0] != 0
    }));
    assert_eq!(
        instances[0].template_contract_fingerprint,
        instances[1].template_contract_fingerprint
    );
    assert_ne!(
        instances[0].machine_argument_contract_fingerprints,
        instances[1].machine_argument_contract_fingerprints
    );
    assert_ne!(instances[0].fingerprint, instances[1].fingerprint);
}

#[test]
fn specialization_identity_changes_with_selected_machine_contract() {
    fn fingerprint(extra_contract: &str) -> u64 {
        let source = format!(
            r#"
                data Main {{}}
                machine selected(value: &i32)
                {extra_contract}
                {{}}
                machine apply<T, machine F>(value: &T)
                where machine F(item: &T)
                {{ F(value); }}
                machine caller(value: &i32) {{ apply<selected>(value); }}
                machine Main::run(&mut self) {{}}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        lower_typed_trees(typed)
            .expect("specialization should check")
            .machine_specializations[0]
            .fingerprint
    }

    assert_ne!(fingerprint(""), fingerprint("ensures true;"));
}

#[test]
fn specialization_identity_ignores_selected_machine_body_edits() {
    fn fingerprint(body: &str) -> u64 {
        let source = format!(
            r#"
                data Main {{}}
                machine selected(value: &i32) {{ {body} }}
                machine apply<T, machine F>(value: &T)
                where machine F(item: &T)
                {{ F(value); }}
                machine caller(value: &i32) {{ apply<selected>(value); }}
                machine Main::run(&mut self) {{}}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        lower_typed_trees(typed)
            .expect("specialization should check")
            .machine_specializations[0]
            .fingerprint
    }

    assert_eq!(fingerprint(""), fingerprint("let one: i32 = 1;"));
}

#[test]
fn generic_template_identity_is_positional_across_parameter_renames() {
    fn fingerprint(machine_parameter: &str, value: &str, item: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<machine {machine_parameter}>({value}: i32)
                where machine {machine_parameter}({item}: i32)
                    requires {item} > 0;
                requires {value} > 0;
                ensures true;
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("accepted template should exist");
        crate::monomorphization::generic_machine_template_fingerprint(&typed, admitted.symbol)
            .expect("generic template should have an identity")
    }

    assert_eq!(
        fingerprint("F", "value", "item"),
        fingerprint("Operation", "input", "candidate")
    );
}

#[test]
fn generic_template_identity_normalizes_crash_route_buckets() {
    fn fingerprint(crash_clauses: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<T>(first: bool, second: bool)
                {crash_clauses}
                ensures true;
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_fingerprint(&typed, admitted.symbol)
            .expect("generic template should have an identity")
    }

    let grouped = r#"
        crashes Trap
            first
            second
    "#;
    let split = r#"
        crashes Trap
            second
        crashes Trap
            first
    "#;
    let duplicated = r#"
        crashes Trap
            first
            second
            first
    "#;
    let unconditional = r#"
        crashes Trap
    "#;
    let explicit_true = r#"
        crashes Trap
            true
    "#;
    let unconditional_with_guard = r#"
        crashes Trap
            first
        crashes Trap
    "#;

    assert_eq!(fingerprint(grouped), fingerprint(split));
    assert_eq!(fingerprint(grouped), fingerprint(duplicated));
    assert_eq!(fingerprint(unconditional), fingerprint(explicit_true));
    assert_eq!(
        fingerprint(unconditional),
        fingerprint(unconditional_with_guard)
    );
    assert_ne!(fingerprint(grouped), fingerprint(unconditional));

    fn slot_fingerprint(crash_clauses: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<T, machine Operation>()
                where machine Operation(first: bool, second: bool)
                    {crash_clauses};
                ensures true;
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_fingerprint(&typed, admitted.symbol)
            .expect("generic template should have an identity")
    }

    assert_eq!(slot_fingerprint(grouped), slot_fingerprint(split));
    assert_eq!(slot_fingerprint(grouped), slot_fingerprint(duplicated));
    assert_eq!(
        slot_fingerprint(unconditional),
        slot_fingerprint(explicit_true)
    );
    assert_eq!(
        slot_fingerprint(unconditional),
        slot_fingerprint(unconditional_with_guard)
    );
}

#[test]
fn generic_template_identity_pins_conformance_bounds_positionally() {
    fn fingerprint(parameter: &str, trait_name: &str) -> u64 {
        let source = format!(
            r#"
                trait First {{ machine inspect(value: &Self); }}
                trait Second {{ machine inspect(value: &Self); }}
                machine admitted<{parameter}>(value: &{parameter})
                where {parameter} satisfies {trait_name}
                {{}}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_fingerprint(&typed, admitted.symbol)
            .expect("generic template should have an identity")
    }

    assert_eq!(fingerprint("T", "First"), fingerprint("Item", "First"));
    assert_ne!(fingerprint("T", "First"), fingerprint("T", "Second"));
}

#[test]
fn generic_template_identity_pins_selected_open_index_operation_authority() {
    fn fingerprint(operator_namespace: &str) -> u64 {
        let source = format!(
            r#"
                domain<T, const I: u64> T::Indexed<I>;

                trait IndexAdd {{
                    machine add(a: Self, b: Self) -> Self;
                    machine add_comm(a: Self, b: Self) -> Self
                    ensures add(a, b) == add(b, a);
                    machine add_assoc(a: Self, b: Self, c: Self) -> Self
                    ensures add(add(a, b), c) == add(a, add(b, c));
                }}

                operator + {operator_namespace}::plus(left: u64, right: u64) -> u64;

                machine plus_index(a: u64, b: u64) -> u64
                satisfies {operator_namespace}::plus, IndexAdd::add as Canonical
                {{ 0 }}

                machine plus_index_comm(a: u64, b: u64) -> u64
                satisfies IndexAdd::add_comm as Canonical
                ensures plus_index(a, b) == plus_index(b, a)
                {{ 0 }}

                machine plus_index_assoc(a: u64, b: u64, c: u64) -> u64
                satisfies IndexAdd::add_assoc as Canonical
                ensures plus_index(plus_index(a, b), c) == plus_index(a, plus_index(b, c))
                {{ 0 }}

                boundary machine admitted<const A: u64, const B: u64>()
                    -> i64 in Indexed<A + B>;
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let mut typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        psi_validation::normalize_open_index_expressions(&mut typed)
            .expect("exact proved index algebra should normalize");
        crate::monomorphization::refresh_closed_domain_instance_identities(&mut typed)
            .expect("selected authority should refresh indexed instance identity");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_fingerprint(&typed, admitted.symbol)
            .expect("generic template should have an identity")
    }

    assert_ne!(
        fingerprint("IndexAlgebra"),
        fingerprint("AlternateIndexAlgebra")
    );
}

#[test]
fn generic_template_identity_pins_independent_operational_interfaces() {
    fn fingerprint(source: String) -> u64 {
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens)
            .unwrap_or_else(|error| panic!("parse should succeed for `{source}`: {error:?}"));
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_fingerprint(&typed, admitted.symbol)
            .expect("generic template should have an identity")
    }

    fn template_fingerprint(template_clause: &str) -> u64 {
        let source = format!(
            r#"
                boundary machine admitted<T>(value: &T) {template_clause}
                ensures true;
            "#
        );
        fingerprint(source)
    }

    fn slot_fingerprint(slot_clause: &str) -> u64 {
        fingerprint(format!(
            r#"
                boundary machine admitted<T, machine F>(value: &T)
                where machine F(item: &T) {slot_clause}
                ensures true;
            "#
        ))
    }

    let template_base = template_fingerprint("");
    assert_ne!(template_base, template_fingerprint("suspends;"));
    assert_ne!(template_base, template_fingerprint("blocks;"));

    let slot_base = slot_fingerprint(";");
    assert_ne!(slot_base, slot_fingerprint("suspends;"));
    assert_ne!(slot_base, slot_fingerprint("blocks;"));
    assert_ne!(slot_fingerprint("suspends;"), slot_fingerprint("blocks;"));

    fn reach_fingerprint(reach: &str) -> u64 {
        fingerprint(format!(
            r#"
                boundary trait Readable {{}}
                boundary trait Filesystem: Readable {{}}

                boundary machine admitted<T>(value: &T)
                reaches {reach}
                ensures true;
            "#
        ))
    }

    assert_eq!(
        reach_fingerprint("Filesystem"),
        reach_fingerprint("Filesystem + Readable"),
        "template identity must consume the normalized service row, including parent closure"
    );

    fn slot_reach_fingerprint(reach: &str) -> u64 {
        fingerprint(format!(
            r#"
                boundary trait Readable {{}}
                boundary trait Filesystem: Readable {{}}

                boundary machine admitted<T, machine F>(value: &T)
                where machine F(item: &T) reaches {reach};
                ensures true;
            "#
        ))
    }

    assert_eq!(
        slot_reach_fingerprint("Filesystem"),
        slot_reach_fingerprint("Filesystem + Readable"),
        "machine-parameter identity must consume the normalized service row"
    );
}

#[test]
fn generic_template_identity_distinguishes_structural_and_nominal_machine_contracts() {
    fn fingerprint(contract: &str) -> u64 {
        let source = format!(
            r#"
                trait Handler {{
                    machine call(value: i32) -> i32;
                }}

                machine admitted<machine F>(value: i32) -> i32
                {contract}
                {{
                    0
                }}
            "#
        );
        let tokens = Lexer::new(&source)
            .tokenize()
            .expect("tokenize should succeed");
        let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
        let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
        let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
        let admitted = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "admitted")
            .expect("generic template should exist");
        crate::monomorphization::generic_machine_template_fingerprint(&typed, admitted.symbol)
            .expect("generic template should have an identity")
    }

    assert_ne!(
        fingerprint("where machine F(value: i32) -> i32;"),
        fingerprint("where machine F satisfies Handler::call;")
    );
}

#[test]
fn consuming_seq_map_specializes_recursive_machine_parameter_calls() {
    let source = r#"
        data Seq<T> {
            case Empty;
            case Cons(head: T, tail: Seq<T>);
        }
        data Main {}

        machine increment(value: u64) -> u64 { value + 1 }

        machine map<T, U, machine F>(items: Seq<T>) -> Seq<U>
        where machine F(value: T) -> U;
        terminates by items;
        {
            transition items {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> Seq::Cons {
                    head: F(head),
                    tail: map<F>(tail)
                }
            }
        }

        machine caller(items: Seq<u64>) -> Seq<u64> {
            map<increment>(items)
        }
        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("consuming Seq map should specialize");

    let map_instances: Vec<_> = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "map"
            })
        })
        .collect();
    assert_eq!(map_instances.len(), 1);
    assert_eq!(map_instances[0].type_arguments, ["u64", "u64"]);
    assert_eq!(map_instances[0].machine_arguments.len(), 1);
}

#[test]
fn unused_recursive_generic_value_template_is_not_emitted_or_fenced() {
    let source = r#"
        data Seq<T> {
            case Empty;
            case Cons(head: T, tail: Seq<T>);
        }
        data Main {}

        machine map<T, U, machine F>(items: Seq<T>) -> Seq<U>
        where machine F(value: T) -> U;
        terminates by items;
        {
            transition items {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> Seq::Cons {
                    head: F(head),
                    tail: map<F>(tail)
                }
            }
        }

        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("unused generic template should remain legal");
    let map = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "map")
        .expect("generic template should remain in checked semantic data");
    assert!(!checked.machine_type_parameters(map).is_empty());
}

#[test]
fn const_generic_template_is_not_consumed_by_machine_specialization() {
    let source = r#"
        data Unit {}
        domain<T, const U: Unit> T::Quantity<U>;

        machine retag<const To: Unit>(value: i64) -> i64 in Quantity<To> {
            transition { _ -> (value as i64 in Quantity<To>) }
        }

        data Main {}
        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("const-generic template should validate");
    let retag = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retag")
        .expect("retag template should remain in checked semantic data");
    let [parameter] = checked.machine_type_parameters(retag) else {
        panic!("retag should retain its const binder");
    };
    assert!(matches!(
        parameter.kind,
        psi_typed_trees::data::TypeParameterKind::Const { .. }
    ));
}

#[test]
fn const_generic_result_indices_produce_distinct_concrete_machine_instances() {
    let source = r#"
        domain<T, const U: u64> T::Quantity<U>;

        machine retag<const To: u64>(value: i64) -> i64 in Quantity<To> {
            transition { _ -> (value as i64 in Quantity<To>) }
        }

        data Main {}
        machine Main::run(&mut self) {
            let first: i64 in Quantity<1> = retag(70);
            let second: i64 in Quantity<2> = retag(70);
        }
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    crate::specialize_static_machine_calls(&mut typed)
        .expect("const result indices should specialize before validation");
    let concrete_return_domains = typed
        .machine_specializations
        .iter()
        .map(|specialization| {
            let machine = typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == specialization.instance)
                .expect("specialized machine");
            let return_type = typed
                .machine_states(machine)
                .first()
                .expect("entry state")
                .return_type;
            let psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } =
                typed.type_reference_table.type_reference(return_type)
            else {
                panic!("specialized return should be constrained");
            };
            let [psi_typed_trees::types::TypeConstraintNode::Domain(domain)] =
                typed.type_reference_table.constraints(*constraints)
            else {
                panic!("specialized return should carry Quantity");
            };
            typed
                .semantic_domains
                .name(domain.semantic_id)
                .expect("indexed semantic identity")
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        concrete_return_domains,
        ["Quantity<integer:u64:1>", "Quantity<integer:u64:2>"]
    );
    let cast_domains = typed
        .expression_table
        .iter_expressions()
        .filter_map(|(_, expression)| {
            let psi_typed_trees::expression::ExpressionNode::Cast(cast) = expression else {
                return None;
            };
            typed
                .semantic_domains
                .name(cast.semantic_domain_id)
                .map(str::to_owned)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cast_domains,
        ["Quantity<integer:u64:1>", "Quantity<integer:u64:2>"]
    );
    let checked = lower_typed_trees(typed).expect("const result indices should specialize");

    let retag = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retag")
        .expect("retag template instance");
    assert!(checked.machine_type_parameters(retag).is_empty());
    let specializations = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == retag.symbol)
        .collect::<Vec<_>>();
    assert_eq!(specializations.len(), 2);
    assert_ne!(specializations[0].instance, specializations[1].instance);
    assert_eq!(specializations[0].const_arguments, ["1"]);
    assert_eq!(specializations[1].const_arguments, ["2"]);
    assert_eq!(
        specializations[0].const_argument_identities,
        ["named(integer-const(1))"]
    );
    assert_eq!(
        specializations[1].const_argument_identities,
        ["named(integer-const(2))"]
    );
    assert_ne!(
        specializations[0].fingerprint,
        specializations[1].fingerprint
    );

    for specialization in specializations {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == specialization.instance)
            .expect("concrete retag instance");
        let state = checked
            .machine_states(machine)
            .first()
            .expect("retag entry state");
        let psi_typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = checked
            .type_reference_table
            .type_reference(state.return_type)
        else {
            panic!("specialized retag result should remain constrained");
        };
        let [psi_typed_trees::types::TypeConstraintNode::Domain(result_domain)] =
            checked.type_reference_table.constraints(*constraints)
        else {
            panic!("specialized retag result should carry Quantity");
        };
        let cast_id = checked
            .expression_table
            .iter_expressions()
            .filter_map(|(_, expression)| {
                let psi_typed_trees::expression::ExpressionNode::Cast(cast) = expression else {
                    return None;
                };
                (cast.semantic_domain_symbol == result_domain.symbol
                    && cast.semantic_domain_id == result_domain.semantic_id)
                    .then_some(cast.semantic_domain_id)
            })
            .find(|identity| *identity == result_domain.semantic_id);
        assert_eq!(cast_id, Some(result_domain.semantic_id));
    }
}

#[test]
fn contract_only_static_selections_do_not_consume_generic_machine_schema() {
    let source = r#"
        data Index { case Zero; }
        data Stream<machine S>
        where machine S(index: Index) -> Index;
        { case Empty; case More(tail: Stream<S>); }
        data Main {}

        boundary machine source(index: Index) -> Index;

        machine equivalent<machine A, machine B>(a: Stream<A>, b: Stream<B>) -> bool
        where machine A(index: Index) -> Index;
        where machine B(index: Index) -> Index;
        {
            transition { _ -> (true) }
        }

        machine reflexive<machine A>(a: Stream<A>)
        where machine A(index: Index) -> Index;
        ensures equivalent<A, A>(a, a)
        {
        }

        machine concrete_premise(a: Stream<source>)
        requires equivalent<source, source>(a, a)
        {
        }

        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    let checked = lower_typed_trees(typed).expect("contract schemas should remain generic");

    let equivalent = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "equivalent")
        .expect("generic relation schema");
    assert_eq!(checked.machine_type_parameters(equivalent).len(), 2);
    assert!(
        checked
            .machine_specializations
            .iter()
            .all(|specialization| specialization.template != equivalent.symbol)
    );
}

#[test]
fn consuming_seq_filter_borrows_each_value_before_preserving_or_dropping_it() {
    let source = r#"
        data Seq<T> {
            case Empty;
            case Cons(head: T, tail: Seq<T>);
        }
        data Main {}

        machine positive(value: &i32) -> bool { value > 0 }

        machine filter<T, machine Predicate>(items: Seq<T>) -> Seq<T>
        where machine Predicate(value: &T) -> bool;
        terminates by items;
        {
            transition items {
                Seq::Empty -> Seq::Empty
                Seq::Cons { head, tail } -> choose(
                    Predicate(head),
                    head,
                    filter<Predicate>(tail)
                )
            }

            state choose(keep: bool, head: T, tail: Seq<T>) -> Seq<T> {
                transition keep {
                    true -> Seq::Cons { head: head, tail: tail }
                    false -> tail
                }
            }
        }

        machine caller(items: Seq<i32>) -> Seq<i32> {
            filter<positive>(items)
        }
        machine Main::run(&mut self) {}
    "#;

    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = lower_syntax_trees(&syntax).expect("symbol resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed).expect("consuming Seq filter should specialize");
}
