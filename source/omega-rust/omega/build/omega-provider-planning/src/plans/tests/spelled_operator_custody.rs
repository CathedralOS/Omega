use super::*;

fn fixed_token_checked_adapter_fixture() -> (psi_checked_trees::CheckedTrees, ProviderPlan) {
    let source = r#"
        data CheckedMath {}
        boundary operator + CheckedMath::add(left: f64, right: f64) -> f64;

        data CheckedMathProvider {}
        machine CheckedMathProvider::add(left: f64, right: f64) -> f64
        satisfies CheckedMath::add
        {
            transition { _ -> (left) }
        }

        machine run(left: f64, right: f64) -> f64 {
            transition { _ -> (left + right) }
        }
    "#;
    let tokens = psi_source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize fixed-token checked-adapter fixture");
    let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse fixed-token checked-adapter fixture");
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve fixed-token checked-adapter fixture");
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type fixed-token checked-adapter fixture");
    let plans = derive_satisfies_plans(&typed, None);
    let [plan] = plans.as_slice() else {
        panic!(
            "fixed-token checked-adapter fixture must derive one provider plan, got {}",
            plans.len()
        )
    };
    let checked = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check fixed-token checked-adapter fixture");
    assert!(
        checked.facts.operators.named_uses.is_empty(),
        "the fixture must exercise only spelled operator custody"
    );
    assert_eq!(
        checked.facts.operators.uses.len(),
        1,
        "the fixture must retain one spelled boundary-operator use"
    );
    (checked, plan.clone())
}

fn missing_trait_plan() -> ProviderPlan {
    ProviderPlan {
        name: "MissingProvider".to_owned(),
        provider_type: "MissingProvider".to_owned(),
        provider_type_package_identity: None,
        target: String::new(),
        schema: ServiceSchema {
            trait_name: "MissingBoundary".to_owned(),
            trait_package_identity: None,
            methods: vec![omega_effects::provider_plan::ServiceMethod {
                name: "missing".to_owned(),
                requirement_owner: "MissingBoundary".to_owned(),
                requirement_owner_package_identity: None,
                requirement_identity: "MissingBoundary::missing".to_owned(),
                parameter_count: 0,
                parameter_type_identities: Vec::new(),
                entry_claims: Vec::new(),
                has_result: false,
                result_type_identity: None,
                result_claims: Vec::new(),
                service_reach: vec!["MissingBoundary".to_owned()],
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
                termination_premises: Vec::new(),
                calling_plan_report_fingerprint: None,
                calling_plan_commitment: None,
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "missing".to_owned(),
            requirement_identity: "MissingBoundary::missing".to_owned(),
            binding: ProviderBinding::VtableSlot { index: 0 },
        }],
        origin_package_identity: None,
        origin_package: String::new(),
    }
}

#[test]
fn selected_fixed_token_checked_adapter_copies_both_plan_coordinates_under_shared_custody() {
    let (checked, plan) = fixed_token_checked_adapter_fixture();
    let (use_handle, use_before) = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .next()
        .expect("one spelled boundary-operator use");
    assert_eq!(use_before.provider_plan_report_fingerprint, 0);
    assert!(use_before.provider_plan_commitment.is_empty());

    let original_contents = checked.clone();
    let original = Arc::new(checked);
    let retained_custodian = Arc::clone(&original);
    let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("select exact fixed-token provider");

    let binding = bind_selected_provider_plan_facts(
        &original,
        std::slice::from_ref(&plan),
        selected,
        &[],
        &[],
    )
    .expect("bind exact fixed-token checked adapter");
    let (bound, selected, _) = binding.into_parts();

    assert!(Arc::ptr_eq(&original, &retained_custodian));
    assert!(!Arc::ptr_eq(&bound, &original));
    assert_eq!(original.as_ref(), &original_contents);
    assert_eq!(retained_custodian.as_ref(), &original_contents);
    let retained_use = original.facts.operators.uses.get(use_handle);
    assert_eq!(retained_use.provider_plan_report_fingerprint, 0);
    assert!(retained_use.provider_plan_commitment.is_empty());

    let bound_use = bound.facts.operators.uses.get(use_handle);
    assert_eq!(
        bound_use.provider_plan_report_fingerprint,
        plan.report_fingerprint()
    );
    assert_eq!(
        bound_use.provider_plan_commitment.as_bytes(),
        plan.identity_digest().as_bytes()
    );
    assert!(selected.installation_reach_resolutions().is_empty());
}

#[test]
fn later_failure_publishes_no_staged_fixed_token_checked_adapter_update() {
    let (checked, operator_plan) = fixed_token_checked_adapter_fixture();
    let use_handle = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, _)| handle)
        .next()
        .expect("one spelled boundary-operator use");
    let missing_trait_plan = missing_trait_plan();
    let candidates = [operator_plan.clone(), missing_trait_plan.clone()];
    let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
        &candidates,
        &[operator_plan.name.clone(), missing_trait_plan.name.clone()],
    )
    .expect("select the operator and later-failing trait plans");
    let original_contents = checked.clone();
    let original = Arc::new(checked);
    let retained_custodian = Arc::clone(&original);

    let diagnostics = bind_selected_provider_plan_facts(&original, &candidates, selected, &[], &[])
        .expect_err("missing typed requirement must reject after spelled-update staging");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "selected provider row `MissingBoundary::missing` resolves to 0 exact typed requirements",
        )
    }));
    assert!(Arc::ptr_eq(&original, &retained_custodian));
    assert_eq!(original.as_ref(), &original_contents);
    assert_eq!(retained_custodian.as_ref(), &original_contents);
    let retained_use = original.facts.operators.uses.get(use_handle);
    assert_eq!(retained_use.provider_plan_report_fingerprint, 0);
    assert!(retained_use.provider_plan_commitment.is_empty());
}
