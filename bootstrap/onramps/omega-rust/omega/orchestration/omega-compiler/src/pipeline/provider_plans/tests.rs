use super::*;

fn normalized_machine_identity(typed: &TypedTrees, name: &str) -> String {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing typed machine `{name}`"));
    typed
        .normalized_machine_overload_identity(machine)
        .expect("typed machine must have an entry overload")
        .identity()
}

fn derive_provider_fixture(source: &str) -> (TypedTrees, ProviderPlan) {
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize provider fixture");
    let syntax =
        psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse provider fixture");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve provider fixture");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type provider fixture");
    let plans = derive_satisfies_plans(&typed, None);
    let [plan] = plans.as_slice() else {
        panic!(
            "provider fixture must derive exactly one plan, got {}",
            plans.len()
        );
    };
    (typed, plan.clone())
}

#[test]
fn provider_derivation_consumes_typed_external_binding_identity() {
    let source = |library: &str, symbol: &str| {
        format!(
            r#"
                boundary trait Process {{
                    machine exit(code: i32);
                }}

                machine exit_leaf(code: i32)
                satisfies Process::exit
                via Binding::DllImport("{library}", "{symbol}");
            "#
        )
    };
    let retained_source = source("retained-library", "retained-symbol");
    let retained_tokens = psi_source_files_to_tokens::Lexer::new(&retained_source)
        .tokenize()
        .expect("tokenize retained binding");
    let retained_syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&retained_tokens)
        .expect("parse retained binding");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&retained_syntax)
        .expect("resolve retained binding");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type retained binding");

    // Derivation accepts no syntax tree: the exact typed id/table is its
    // only external-binding authority.
    let plans = derive_satisfies_plans(&typed, None);
    let [plan] = plans.as_slice() else {
        panic!("one external provider plan")
    };

    assert_eq!(
        plan.rows[0].binding,
        ProviderBinding::StringBackedImportBootstrap {
            library: "retained-library".to_owned(),
            symbol: "retained-symbol".to_owned(),
        }
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
        via Binding::VtableSlot(1);

        machine second_leaf()
        satisfies Pair::second
        via Binding::VtableSlot(2);
    "#;
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize two-row provider fixture");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse two-row provider fixture");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve two-row provider fixture");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type two-row provider fixture");
    let derived = derive_satisfies_plans_with_provenance(&typed, None);
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
    let mut checked = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("bounded provider should check");
    let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("one selected PIC plan");
    let selected =
        bind_selected_provider_plan_facts(&mut checked, std::slice::from_ref(&plan), selected, &[])
            .expect("selected PIC reach should resolve");
    let requirement_identity = plan.rows[0].requirement_identity.as_str();
    let resolution = selected
        .installation_reach_resolution(requirement_identity)
        .expect("bounded requirement resolution");

    assert_eq!(resolution.upper_bound, ["MachineControl", "PortIo"]);
    assert_eq!(resolution.resolved_row, ["PortIo"]);
    assert_eq!(
        resolution.provider_plan_identity,
        plan.identity_fingerprint()
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
    let mut checked = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("boundary operator provider should check");
    let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("one selected boundary operator plan");
    let selected =
        bind_selected_provider_plan_facts(&mut checked, std::slice::from_ref(&plan), selected, &[])
            .expect("boundary operator selection must not require a trait installation row");

    assert!(selected.installation_reach_resolutions().is_empty());
}

fn selection_plan(name: &str, methods: &[&str], rows: &[&str]) -> ProviderPlan {
    ProviderPlan {
        name: name.to_owned(),
        provider_type: name.to_owned(),
        provider_type_package_identity: None,
        target: String::new(),
        schema: ServiceSchema {
            trait_name: "Pair".to_owned(),
            trait_package_identity: None,
            methods: methods
                .iter()
                .map(|method| omega_effects::provider_plan::ServiceMethod {
                    name: (*method).to_owned(),
                    requirement_owner: "Pair".to_owned(),
                    requirement_owner_package_identity: None,
                    requirement_identity: format!("Pair::{method}"),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["Pair".to_owned()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_fingerprint: None,
                })
                .collect(),
        },
        rows: rows
            .iter()
            .map(|method| ProviderPlanRow {
                method: (*method).to_owned(),
                requirement_identity: format!("Pair::{method}"),
                binding: ProviderBinding::VtableSlot { index: 0 },
            })
            .collect(),
        origin_package_identity: None,
        origin_package: String::new(),
    }
}

fn package_selection(
    boundary_trait: &str,
    boundary_package: psi_core::PackageKeyIdentity,
    provider_type: &str,
    provider_package: psi_core::PackageKeyIdentity,
) -> crate::pipeline::build_config::ProviderSelection {
    let mut selection = crate::pipeline::build_config::ProviderSelection::exact_for_test(
        boundary_trait,
        provider_type,
    );
    selection.boundary_trait.package = Some(boundary_package);
    selection.provider_type.package = Some(provider_package);
    selection
}

fn selected_plan_names(plans: &[ProviderPlan]) -> Vec<String> {
    plans.iter().map(|plan| plan.name.clone()).collect()
}

#[test]
fn provider_grant_ledger_resolves_one_exact_selector_subject() {
    let first = selection_plan("FirstProvider", &["first"], &["first"]);
    let candidates = vec![first.clone()];
    let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
        &candidates,
        &[first.name.clone()],
    )
    .expect("selected provider");
    let grants = resolve_selected_provider_grants(
        &candidates,
        &selected,
        &[
            "FirstProvider".to_owned(),
            "Pair".to_owned(),
            "OtherFact".to_owned(),
            "Pair".to_owned(),
        ],
    )
    .expect("exact provider selectors");

    assert_eq!(grants.len(), 3);
    assert_eq!(grants[0].selector_kind, ProviderGrantSelectorKind::PlanName);
    assert_eq!(grants[0].commitment(), "provider plan: FirstProvider");
    assert_eq!(
        grants[1].selector_kind,
        ProviderGrantSelectorKind::ProviderSlot
    );
    assert_eq!(grants[1].commitment(), "provider slot: Pair");
    assert_eq!(grants[2], grants[1]);
    assert!(
        grants
            .iter()
            .all(|grant| grant.selected_plan_identity == first.identity_fingerprint())
    );

    let mut same_subject = first.clone();
    same_subject.name = same_subject.schema.trait_name.clone();
    let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&same_subject),
        &[same_subject.name.clone()],
    )
    .expect("same plan and slot subject");
    let grants = resolve_selected_provider_grants(
        std::slice::from_ref(&same_subject),
        &selected,
        &[same_subject.name.clone()],
    )
    .expect("same subject is canonical");
    assert_eq!(grants[0].selector_kind, ProviderGrantSelectorKind::PlanName);
}

#[test]
fn provider_grant_ledger_rejects_ambiguity_and_unselected_subjects() {
    enum Corruption {
        DuplicatePlanName,
        UnselectedPlan,
        MissingSelectedSlot,
        DistinctPlanAndSlot,
        UnselectedPlanAndSelectedSlot,
        SelectedCandidateDrift,
    }
    let cases = [
        (
            Corruption::DuplicatePlanName,
            "2 exact provider plan candidates",
        ),
        (Corruption::UnselectedPlan, "unselected provider plan"),
        (
            Corruption::MissingSelectedSlot,
            "provider slot with no selected provider plan",
        ),
        (
            Corruption::DistinctPlanAndSlot,
            "distinct provider plan and slot subjects",
        ),
        (
            Corruption::UnselectedPlanAndSelectedSlot,
            "unselected provider plan",
        ),
        (
            Corruption::SelectedCandidateDrift,
            "resolves to 0 exact candidate rows",
        ),
    ];

    for (corruption, expected) in cases {
        let mut first = selection_plan("FirstProvider", &["first"], &["first"]);
        first.schema.trait_name = "FirstSlot".to_owned();
        let mut second = selection_plan("SecondProvider", &["first"], &["first"]);
        second.schema.trait_name = "SecondSlot".to_owned();
        let (candidates, selected_candidates, selected_names, grant) = match corruption {
            Corruption::DuplicatePlanName => {
                second.name = first.name.clone();
                (
                    vec![first.clone(), second],
                    vec![first.clone()],
                    vec![first.name.clone()],
                    first.name.clone(),
                )
            }
            Corruption::UnselectedPlan => (
                vec![first.clone(), second.clone()],
                vec![first.clone()],
                vec![first.name.clone()],
                second.name.clone(),
            ),
            Corruption::MissingSelectedSlot => (
                vec![first.clone(), second.clone()],
                vec![first.clone()],
                vec![first.name.clone()],
                second.schema.trait_name.clone(),
            ),
            Corruption::DistinctPlanAndSlot => {
                second.schema.trait_name = first.name.clone();
                (
                    vec![first.clone(), second.clone()],
                    vec![first.clone(), second.clone()],
                    vec![first.name.clone(), second.name.clone()],
                    first.name.clone(),
                )
            }
            Corruption::UnselectedPlanAndSelectedSlot => {
                second.schema.trait_name = first.name.clone();
                (
                    vec![first.clone(), second.clone()],
                    vec![second.clone()],
                    vec![second.name.clone()],
                    first.name.clone(),
                )
            }
            Corruption::SelectedCandidateDrift => {
                let mut drifted = first.clone();
                drifted.origin_package = "drifted".to_owned();
                (
                    vec![first.clone()],
                    vec![drifted],
                    vec![first.name.clone()],
                    first.name.clone(),
                )
            }
        };
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            &selected_candidates,
            &selected_names,
        )
        .expect("selected fixture");
        let diagnostic = resolve_selected_provider_grants(&candidates, &selected, &[grant])
            .expect_err("invalid provider selector custody must reject");
        assert!(
            diagnostic.message.contains(expected),
            "expected {expected:?}, got {diagnostic:?}",
        );
    }
}

fn push_boundary_requirement(
    checked: &mut psi_checked_trees::CheckedTrees,
    owner_symbol: psi_symbols::SymbolHandle,
    owner_name: &str,
    requirement_symbol: psi_symbols::SymbolHandle,
    requirement_name: &str,
) -> String {
    let mut owner = psi_typed_trees::trait_definition::TraitDefinition {
        symbol: owner_symbol,
        is_boundary: true,
        name: psi_typed_trees::name::Identifier::generated(owner_name),
        ..Default::default()
    };
    checked.typed.push_trait_machine_signature(
        &mut owner,
        psi_typed_trees::signature::StateSignature {
            symbol: requirement_symbol,
            name: psi_typed_trees::name::Identifier::generated(requirement_name),
            ..Default::default()
        },
    );
    checked.typed.push_trait_definition(owner);
    let owner = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == owner_symbol)
        .expect("inserted boundary owner");
    let requirement = checked
        .typed
        .trait_machine_signatures(owner)
        .iter()
        .find(|requirement| requirement.symbol == requirement_symbol)
        .expect("inserted boundary requirement");
    checked
        .typed
        .normalized_trait_requirement_overload_identity(owner, requirement)
        .identity()
}

fn set_exact_requirement(
    plan: &mut ProviderPlan,
    schema: &str,
    owner: &str,
    requirement_identity: &str,
) {
    plan.schema.trait_name = schema.to_owned();
    plan.schema.methods[0].requirement_owner = owner.to_owned();
    plan.schema.methods[0].requirement_identity = requirement_identity.to_owned();
    plan.rows[0].requirement_identity = requirement_identity.to_owned();
}

fn append_admitted_fact(
    checked: &mut psi_checked_trees::CheckedTrees,
    subject_symbol: psi_symbols::SymbolHandle,
    domain_symbol: psi_symbols::SymbolHandle,
    owner_symbol: psi_symbols::SymbolHandle,
    requirement_symbol: psi_symbols::SymbolHandle,
) -> psi_facts::FactHandle {
    let place = checked.facts.semantic.append_symbol_place(subject_symbol);
    checked.facts.semantic.append_fact(psi_facts::Fact {
        place: psi_facts::FactPlace::Place(place),
        point: psi_facts::ProgramPoint::Global,
        origin: psi_facts::FactOrigin::CallEnsures,
        evidence: psi_facts::QualificationEvidence::from_admitted_requirement(
            owner_symbol,
            requirement_symbol,
        ),
        payload: psi_facts::FactPayload::DomainMembership {
            value: Default::default(),
            domain: Default::default(),
            domain_symbol,
        },
    })
}

#[test]
fn selected_synchronous_invocation_graph_rejects_cycles_only_after_selection() {
    let mut alpha = selection_plan("alpha", &["run"], &["run"]);
    alpha.schema.trait_name = "Alpha".to_owned();
    alpha.schema.methods[0].synchronous_invocations = vec!["Beta".to_owned()];
    let mut beta = selection_plan("beta", &["run"], &["run"]);
    beta.schema.trait_name = "Beta".to_owned();
    beta.schema.methods[0].synchronous_invocations = vec!["Alpha".to_owned()];

    validate_selected_synchronous_invocation_cycles(&TypedTrees::default(), &[alpha.clone()])
        .expect("an unselected potential return edge is not realized");

    let diagnostics =
        validate_selected_synchronous_invocation_cycles(&TypedTrees::default(), &[alpha, beta])
            .expect_err("the selected Alpha -> Beta -> Alpha graph must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("cyclic synchronous `invokes` graph")
            && diagnostic.message.contains("Alpha -> Beta -> Alpha")
    }));
}

#[derive(Clone, Copy, Debug)]
enum SelectedInvocationDrift {
    None,
    EmptyPlanName,
    DuplicatePlan,
    DuplicateSelectedSchema,
    EmptyMethodIdentity,
    DuplicateMethodIdentity,
    EmptyRowIdentity,
    CrossRowIdentity,
    MissingRow,
    DuplicateRow,
    EmptyInvocation,
    DuplicateInvocation,
}

#[test]
fn selected_synchronous_invocation_identity_drift_rejects_exactly() {
    let cases = [
        (SelectedInvocationDrift::None, None),
        (
            SelectedInvocationDrift::EmptyPlanName,
            Some("name is empty"),
        ),
        (
            SelectedInvocationDrift::DuplicatePlan,
            Some("listed more than once"),
        ),
        (
            SelectedInvocationDrift::DuplicateSelectedSchema,
            Some("realized by more than one selected ProviderPlan"),
        ),
        (
            SelectedInvocationDrift::EmptyMethodIdentity,
            Some("schema method `run` has no exact"),
        ),
        (
            SelectedInvocationDrift::DuplicateMethodIdentity,
            Some("contains 2 schema methods"),
        ),
        (
            SelectedInvocationDrift::EmptyRowIdentity,
            Some("binds 0 exact synchronous-invocation rows"),
        ),
        (
            SelectedInvocationDrift::CrossRowIdentity,
            Some("binds 0 exact synchronous-invocation rows"),
        ),
        (
            SelectedInvocationDrift::MissingRow,
            Some("binds 0 exact synchronous-invocation rows"),
        ),
        (
            SelectedInvocationDrift::DuplicateRow,
            Some("binds 2 exact synchronous-invocation rows"),
        ),
        (
            SelectedInvocationDrift::EmptyInvocation,
            Some("empty synchronous-invocation identity"),
        ),
        (
            SelectedInvocationDrift::DuplicateInvocation,
            Some("not strictly increasing"),
        ),
    ];

    for (drift, expected) in cases {
        let mut alpha = selection_plan("alpha", &["run"], &["run"]);
        alpha.schema.trait_name = "pkg::Alpha".to_owned();
        alpha.schema.methods[0].synchronous_invocations = vec!["pkg::Beta".to_owned()];
        let mut beta = selection_plan("beta", &["run"], &["run"]);
        beta.schema.trait_name = "pkg::Beta".to_owned();
        let mut plans = vec![alpha, beta];
        match drift {
            SelectedInvocationDrift::None => {}
            SelectedInvocationDrift::EmptyPlanName => plans[0].name.clear(),
            SelectedInvocationDrift::DuplicatePlan => {
                let duplicate = plans[0].clone();
                plans.push(duplicate);
            }
            SelectedInvocationDrift::DuplicateSelectedSchema => {
                plans[1].schema.trait_name = plans[0].schema.trait_name.clone();
            }
            SelectedInvocationDrift::EmptyMethodIdentity => {
                plans[0].schema.methods[0].requirement_identity.clear();
            }
            SelectedInvocationDrift::DuplicateMethodIdentity => {
                let duplicate = plans[0].schema.methods[0].clone();
                plans[0].schema.methods.push(duplicate);
            }
            SelectedInvocationDrift::EmptyRowIdentity => {
                plans[0].rows[0].requirement_identity.clear();
            }
            SelectedInvocationDrift::CrossRowIdentity => {
                plans[0].rows[0].requirement_identity = "pkg::Other::run".to_owned();
            }
            SelectedInvocationDrift::MissingRow => plans[0].rows.clear(),
            SelectedInvocationDrift::DuplicateRow => {
                let duplicate = plans[0].rows[0].clone();
                plans[0].rows.push(duplicate);
            }
            SelectedInvocationDrift::EmptyInvocation => {
                plans[0].schema.methods[0].synchronous_invocations = vec![String::new()];
            }
            SelectedInvocationDrift::DuplicateInvocation => {
                plans[0].schema.methods[0].synchronous_invocations =
                    vec!["pkg::Beta".to_owned(), "pkg::Beta".to_owned()];
            }
        }

        let result =
            validate_selected_synchronous_invocation_cycles(&TypedTrees::default(), &plans);
        match expected {
            None => result.expect("exact selected direct graph is valid"),
            Some(expected) => {
                let diagnostics = result.expect_err("identity drift must fail closed");
                assert!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains(expected)),
                    "{drift:?}: expected `{expected}`, got {diagnostics:?}",
                );
            }
        }
    }
}

#[test]
fn selected_synchronous_invocation_edges_require_complete_schema_identity() {
    let mut alpha = selection_plan("alpha", &["run"], &["run"]);
    alpha.schema.trait_name = "a::Alpha".to_owned();
    alpha.schema.methods[0].synchronous_invocations = vec!["a::Beta".to_owned()];
    let mut beta = selection_plan("beta", &["run"], &["run"]);
    beta.schema.trait_name = "b::Beta".to_owned();
    beta.schema.methods[0].synchronous_invocations = vec!["a::Alpha".to_owned()];

    validate_selected_synchronous_invocation_cycles(
        &TypedTrees::default(),
        &[alpha.clone(), beta.clone()],
    )
    .expect("same-leaf foreign schema must not manufacture an edge");

    alpha.schema.methods[0].synchronous_invocations = vec!["b::Beta".to_owned()];
    let diagnostics =
        validate_selected_synchronous_invocation_cycles(&TypedTrees::default(), &[alpha, beta])
            .expect_err("the exact canonical Alpha -> Beta -> Alpha graph must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("a::Alpha -> b::Beta -> a::Alpha")
    }));
}

#[test]
fn selected_synchronous_invocation_rejects_same_spelled_cross_package_targets() {
    let first_package =
        psi_core::PackageKeyIdentity::from_digest([0x41; 32]).expect("nonzero package identity");
    let second_package =
        psi_core::PackageKeyIdentity::from_digest([0x42; 32]).expect("nonzero package identity");
    let mut first = selection_plan("first", &["run"], &["run"]);
    first.schema.trait_name = "Shared".to_owned();
    first.schema.trait_package_identity = Some(first_package);
    first.schema.methods[0].synchronous_invocations = vec!["Shared".to_owned()];
    let mut second = selection_plan("second", &["run"], &["run"]);
    second.schema.trait_name = "Shared".to_owned();
    second.schema.trait_package_identity = Some(second_package);

    let diagnostics =
        validate_selected_synchronous_invocation_cycles(&TypedTrees::default(), &[first, second])
            .expect_err("a readable target cannot choose between package-qualified slots");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("ambiguous across 2 package-qualified boundary slots")
    }));
}

fn boundary_trait(symbol: u32, name: &str) -> psi_typed_trees::trait_definition::TraitDefinition {
    psi_typed_trees::trait_definition::TraitDefinition {
        symbol: psi_symbols::SymbolHandle::from_arena_index(symbol),
        name: psi_typed_trees::name::Identifier::generated(name),
        is_boundary: true,
        ..Default::default()
    }
}

fn checked_invocation_fixture(
    parameter_trait: u32,
    parameter_count: usize,
) -> (
    TypedTrees,
    ProviderPlan,
    psi_effects::InvocationInferencePlan,
) {
    let source = psi_symbols::SymbolHandle::from_arena_index(31);
    let target = psi_symbols::SymbolHandle::from_arena_index(32);
    let foreign_source = psi_symbols::SymbolHandle::from_arena_index(33);
    let machine_symbol = psi_symbols::SymbolHandle::from_arena_index(34);
    let mut typed = TypedTrees::default();
    typed.push_trait_definition(boundary_trait(31, "pkg::Source"));
    typed.push_trait_definition(boundary_trait(32, "pkg::Target"));
    typed.push_trait_definition(boundary_trait(33, "other::Source"));
    let type_symbol = match parameter_trait {
        31 => source,
        32 => target,
        33 => foreign_source,
        other => psi_symbols::SymbolHandle::from_arena_index(other),
    };
    let type_reference =
        typed
            .type_reference_table
            .insert(psi_typed_trees::types::TypeReferenceNode::Named {
                symbol: type_symbol,
                name: psi_typed_trees::name::Identifier::generated("binding"),
            });
    let mut entry = psi_typed_trees::state::State::default();
    typed.push_state_parameter(
        &mut entry,
        psi_typed_trees::signature::StateParameter {
            type_reference,
            name: psi_typed_trees::name::Identifier::generated("binding"),
            ..Default::default()
        },
    );
    let mut machine = psi_typed_trees::machine::Machine {
        symbol: machine_symbol,
        name: psi_typed_trees::name::Identifier::generated("Provider::run"),
        attached_data: Some(psi_typed_trees::name::Identifier::generated("Provider")),
        ..Default::default()
    };
    typed.push_machine_state(&mut machine, entry);
    let machine_identity = typed
        .normalized_machine_overload_identity(&machine)
        .expect("checked invocation machine must have an entry overload")
        .identity();
    typed.push_machine(machine);

    let mut plan = selection_plan("provider", &["run"], &["run"]);
    plan.provider_type = "Provider".to_owned();
    plan.schema.trait_name = "pkg::Source".to_owned();
    plan.schema.methods[0].requirement_owner = "pkg::Source".to_owned();
    plan.schema.methods[0].parameter_count = parameter_count;
    plan.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity: None,
    };
    let inferred = psi_effects::InvocationInferencePlan {
        machines: vec![psi_effects::MachineInvocationInference {
            machine: machine_symbol,
            published: Vec::new(),
            inferred_direct: vec![psi_effects::InvocationTarget::Parameter(0)],
            inferred_transitive: vec![psi_effects::InvocationTarget::Parameter(0)],
            effective: vec![psi_effects::InvocationTarget::Parameter(0)],
        }],
    };
    (typed, plan, inferred)
}

#[derive(Clone, Copy, Debug)]
enum CheckedInvocationDrift {
    None,
    MissingOwner,
    DuplicateOwner,
    AbsentMachine,
    DuplicateMachine,
    AbsentInference,
    DuplicateInference,
    OutOfRangeParameter,
    UnknownParameterType,
    InvalidService,
    UnknownService,
    NonBoundaryService,
    DuplicateBoundarySymbol,
}

#[test]
fn checked_synchronous_invocation_targets_reject_every_exact_drift() {
    let cases = [
        (CheckedInvocationDrift::None, None),
        (
            CheckedInvocationDrift::MissingOwner,
            Some("neither one exact boundary trait nor one exact boundary operator"),
        ),
        (
            CheckedInvocationDrift::DuplicateOwner,
            Some("resolves to 2 exact boundary traits"),
        ),
        (
            CheckedInvocationDrift::AbsentMachine,
            Some("is absent from typed machines"),
        ),
        (
            CheckedInvocationDrift::DuplicateMachine,
            Some("resolves to 2 exact typed machines"),
        ),
        (
            CheckedInvocationDrift::AbsentInference,
            Some("0 exact synchronous-invocation inference summaries"),
        ),
        (
            CheckedInvocationDrift::DuplicateInference,
            Some("2 exact synchronous-invocation inference summaries"),
        ),
        (
            CheckedInvocationDrift::OutOfRangeParameter,
            Some("no exact non-self synchronous-invocation parameter 1"),
        ),
        (
            CheckedInvocationDrift::UnknownParameterType,
            Some("resolves to 0 exact boundary traits"),
        ),
        (
            CheckedInvocationDrift::InvalidService,
            Some("invalid exact synchronous-invocation service symbol"),
        ),
        (
            CheckedInvocationDrift::UnknownService,
            Some("resolves to 0 exact boundary traits"),
        ),
        (
            CheckedInvocationDrift::NonBoundaryService,
            Some("resolves to 0 exact boundary traits"),
        ),
        (
            CheckedInvocationDrift::DuplicateBoundarySymbol,
            Some("resolves to 2 exact boundary traits"),
        ),
    ];

    for (drift, expected) in cases {
        let parameter_trait = if matches!(drift, CheckedInvocationDrift::UnknownParameterType) {
            99
        } else {
            32
        };
        let (mut typed, mut plan, mut inferred) = checked_invocation_fixture(parameter_trait, 1);
        match drift {
            CheckedInvocationDrift::None => {}
            CheckedInvocationDrift::MissingOwner => {
                plan.schema.methods[0].requirement_owner = "pkg::Missing".to_owned();
            }
            CheckedInvocationDrift::DuplicateOwner => {
                typed.push_trait_definition(boundary_trait(35, "pkg::Source"));
            }
            CheckedInvocationDrift::AbsentMachine => {
                plan.rows[0].binding = ProviderBinding::CheckedAdapter {
                    machine_identity: "Provider::missing".to_owned(),
                    machine_package_identity: None,
                };
            }
            CheckedInvocationDrift::DuplicateMachine => {
                let duplicate = typed.machines()[0].clone();
                typed.push_machine(duplicate);
            }
            CheckedInvocationDrift::AbsentInference => inferred.machines.clear(),
            CheckedInvocationDrift::DuplicateInference => {
                let duplicate = inferred.machines[0].clone();
                inferred.machines.push(duplicate);
            }
            CheckedInvocationDrift::OutOfRangeParameter => {
                inferred.machines[0].inferred_transitive =
                    vec![psi_effects::InvocationTarget::Parameter(1)];
            }
            CheckedInvocationDrift::UnknownParameterType => {}
            CheckedInvocationDrift::InvalidService => {
                inferred.machines[0].inferred_transitive =
                    vec![psi_effects::InvocationTarget::Service(
                        psi_symbols::SymbolHandle::invalid(),
                    )];
            }
            CheckedInvocationDrift::UnknownService => {
                inferred.machines[0].inferred_transitive =
                    vec![psi_effects::InvocationTarget::Service(
                        psi_symbols::SymbolHandle::from_arena_index(99),
                    )];
            }
            CheckedInvocationDrift::NonBoundaryService => {
                typed.push_trait_definition(psi_typed_trees::trait_definition::TraitDefinition {
                    symbol: psi_symbols::SymbolHandle::from_arena_index(99),
                    name: psi_typed_trees::name::Identifier::generated("pkg::Plain"),
                    ..Default::default()
                });
                inferred.machines[0].inferred_transitive =
                    vec![psi_effects::InvocationTarget::Service(
                        psi_symbols::SymbolHandle::from_arena_index(99),
                    )];
            }
            CheckedInvocationDrift::DuplicateBoundarySymbol => {
                typed.push_trait_definition(boundary_trait(32, "pkg::DuplicateTarget"));
            }
        }
        let method = &plan.schema.methods[0];
        let row = &plan.rows[0];
        let result = exact_checked_adapter_invocations(&typed, &inferred, &plan, method, row);
        match expected {
            None => assert_eq!(
                result.expect("exact checked target resolves"),
                vec!["pkg::Target".to_owned()],
            ),
            Some(expected) => assert!(
                result
                    .expect_err("checked invocation identity drift must reject")
                    .message
                    .contains(expected),
                "{drift:?}: expected `{expected}`",
            ),
        }
    }
}

#[test]
fn self_forwarding_erases_only_the_exact_schema_receiver() {
    let (typed, plan, inferred) = checked_invocation_fixture(31, 0);
    assert_eq!(
        exact_checked_adapter_invocations(
            &typed,
            &inferred,
            &plan,
            &plan.schema.methods[0],
            &plan.rows[0],
        )
        .expect("exact receiver forwarding resolves"),
        Vec::<String>::new(),
    );

    let (typed, plan, inferred) = checked_invocation_fixture(33, 0);
    assert_eq!(
        exact_checked_adapter_invocations(
            &typed,
            &inferred,
            &plan,
            &plan.schema.methods[0],
            &plan.rows[0],
        )
        .expect("same-leaf foreign receiver remains an external edge"),
        vec!["other::Source".to_owned()],
    );
}

#[test]
fn implicit_selection_never_combines_partial_candidates() {
    let plans = vec![
        selection_plan("FirstProvider", &["first", "second"], &["first"]),
        selection_plan("SecondProvider", &["first", "second"], &["second"]),
    ];
    assert_eq!(
        selected_plan_names(
            &select_provider_plans(&plans, omega_target::NativeTarget::host(), &[], &[])
                .expect("partial candidates are reportable, not ambiguous")
        ),
        Vec::<String>::new(),
        "two partial candidates are not one provider"
    );
}

#[test]
fn implicit_selection_returns_the_unique_covering_candidate() {
    let plans = vec![
        selection_plan(
            "CompleteProvider",
            &["first", "second"],
            &["first", "second"],
        ),
        selection_plan("PartialProvider", &["first", "second"], &["first"]),
    ];
    assert_eq!(
        selected_plan_names(
            &select_provider_plans(&plans, omega_target::NativeTarget::host(), &[], &[])
                .expect("one covering candidate selects")
        ),
        vec!["CompleteProvider".to_owned()]
    );
}

#[test]
fn external_root_bridge_requires_one_exact_retained_boundary_slot() {
    let mut first = selection_plan("FirstProvider", &["run"], &["run"]);
    first.schema.trait_name = "first::Pair".into();
    let mut second = selection_plan("SecondProvider", &["run"], &["run"]);
    second.schema.trait_name = "second::Pair".into();
    let facts = omega_effects::SelectedProviderPlanFacts::from_selection(
        &[first.clone(), second],
        &["FirstProvider".into(), "SecondProvider".into()],
    )
    .expect("distinct qualified boundary slots may both be selected");
    assert_eq!(
        selected_external_root_provider_plan_id(&facts, "first::Pair")
            .expect("qualified slot resolves")
            .normalized_identity(),
        first.identity_fingerprint()
    );
    assert!(
        selected_external_root_provider_plan_id(&facts, "Pair")
            .expect_err("an ambiguous leaf slot must reject")
            .0
            .contains("matches 2 retained selected provider plans")
    );
}

#[test]
fn granted_selected_plan_attaches_receipt_by_exact_inherited_requirement() {
    let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
    let subject_symbol = psi_symbols::SymbolHandle::from_arena_index(8);
    let domain_symbol = psi_symbols::SymbolHandle::from_arena_index(9);
    let requirement_symbol = psi_symbols::SymbolHandle::from_arena_index(10);
    let mut checked = psi_checked_trees::CheckedTrees::default();
    let requirement_identity = push_boundary_requirement(
        &mut checked,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    let fact = append_admitted_fact(
        &mut checked,
        subject_symbol,
        domain_symbol,
        owner_symbol,
        requirement_symbol,
    );
    let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
    set_exact_requirement(
        &mut selected,
        "PairChild",
        "PairBase",
        &requirement_identity,
    );
    let identity = selected.identity_fingerprint();

    bind_selected_provider_plan_facts(
        &mut checked,
        std::slice::from_ref(&selected),
        omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &["FirstProvider".to_owned()],
        )
        .expect("canonical selected facts"),
        &["PairChild".to_owned()],
    )
    .expect("exact inherited requirement binds the selected child-schema plan");

    assert_eq!(
        checked
            .facts
            .semantic
            .facts
            .get(fact)
            .evidence
            .receipt_identity,
        identity
    );
}

#[test]
fn granted_selected_plan_does_not_stamp_a_different_exact_requirement() {
    let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
    let selected_requirement = psi_symbols::SymbolHandle::from_arena_index(10);
    let evidence_requirement = psi_symbols::SymbolHandle::from_arena_index(11);
    let mut checked = psi_checked_trees::CheckedTrees::default();
    let mut owner = psi_typed_trees::trait_definition::TraitDefinition {
        symbol: owner_symbol,
        is_boundary: true,
        name: psi_typed_trees::name::Identifier::generated("PairBase"),
        ..Default::default()
    };
    for (symbol, name) in [
        (selected_requirement, "first"),
        (evidence_requirement, "second"),
    ] {
        checked.typed.push_trait_machine_signature(
            &mut owner,
            psi_typed_trees::signature::StateSignature {
                symbol,
                name: psi_typed_trees::name::Identifier::generated(name),
                ..Default::default()
            },
        );
    }
    checked.typed.push_trait_definition(owner);
    let owner = checked
        .typed
        .traits()
        .iter()
        .find(|definition| definition.symbol == owner_symbol)
        .expect("inserted boundary owner");
    let requirement = checked
        .typed
        .trait_machine_signatures(owner)
        .iter()
        .find(|requirement| requirement.symbol == selected_requirement)
        .expect("selected boundary requirement");
    let requirement_identity = checked
        .typed
        .normalized_trait_requirement_overload_identity(owner, requirement)
        .identity();
    let fact = append_admitted_fact(
        &mut checked,
        psi_symbols::SymbolHandle::from_arena_index(8),
        psi_symbols::SymbolHandle::from_arena_index(9),
        owner_symbol,
        evidence_requirement,
    );
    let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
    set_exact_requirement(
        &mut selected,
        "PairChild",
        "PairBase",
        &requirement_identity,
    );

    bind_selected_provider_plan_facts(
        &mut checked,
        std::slice::from_ref(&selected),
        omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &["FirstProvider".to_owned()],
        )
        .expect("canonical selected facts"),
        &["PairChild".to_owned()],
    )
    .expect("a different exact requirement is simply not stamped");

    assert_eq!(
        checked
            .facts
            .semantic
            .facts
            .get(fact)
            .evidence
            .receipt_identity,
        0
    );
}

#[test]
fn admitted_receipt_rejects_a_requirement_outside_its_exact_owner() {
    let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
    let requirement_symbol = psi_symbols::SymbolHandle::from_arena_index(10);
    let mut checked = psi_checked_trees::CheckedTrees::default();
    let requirement_identity = push_boundary_requirement(
        &mut checked,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    append_admitted_fact(
        &mut checked,
        psi_symbols::SymbolHandle::from_arena_index(8),
        psi_symbols::SymbolHandle::from_arena_index(9),
        owner_symbol,
        psi_symbols::SymbolHandle::from_arena_index(11),
    );
    let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
    set_exact_requirement(
        &mut selected,
        "PairChild",
        "PairBase",
        &requirement_identity,
    );

    let diagnostics = bind_selected_provider_plan_facts(
        &mut checked,
        std::slice::from_ref(&selected),
        omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &["FirstProvider".to_owned()],
        )
        .expect("canonical selected facts"),
        &["PairChild".to_owned()],
    )
    .expect_err("an admitted requirement outside the exact owner must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resolves to 0 exact typed signatures")
    }));
}

#[test]
fn admitted_receipt_owner_and_signature_custody_is_exact_and_atomic() {
    let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
    let requirement_symbol = psi_symbols::SymbolHandle::from_arena_index(10);
    let mut checked = psi_checked_trees::CheckedTrees::default();
    let requirement_identity = push_boundary_requirement(
        &mut checked,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    let valid = append_admitted_fact(
        &mut checked,
        psi_symbols::SymbolHandle::from_arena_index(8),
        psi_symbols::SymbolHandle::from_arena_index(9),
        owner_symbol,
        requirement_symbol,
    );
    append_admitted_fact(
        &mut checked,
        psi_symbols::SymbolHandle::from_arena_index(11),
        psi_symbols::SymbolHandle::from_arena_index(12),
        owner_symbol,
        psi_symbols::SymbolHandle::from_arena_index(90),
    );
    let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
    set_exact_requirement(
        &mut selected,
        "PairChild",
        "PairBase",
        &requirement_identity,
    );

    let diagnostics = bind_selected_provider_plan_facts(
        &mut checked,
        std::slice::from_ref(&selected),
        omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &[selected.name.clone()],
        )
        .expect("selected provider"),
        &[selected.schema.trait_name.clone()],
    )
    .expect_err("late missing signature must reject every staged receipt");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resolves to 0 exact typed signatures")
    }));
    assert_eq!(
        checked
            .facts
            .semantic
            .facts
            .get(valid)
            .evidence
            .receipt_identity,
        0,
        "late failure must not publish an earlier valid receipt",
    );

    let mut duplicate_owner = checked.clone();
    duplicate_owner.typed.push_trait_definition(
        psi_typed_trees::trait_definition::TraitDefinition {
            symbol: owner_symbol,
            is_boundary: true,
            name: psi_typed_trees::name::Identifier::generated("DuplicatePairBase"),
            ..Default::default()
        },
    );
    let diagnostics = bind_selected_provider_plan_facts(
        &mut duplicate_owner,
        std::slice::from_ref(&selected),
        omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &[selected.name.clone()],
        )
        .expect("selected provider"),
        &[selected.schema.trait_name.clone()],
    )
    .expect_err("duplicate exact owner must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resolves to 2 exact typed boundary requirement owners")
    }));

    let other_owner = psi_symbols::SymbolHandle::from_arena_index(20);
    let other_requirement = psi_symbols::SymbolHandle::from_arena_index(21);
    let mut cross_owned = psi_checked_trees::CheckedTrees::default();
    push_boundary_requirement(
        &mut cross_owned,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    push_boundary_requirement(
        &mut cross_owned,
        other_owner,
        "OtherBase",
        other_requirement,
        "other",
    );
    append_admitted_fact(
        &mut cross_owned,
        psi_symbols::SymbolHandle::from_arena_index(22),
        psi_symbols::SymbolHandle::from_arena_index(23),
        owner_symbol,
        other_requirement,
    );
    let diagnostics = bind_selected_provider_plan_facts(
        &mut cross_owned,
        std::slice::from_ref(&selected),
        omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &[selected.name.clone()],
        )
        .expect("selected provider"),
        &[selected.schema.trait_name.clone()],
    )
    .expect_err("cross-owned exact signature must reject");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("belongs to exact trait") })
    );

    let mut duplicate_signature = psi_checked_trees::CheckedTrees::default();
    push_boundary_requirement(
        &mut duplicate_signature,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    push_boundary_requirement(
        &mut duplicate_signature,
        other_owner,
        "OtherBase",
        requirement_symbol,
        "duplicate",
    );
    append_admitted_fact(
        &mut duplicate_signature,
        psi_symbols::SymbolHandle::from_arena_index(24),
        psi_symbols::SymbolHandle::from_arena_index(25),
        owner_symbol,
        requirement_symbol,
    );
    let diagnostics = bind_selected_provider_plan_facts(
        &mut duplicate_signature,
        std::slice::from_ref(&selected),
        omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &[selected.name.clone()],
        )
        .expect("selected provider"),
        &[selected.schema.trait_name.clone()],
    )
    .expect_err("duplicate exact signature must reject");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("resolves to 2 exact typed signatures")
    }));
}

#[test]
fn admitted_receipt_rejects_duplicate_exact_granted_plan_matches() {
    let owner_symbol = psi_symbols::SymbolHandle::from_arena_index(7);
    let requirement_symbol = psi_symbols::SymbolHandle::from_arena_index(10);
    let mut checked = psi_checked_trees::CheckedTrees::default();
    let requirement_identity = push_boundary_requirement(
        &mut checked,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    append_admitted_fact(
        &mut checked,
        psi_symbols::SymbolHandle::from_arena_index(8),
        psi_symbols::SymbolHandle::from_arena_index(9),
        owner_symbol,
        requirement_symbol,
    );
    let mut first = selection_plan("FirstProvider", &["first"], &["first"]);
    set_exact_requirement(&mut first, "PairChildA", "PairBase", &requirement_identity);
    let mut second = selection_plan("SecondProvider", &["first"], &["first"]);
    set_exact_requirement(&mut second, "PairChildB", "PairBase", &requirement_identity);

    let diagnostics = bind_selected_provider_plan_facts(
        &mut checked,
        &[first.clone(), second.clone()],
        omega_effects::SelectedProviderPlanFacts::from_selection(
            &[first, second],
            &["FirstProvider".to_owned(), "SecondProvider".to_owned()],
        )
        .expect("distinct selected slots may retain duplicate requirement identities"),
        &["PairChildA".to_owned(), "PairChildB".to_owned()],
    )
    .expect_err("two granted exact matches must reject rather than choose by order");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("matches 2 granted selected provider plans")
    }));
}

#[test]
fn explicit_selection_resolves_covering_ambiguity_by_provider_type() {
    let plans = vec![
        selection_plan("FirstProvider", &["first"], &["first"]),
        selection_plan("SecondProvider", &["first"], &["first"]),
    ];
    let selected = select_provider_plans(
        &plans,
        omega_target::NativeTarget::host(),
        &[],
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pair",
                "SecondProvider",
            ),
        ],
    )
    .expect("the build root owns the slot choice");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["SecondProvider".to_owned()]
    );
}

#[test]
fn same_spelled_package_slots_and_providers_remain_distinct() {
    let first_package =
        psi_core::PackageKeyIdentity::from_digest([0x61; 32]).expect("nonzero package identity");
    let second_package =
        psi_core::PackageKeyIdentity::from_digest([0x62; 32]).expect("nonzero package identity");

    let mut first = selection_plan("first-plan", &["choose"], &["choose"]);
    first.schema.trait_name = "Shared".to_owned();
    first.schema.trait_package_identity = Some(first_package);
    first.provider_type = "Provider".to_owned();
    first.provider_type_package_identity = Some(first_package);

    let mut second = selection_plan("second-plan", &["choose"], &["choose"]);
    second.schema.trait_name = "Shared".to_owned();
    second.schema.trait_package_identity = Some(second_package);
    second.provider_type = "Provider".to_owned();
    second.provider_type_package_identity = Some(second_package);

    let plans = [first, second];
    let automatic = select_provider_plans(&plans, omega_target::NativeTarget::host(), &[], &[])
        .expect("each exact slot has one covering provider");
    assert_eq!(
        selected_plan_names(&automatic),
        vec!["first-plan".to_owned(), "second-plan".to_owned()]
    );

    let selected = select_provider_plans(
        &plans,
        omega_target::NativeTarget::host(),
        &[
            package_selection("Shared", first_package, "Provider", first_package),
            package_selection("Shared", second_package, "Provider", second_package),
        ],
        &[],
    )
    .expect("same readable paths in distinct packages select their exact plans");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["first-plan".to_owned(), "second-plan".to_owned()]
    );
}

#[test]
fn provider_from_another_package_cannot_satisfy_the_selected_slot() {
    let boundary_package =
        psi_core::PackageKeyIdentity::from_digest([0x71; 32]).expect("nonzero package identity");
    let provider_package =
        psi_core::PackageKeyIdentity::from_digest([0x72; 32]).expect("nonzero package identity");
    let mut plan = selection_plan("provider-plan", &["choose"], &["choose"]);
    plan.schema.trait_package_identity = Some(boundary_package);
    plan.provider_type = "Provider".to_owned();
    plan.provider_type_package_identity = Some(boundary_package);

    let diagnostics = select_provider_plans(
        &[plan],
        omega_target::NativeTarget::host(),
        &[],
        &[package_selection(
            "Pair",
            boundary_package,
            "Provider",
            provider_package,
        )],
    )
    .expect_err("same provider spelling from another package is not the selected identity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("no candidate exists in the loaded dependency closure")
    }));
}

#[test]
fn selection_does_not_fall_back_to_a_boundary_slot_leaf() {
    let mut first = selection_plan("FirstProvider", &["choose"], &["choose"]);
    first.schema.trait_name = "first::Pick".to_owned();
    let mut second = selection_plan("SecondProvider", &["choose"], &["choose"]);
    second.schema.trait_name = "second::Pick".to_owned();

    let diagnostics = select_provider_plans(
        &[first, second],
        omega_target::NativeTarget::host(),
        &[],
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pick",
                "FirstProvider",
            ),
        ],
    )
    .expect_err("a readable leaf is not a boundary identity");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("unknown boundary slot `Pick`") }),
        "expected an exact-identity diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn exact_slot_identity_does_not_select_a_qualified_same_leaf_slot() {
    let exact = selection_plan("ExactProvider", &["choose"], &["choose"]);
    let mut qualified = selection_plan("QualifiedProvider", &["choose"], &[]);
    qualified.schema.trait_name = "package::Pair".to_owned();

    let selected = select_provider_plans(
        &[exact, qualified],
        omega_target::NativeTarget::host(),
        &[],
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pair",
                "ExactProvider",
            ),
        ],
    )
    .expect("only the exact canonical slot participates in the declaration");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["ExactProvider".to_owned()]
    );
}

#[test]
fn exact_provider_identity_does_not_select_a_qualified_same_leaf_provider() {
    let exact = selection_plan("exact-plan", &["choose"], &["choose"]);
    let mut qualified = selection_plan("qualified-plan", &["choose"], &["choose"]);
    qualified.provider_type = "package::exact-plan".to_owned();

    let selected = select_provider_plans(
        &[exact, qualified],
        omega_target::NativeTarget::host(),
        &[],
        &[crate::pipeline::build_config::ProviderSelection::exact_for_test("Pair", "exact-plan")],
    )
    .expect("only the exact canonical provider identity matches");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["exact-plan".to_owned()]
    );
}

#[test]
fn canonical_slot_resolution_catches_duplicate_selection_spellings() {
    let mut plan = selection_plan("FirstProvider", &["choose"], &["choose"]);
    plan.schema.trait_name = "package::Pick".to_owned();

    let diagnostics = select_provider_plans(
        &[plan],
        omega_target::NativeTarget::host(),
        &[],
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "package::Pick",
                "FirstProvider",
            ),
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "package::Pick",
                "SecondProvider",
            ),
        ],
    )
    .expect_err("one canonical slot cannot be selected twice through aliases");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("provider selection for slot `package::Pick` more than once")),
        "expected canonical duplicate-slot diagnostic, got {diagnostics:#?}"
    );
}

#[test]
fn target_default_does_not_fall_back_to_a_boundary_slot_leaf() {
    let mut first = selection_plan("FirstProvider", &["choose"], &["choose"]);
    first.schema.trait_name = "first::Pick".to_owned();
    let mut second = selection_plan("SecondProvider", &["choose"], &["choose"]);
    second.schema.trait_name = "second::Pick".to_owned();

    let diagnostics = select_provider_plans(
        &[first, second],
        omega_target::NativeTarget::host(),
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pick",
                "FirstProvider",
            ),
        ],
        &[],
    )
    .expect_err("a target default must name one canonical slot");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(
                "target package selects provider `FirstProvider` for unknown boundary slot `Pick`"
            ))
    );
}

#[test]
fn explicit_selection_refuses_partial_provider() {
    let plans = vec![selection_plan(
        "PartialProvider",
        &["first", "second"],
        &["first"],
    )];
    let diagnostics = select_provider_plans(
        &plans,
        omega_target::NativeTarget::host(),
        &[],
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pair",
                "PartialProvider",
            ),
        ],
    )
    .expect_err("selection never manufactures missing rows");
    assert!(diagnostics[0].message.contains("is partial"));
}

#[test]
fn target_default_resolves_covering_ambiguity() {
    let plans = vec![
        selection_plan("FirstProvider", &["first"], &["first"]),
        selection_plan("SecondProvider", &["first"], &["first"]),
    ];
    let selected = select_provider_plans(
        &plans,
        omega_target::NativeTarget::host(),
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pair",
                "FirstProvider",
            ),
        ],
        &[],
    )
    .expect("the selected target package supplies the slot default");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["FirstProvider".to_owned()]
    );
}

#[test]
fn duplicate_exact_target_defaults_do_not_conflict() {
    let mut plan = selection_plan("package-provider", &["first"], &["first"]);
    plan.provider_type = "package::FirstProvider".to_owned();
    let defaults = [
        crate::pipeline::build_config::ProviderSelection::exact_for_test(
            "Pair",
            "package::FirstProvider",
        ),
        crate::pipeline::build_config::ProviderSelection::exact_for_test(
            "Pair",
            "package::FirstProvider",
        ),
    ];
    let selected = select_provider_plans(
        &[plan.clone()],
        omega_target::NativeTarget::host(),
        &defaults,
        &[],
    )
    .expect("duplicate declarations of one exact provider identity are one target default");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["package-provider".to_owned()]
    );
    let selected = select_provider_plans_with_provenance(
        &[DerivedProviderPlan {
            plan,
            provenance: ProviderPlanProvenance {
                schema: ProviderSchemaDeclaration::BoundaryTrait(
                    psi_symbols::SymbolHandle::invalid(),
                ),
                provider_type: None,
                row_requirements: Vec::new(),
                row_realizations: Vec::new(),
            },
        }],
        omega_target::NativeTarget::host(),
        &defaults,
        &[],
    )
    .expect("selection carries every accepted duplicate default site");
    let [selected] = selected.as_slice() else {
        panic!("one selected provider")
    };
    let ProviderSelectionProvenance::TargetDefault(declarations) = &selected.selected_by else {
        panic!("target-default provenance")
    };
    assert_eq!(declarations.len(), 2);
}

#[test]
fn build_override_wins_over_target_default() {
    let plans = vec![
        selection_plan("FirstProvider", &["first"], &["first"]),
        selection_plan("SecondProvider", &["first"], &["first"]),
    ];
    let selected = select_provider_plans(
        &plans,
        omega_target::NativeTarget::host(),
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pair",
                "FirstProvider",
            ),
        ],
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pair",
                "SecondProvider",
            ),
        ],
    )
    .expect("the build root owns the final slot choice");
    assert_eq!(
        selected_plan_names(&selected),
        vec!["SecondProvider".to_owned()]
    );
}

#[test]
fn conflicting_target_defaults_are_loud() {
    let plans = vec![
        selection_plan("FirstProvider", &["first"], &["first"]),
        selection_plan("SecondProvider", &["first"], &["first"]),
    ];
    let diagnostics = select_provider_plans(
        &plans,
        omega_target::NativeTarget::host(),
        &[
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pair",
                "FirstProvider",
            ),
            crate::pipeline::build_config::ProviderSelection::exact_for_test(
                "Pair",
                "SecondProvider",
            ),
        ],
        &[],
    )
    .expect_err("a target has one default provider per slot");
    assert!(
        diagnostics[0]
            .message
            .contains("conflicting target-package defaults")
    );
}

#[test]
fn table_field_leaf_requires_an_attached_layout_owner() {
    let mut plan = selection_plan("field-leaf", &["first"], &[]);
    plan.provider_type.clear();
    plan.rows.push(ProviderPlanRow {
        method: "first".to_owned(),
        requirement_identity: "Pair::first".to_owned(),
        binding: ProviderBinding::VtableField {
            table: String::new(),
            field: "first".to_owned(),
        },
    });

    let diagnostics = validate_provider_plan_candidates(&TypedTrees::default(), &[plan]);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("without an attached provider data type")
    );
}

#[test]
fn checked_adapter_requires_a_nominal_provider_type() {
    let mut plan = selection_plan("free-adapter", &["first"], &[]);
    plan.provider_type.clear();
    plan.rows.push(ProviderPlanRow {
        method: "first".to_owned(),
        requirement_identity: "Pair::first".to_owned(),
        binding: ProviderBinding::CheckedAdapter {
            machine_identity: "first_adapter".to_owned(),
            machine_package_identity: None,
        },
    });

    let diagnostics = validate_provider_plan_candidates(&TypedTrees::default(), &[plan]);

    // Candidate-shape and typed-resolution validation remain cumulative:
    // the impossible free adapter has neither a nominal owner nor a typed
    // machine that could supply one.
    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no nominal provider type"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent from typed machines"))
    );
}

#[test]
fn checked_adapter_must_resolve_to_its_exact_checked_provider_conformance() {
    let source = r#"
        boundary trait Readable {
            machine read() -> i32;
        }

        boundary trait OtherBoundary {
            machine other() -> i32;
        }

        data Provider {}
        data OtherProvider {}

        machine Provider::read() -> i32 satisfies Readable::read { 1 }
        machine Provider::helper() -> i32 { 2 }
        machine OtherProvider::helper() -> i32 { 3 }
        machine Provider::external() -> i32
        satisfies OtherBoundary::other
        via Binding::CompilerIntrinsic;
    "#;
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize adapter ownership fixture");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse adapter ownership fixture");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve adapter ownership fixture");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type adapter ownership fixture");
    let plan = derive_satisfies_plans(&typed, None)
        .into_iter()
        .find(|plan| plan.schema.trait_name == "Readable")
        .expect("Readable provider plan");
    assert!(
        validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty(),
        "the exact checked provider conformance remains valid"
    );

    let mut absent = plan.clone();
    absent.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: "Provider::absent".to_owned(),
        machine_package_identity: None,
    };
    assert!(
        validate_provider_plan_candidates(&typed, &[absent])
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent from typed machines"))
    );

    let mut wrong_provider = plan.clone();
    wrong_provider.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: normalized_machine_identity(&typed, "OtherProvider::helper"),
        machine_package_identity: None,
    };
    assert!(
        validate_provider_plan_candidates(&typed, &[wrong_provider])
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("belongs to provider `OtherProvider`, not selected provider `Provider`"))
    );

    let mut external = plan.clone();
    external.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: normalized_machine_identity(&typed, "Provider::external"),
        machine_package_identity: None,
    };
    assert!(
        validate_provider_plan_candidates(&typed, &[external])
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("does not name a checked body with an entry state"))
    );

    let mut unrelated = plan;
    unrelated.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: normalized_machine_identity(&typed, "Provider::helper"),
        machine_package_identity: None,
    };
    assert!(
        validate_provider_plan_candidates(&typed, &[unrelated])
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("has no exact checked satisfies edge for requirement identity"))
    );
}

#[test]
fn checked_operator_adapter_must_resolve_to_its_exact_operator_conformance() {
    let source = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32;

        data OtherMath {}
        boundary operator OtherMath::offset_zero(value: i32) -> i32;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        {
            transition { _ -> (input) }
        }
        machine CheckedMathProvider::decoy_impl(input: i32) -> i32
        satisfies OtherMath::offset_zero
        {
            transition { _ -> (input) }
        }
        machine CheckedMathProvider::wrong_signature(input: u64) -> u64
        satisfies CheckedMath::offset_zero
        {
            transition { _ -> (input) }
        }
    "#;
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize checked operator adapter fixture");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse checked operator adapter fixture");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve checked operator adapter fixture");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type checked operator adapter fixture");
    let operator = typed
        .operators()
        .iter()
        .find(|operator| {
            typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .eq(["CheckedMath", "offset_zero"])
        })
        .expect("CheckedMath::offset_zero operator");
    let identity =
        psi_typed_trees::operator::boundary_operator_requirement_identity(&typed, operator);
    let plan = derive_satisfies_plans(&typed, None)
        .into_iter()
        .find(|plan| plan.schema.trait_name == identity)
        .expect("CheckedMath::offset_zero provider plan");
    assert!(
        validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty(),
        "the exact checked operator conformance remains valid"
    );

    for unrelated in [
        "CheckedMathProvider::decoy_impl",
        "CheckedMathProvider::wrong_signature",
    ] {
        let mut invalid = plan.clone();
        invalid.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine_identity: normalized_machine_identity(&typed, unrelated),
            machine_package_identity: None,
        };
        assert!(
            validate_provider_plan_candidates(&typed, &[invalid])
                .iter()
                .any(|diagnostic| diagnostic
                    .message
                    .contains("has no exact checked satisfies edge for requirement identity")),
            "operator adapter `{unrelated}` must not satisfy the exact operator row"
        );
    }
}

#[test]
fn syscall_derivation_retains_exact_number_before_range_validation() {
    fn derive(number: i64) -> (TypedTrees, Vec<omega_effects::provider_plan::ProviderPlan>) {
        let source = format!(
            r#"
                boundary trait Process {{
                    machine exit(code: i32);
                }}

                machine exit_leaf(code: i32)
                satisfies Process::exit
                via Binding::Syscall({number});
            "#
        );
        let tokens = psi_source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokenize syscall leaf");
        let syntax =
            psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse syscall leaf");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve syscall leaf");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type syscall leaf");
        let plans = derive_satisfies_plans(&typed, None);
        (typed, plans)
    }

    let maximum = i64::from(u32::MAX);
    let (typed, plans) = derive(maximum);
    let ProviderBinding::Syscall { number } = &plans[0].rows[0].binding else {
        panic!("source syscall leaf must retain a syscall binding");
    };
    assert_eq!(*number, maximum);
    assert!(validate_provider_plan_candidates(&typed, &plans).is_empty());

    let oversized = maximum + 1;
    let (typed, plans) = derive(oversized);
    let ProviderBinding::Syscall { number } = &plans[0].rows[0].binding else {
        panic!("source syscall leaf must retain a syscall binding");
    };
    assert_eq!(*number, oversized);
    assert_ne!(*number, 0, "oversized syscall must not normalize to zero");
    let diagnostics = validate_provider_plan_candidates(&typed, &plans);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("target syscall plan requires a value in 0..=4294967295")
    }));
}

#[test]
fn checked_adapter_rejects_symbol_resolved_service_widening() {
    let source = r#"
        boundary trait Queryable {
            machine query();
        }

        boundary trait Readable {
            machine read(queryable: &mut Queryable);
        }

        data Provider {}

        machine Provider::read(queryable: &mut Queryable)
        satisfies Readable::read {
            queryable.query();
        }
    "#;
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse provider");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve provider");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type provider");
    let plans = derive_satisfies_plans(&typed, None);

    let diagnostics = validate_provider_plan_candidates(&typed, &plans);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("boundary service(s) [Queryable]")
            && diagnostic
                .message
                .contains("declared service ceiling [Readable]")
    }));
}

#[test]
fn provider_candidate_requires_exact_canonical_typed_schema() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        Method,
        RequirementOwner,
        RequirementIdentity,
        ParameterShape,
        EntryClaim,
        ResultShape,
        ResultClaim,
        ServiceReach,
        SynchronousInvocation,
        Suspension,
        Blocking,
        Termination,
        CallingPlan,
    }

    let source = r#"
        boundary trait Readable {
            machine read();
        }

        data Provider {}

        machine Provider::read()
        satisfies Readable::read {}
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    assert!(validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty());

    for drift in [
        Drift::Method,
        Drift::RequirementOwner,
        Drift::RequirementIdentity,
        Drift::ParameterShape,
        Drift::EntryClaim,
        Drift::ResultShape,
        Drift::ResultClaim,
        Drift::ServiceReach,
        Drift::SynchronousInvocation,
        Drift::Suspension,
        Drift::Blocking,
        Drift::Termination,
        Drift::CallingPlan,
    ] {
        let mut drifted = plan.clone();
        let method = &mut drifted.schema.methods[0];
        match drift {
            Drift::Method => {
                method.name = "other".to_owned();
                drifted.rows[0].method = method.name.clone();
            }
            Drift::RequirementOwner => method.requirement_owner = "Other".to_owned(),
            Drift::RequirementIdentity => {
                method.requirement_identity = "Other::read()".to_owned();
                drifted.rows[0].requirement_identity = method.requirement_identity.clone();
            }
            Drift::ParameterShape => {
                method.parameter_count = 1;
                method.parameter_type_identities = vec!["i32".to_owned()];
            }
            Drift::EntryClaim => {
                method.parameter_count = 1;
                method.parameter_type_identities = vec!["i32 in Accepted".to_owned()];
                method.entry_claims = vec![omega_effects::provider_plan::ServiceEntryClaim {
                    parameter_index: 0,
                    carrier_identity: "named(name(Token))".to_owned(),
                    domain: "Accepted".to_owned(),
                    predicate_body: psi_language_semantics::DomainPredicateBody::Bodyless,
                    effective_carry: psi_language_semantics::CarryPolicy::STRICT,
                    authority_flow:
                        omega_effects::provider_plan::ServiceEntryAuthorityFlow::Accepts,
                }];
            }
            Drift::ResultShape => {
                method.has_result = true;
                method.result_type_identity = Some("i32".to_owned());
            }
            Drift::ResultClaim => {
                method.has_result = true;
                method.result_type_identity = Some("i32 in Returned".to_owned());
                method.result_claims = vec![omega_effects::provider_plan::ServiceResultClaim {
                    domain: "Returned".to_owned(),
                    effective_carry: psi_language_semantics::CarryPolicy::STRICT,
                }];
            }
            Drift::ServiceReach => {
                method.service_reach.push("Writable".to_owned());
                method.service_reach.sort_unstable();
            }
            Drift::SynchronousInvocation => {
                method.synchronous_invocations.push("Writable".to_owned())
            }
            Drift::Suspension => method.may_suspend = true,
            Drift::Blocking => method.may_block = true,
            Drift::Termination => method.terminates_guarantee = true,
            Drift::CallingPlan => method.calling_plan_fingerprint = Some(1),
        }

        let diagnostics = validate_provider_plan_candidates(&typed, &[drifted]);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("does not equal its exact canonical typed schema")),
            "{drift:?} must fail canonical typed schema custody: {diagnostics:?}",
        );
    }
}

#[test]
fn forged_service_ceiling_cannot_launder_checked_adapter_reach() {
    let source = r#"
        boundary trait Queryable {
            machine query();
        }

        boundary trait Readable {
            machine read(queryable: &mut Queryable);
        }

        data Provider {}

        machine Provider::read(queryable: &mut Queryable)
        satisfies Readable::read {
            queryable.query();
        }
    "#;
    let (typed, mut plan) = derive_provider_fixture(source);
    plan.schema.methods[0]
        .service_reach
        .push("Queryable".to_owned());
    plan.schema.methods[0].service_reach.sort_unstable();
    plan.schema.methods[0].service_reach.dedup();

    let diagnostics = validate_provider_plan_candidates(&typed, &[plan]);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not equal its exact canonical typed schema")
    }));
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("boundary service(s) [Queryable] outside")
    }));
}

#[test]
fn canonical_schema_resolution_is_exact_and_unique() {
    let source = r#"
        boundary trait Readable {
            machine read();
        }

        boundary trait Unrelated {
            machine inspect();
        }

        data Provider {}

        machine Provider::read()
        satisfies Readable::read {}
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    assert_eq!(
        exact_canonical_provider_schema(&typed, &plan).expect("exact schema"),
        plan.schema,
    );

    for identity in ["Missing", "pkg::Readable"] {
        let mut drifted = plan.clone();
        drifted.schema.trait_name = identity.to_owned();
        let diagnostic = exact_canonical_provider_schema(&typed, &drifted)
            .expect_err("unknown and qualified-leaf impostor schemas must reject");
        assert!(diagnostic.message.contains("resolves to 0 canonical typed"));
    }

    let mut duplicated = typed.clone();
    let duplicate = duplicated
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Readable")
        .expect("Readable trait")
        .clone();
    duplicated.push_trait_definition(duplicate);
    let diagnostic = exact_canonical_provider_schema(&duplicated, &plan)
        .expect_err("duplicate exact schema authority must reject");
    assert!(
        diagnostic
            .message
            .contains("resolves to 2 canonical typed boundary traits")
    );
}

#[test]
fn canonical_schema_accepts_exact_inherited_requirement() {
    let source = r#"
        boundary trait Parent {
            machine read();
        }

        boundary trait Child {
            requires Parent;
        }

        data Provider {}

        ProviderChild: Provider satisfies Child;

        machine Provider::read()
        satisfies Parent::read {}
    "#;
    let (typed, mut plan) = derive_provider_fixture(source);
    let child = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Child")
        .expect("Child boundary schema");
    plan.schema = ServiceSchema::from_typed(&typed, child).expect("typed child schema");

    assert!(
        validate_provider_plan_candidates(&typed, &[plan]).is_empty(),
        "an exact child schema may retain its inherited parent requirement",
    );
}

#[test]
fn canonical_schema_rejects_duplicate_exact_carrier_arguments() {
    let source = r#"
        boundary trait Readable {
            machine read(&mut self);
        }

        data Provider {}

        ProviderReadable: Provider satisfies Readable;

        machine Provider::read(&mut self)
        satisfies Readable::read {}
    "#;
    let (mut typed, plan) = derive_provider_fixture(source);
    assert_eq!(typed.conformances().len(), 1);
    let duplicate = typed.conformances()[0].clone();
    typed.push_conformance(duplicate);

    let diagnostic = exact_canonical_provider_schema(&typed, &plan)
        .expect_err("duplicate carrier argument custody must reject");
    assert!(
        diagnostic
            .message
            .contains("resolves to 2 exact carrier argument rows")
    );
}
