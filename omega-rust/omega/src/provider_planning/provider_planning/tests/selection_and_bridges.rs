use super::{
    CheckedInvocationDrift, SelectedInvocationDrift, append_admitted_fact,
    bind_selected_provider_plan_facts_for_test, boundary_trait, checked_invocation_fixture,
    push_boundary_requirement, selected_plan_names, selection_plan, set_exact_requirement,
};
use crate::provider_planning::provider_planning::{
    Arc, ProviderBinding, ProviderGrantSelectorKind, TypedTrees, bind_selected_provider_plan_facts,
    exact_checked_adapter_invocations, resolve_selected_provider_grants, select_provider_plans,
    validate_selected_synchronous_invocation_cycles,
};
#[cfg(feature = "installed-writer")]
use crate::provider_planning::provider_planning::{
    selected_external_root_provider_plan, selected_external_root_provider_plan_id,
};

#[test]
fn selected_provider_provenance_retains_owner_controlled_composition_mode() {
    let fused =
        crate::provider_planning::ProviderSelection::exact_for_test("ClockHost", "MonotonicClock");
    assert_eq!(
        crate::provider_planning::provider_planning::selection_provenance::ProviderSelectionProvenance::BuildOverride(
            vec![fused]
        )
        .composition_mode()
        .expect("omitted mode remains fused"),
        crate::provider_planning::CompositionMode::Fused,
    );

    let mut independent =
        crate::provider_planning::ProviderSelection::exact_for_test("ClockHost", "MonotonicClock");
    independent.composition_mode = crate::provider_planning::CompositionMode::Independent;
    assert_eq!(
        crate::provider_planning::provider_planning::selection_provenance::ProviderSelectionProvenance::BuildOverride(
            vec![independent.clone()]
        )
        .composition_mode()
        .expect("root build owns the independent request"),
        crate::provider_planning::CompositionMode::Independent,
    );
    assert!(
        crate::provider_planning::provider_planning::selection_provenance::ProviderSelectionProvenance::TargetDefault(
            vec![independent]
        )
        .composition_mode()
        .expect_err("target defaults cannot componentize themselves")
        .contains("owner-controlled build")
    );
    assert_eq!(
        crate::provider_planning::provider_planning::selection_provenance::ProviderSelectionProvenance::UniqueCoveringCandidate
            .composition_mode()
            .expect("automatic selection remains fused"),
        crate::provider_planning::CompositionMode::Fused,
    );
}

#[test]
fn provider_grant_ledger_resolves_one_exact_selector_subject() {
    let first = selection_plan("FirstProvider", &["first"], &["first"]);
    let candidates = vec![first.clone()];
    let selected = abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selection(
        &candidates,
        std::slice::from_ref(&first.name),
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
    assert!(grants.iter().all(|grant| {
        grant.selected_plan_report_identity == first.report_fingerprint()
            && grant.selected_plan_digest == first.identity_digest()
            && grant.selected_plan == first
            && grant.replays_selected_plan(&first)
    }));

    let mut compact_equal_substitute = first.clone();
    compact_equal_substitute.schema.methods[0].requirement_owner = "OtherPair".to_owned();
    assert_eq!(
        compact_equal_substitute.report_fingerprint(),
        first.report_fingerprint(),
        "the compact report identity omits the exact requirement-owner spelling"
    );
    assert_ne!(compact_equal_substitute, first);
    assert_ne!(
        compact_equal_substitute.identity_digest(),
        first.identity_digest()
    );
    assert!(
        grants
            .iter()
            .all(|grant| !grant.replays_selected_plan(&compact_equal_substitute))
    );

    let mut same_subject = first.clone();
    same_subject.name = same_subject.schema.trait_name.clone();
    let selected = abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selection(
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
fn provider_grant_binding_rejects_compact_equal_selected_plan_substitution() {
    let candidate = selection_plan("Provider", &["run"], &["run"]);
    let mut substituted = candidate.clone();
    substituted.schema.methods[0].requirement_owner = "OtherPair".to_owned();
    assert_eq!(
        substituted.report_fingerprint(),
        candidate.report_fingerprint()
    );
    assert_ne!(substituted.identity_digest(), candidate.identity_digest());
    let selected = abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selected_plans(vec![substituted])
        .expect("compact-equal selected provider closure");
    let original = Arc::new(typed_trees_to_checked_trees::checked_trees::CheckedTrees::default());
    let original_contents = original.as_ref().clone();

    let diagnostics = bind_selected_provider_plan_facts(
        &original,
        std::slice::from_ref(&candidate),
        selected,
        std::slice::from_ref(&candidate.name),
        &[],
    )
    .expect_err("compact-equal selected plan must not satisfy an exact provider grant");

    assert!(diagnostics[0].message.contains("0 exact candidate rows"));
    assert_eq!(original.as_ref(), &original_contents);
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
        let selected = abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selection(
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
    let first_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x41; 32])
        .expect("nonzero package identity");
    let second_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x42; 32])
        .expect("nonzero package identity");
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
                inferred.machines[0].inferred_transitive = vec![
                    typed_trees_to_checked_trees::flow_effects::InvocationTarget::Parameter(1),
                ];
            }
            CheckedInvocationDrift::UnknownParameterType => {}
            CheckedInvocationDrift::InvalidService => {
                inferred.machines[0].inferred_transitive = vec![
                    typed_trees_to_checked_trees::flow_effects::InvocationTarget::Service(
                        symbols::SymbolHandle::invalid(),
                    ),
                ];
            }
            CheckedInvocationDrift::UnknownService => {
                inferred.machines[0].inferred_transitive = vec![
                    typed_trees_to_checked_trees::flow_effects::InvocationTarget::Service(
                        symbols::SymbolHandle::from_arena_index(99),
                    ),
                ];
            }
            CheckedInvocationDrift::NonBoundaryService => {
                typed.push_trait_definition(symbol_resolved_trees_to_typed_trees::typed_trees::trait_definition::TraitDefinition {
                    symbol: symbols::SymbolHandle::from_arena_index(99),
                    name: symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier::generated("pkg::Plain"),
                    ..Default::default()
                });
                inferred.machines[0].inferred_transitive = vec![
                    typed_trees_to_checked_trees::flow_effects::InvocationTarget::Service(
                        symbols::SymbolHandle::from_arena_index(99),
                    ),
                ];
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
            &select_provider_plans(&plans, target::NativeTarget::host(), &[], &[])
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
            &select_provider_plans(&plans, target::NativeTarget::host(), &[], &[])
                .expect("one covering candidate selects")
        ),
        vec!["CompleteProvider".to_owned()]
    );
}

#[test]
#[cfg(feature = "installed-writer")]
fn external_root_bridge_requires_one_exact_retained_boundary_slot() {
    let mut first = selection_plan("FirstProvider", &["run"], &["run"]);
    first.schema.trait_name = "first::Pair".into();
    let mut second = selection_plan("SecondProvider", &["run"], &["run"]);
    second.schema.trait_name = "second::Pair".into();
    let facts = abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selection(
        &[first.clone(), second],
        &["FirstProvider".into(), "SecondProvider".into()],
    )
    .expect("distinct qualified boundary slots may both be selected");
    assert_eq!(
        selected_external_root_provider_plan_id(&facts, "first::Pair")
            .expect("qualified slot resolves")
            .normalized_identity(),
        first.report_fingerprint()
    );
    assert!(
        selected_external_root_provider_plan_id(&facts, "Pair")
            .expect_err("an ambiguous leaf slot must reject")
            .0
            .contains("matches 2 retained selected provider plans")
    );
}

#[test]
#[cfg(feature = "installed-writer")]
fn external_root_bridge_rejects_compact_equal_exact_plan_substitution() {
    let plan = selection_plan("Provider", &["run"], &["run"]);
    let facts = abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
        .expect("selected provider facts");
    let mut selected = selected_external_root_provider_plan(&facts, "Pair")
        .expect("external-root provider selection");
    let compact_identity = selected.identity;
    let strong_identity = selected.digest;

    selected.exact_plan.schema.methods[0].requirement_owner = "OtherPair".into();
    selected.schema = selected.exact_plan.schema.clone();

    assert_eq!(selected.identity, compact_identity);
    assert_eq!(
        selected.exact_plan.report_fingerprint(),
        compact_identity.normalized_identity(),
        "the compatibility fingerprint omits this exact structural field"
    );
    assert_ne!(selected.exact_plan.identity_digest(), strong_identity);
    assert!(selected.entry_claims("Pair::run").is_err());
}

#[test]
fn granted_selected_plan_attaches_receipt_by_exact_inherited_requirement() {
    let owner_symbol = symbols::SymbolHandle::from_arena_index(7);
    let subject_symbol = symbols::SymbolHandle::from_arena_index(8);
    let domain_symbol = symbols::SymbolHandle::from_arena_index(9);
    let requirement_symbol = symbols::SymbolHandle::from_arena_index(10);
    let mut checked = typed_trees_to_checked_trees::checked_trees::CheckedTrees::default();
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
    let identity = selected.report_fingerprint();
    let original_contents = checked.clone();
    let original = Arc::new(checked);

    let binding = bind_selected_provider_plan_facts(
        &original,
        std::slice::from_ref(&selected),
        abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&selected),
            &["FirstProvider".to_owned()],
        )
        .expect("canonical selected facts"),
        &["PairChild".to_owned()],
        &[],
    )
    .expect("exact inherited requirement binds the selected child-schema plan");
    let (checked, _, _) = binding.into_parts();

    assert!(!Arc::ptr_eq(&checked, &original));
    assert_eq!(original.as_ref(), &original_contents);
    assert_eq!(
        original
            .facts
            .semantic
            .facts
            .get(fact)
            .evidence
            .receipt_identity,
        0
    );
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
    let owner_symbol = symbols::SymbolHandle::from_arena_index(7);
    let selected_requirement = symbols::SymbolHandle::from_arena_index(10);
    let evidence_requirement = symbols::SymbolHandle::from_arena_index(11);
    let mut checked = typed_trees_to_checked_trees::checked_trees::CheckedTrees::default();
    let mut owner =
        symbol_resolved_trees_to_typed_trees::typed_trees::trait_definition::TraitDefinition {
            symbol: owner_symbol,
            is_boundary: true,
            name: symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier::generated(
                "PairBase",
            ),
            ..Default::default()
        };
    for (symbol, name) in [
        (selected_requirement, "first"),
        (evidence_requirement, "second"),
    ] {
        checked.typed.push_trait_machine_signature(
            &mut owner,
            symbol_resolved_trees_to_typed_trees::typed_trees::signature::StateSignature {
                symbol,
                name:
                    symbol_resolved_trees_to_typed_trees::typed_trees::name::Identifier::generated(
                        name,
                    ),
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
        symbols::SymbolHandle::from_arena_index(8),
        symbols::SymbolHandle::from_arena_index(9),
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

    bind_selected_provider_plan_facts_for_test(
        &mut checked,
        std::slice::from_ref(&selected),
        abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selection(
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
    let owner_symbol = symbols::SymbolHandle::from_arena_index(7);
    let requirement_symbol = symbols::SymbolHandle::from_arena_index(10);
    let mut checked = typed_trees_to_checked_trees::checked_trees::CheckedTrees::default();
    let requirement_identity = push_boundary_requirement(
        &mut checked,
        owner_symbol,
        "PairBase",
        requirement_symbol,
        "first",
    );
    append_admitted_fact(
        &mut checked,
        symbols::SymbolHandle::from_arena_index(8),
        symbols::SymbolHandle::from_arena_index(9),
        owner_symbol,
        symbols::SymbolHandle::from_arena_index(11),
    );
    let mut selected = selection_plan("FirstProvider", &["first"], &["first"]);
    set_exact_requirement(
        &mut selected,
        "PairChild",
        "PairBase",
        &requirement_identity,
    );

    let diagnostics = bind_selected_provider_plan_facts_for_test(
        &mut checked,
        std::slice::from_ref(&selected),
        abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts::from_selection(
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
