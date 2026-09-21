use super::{
    Arc, ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceSchema,
    bind_selected_provider_plan_facts, derive_satisfies_plans,
};
use crate::ProviderPlanDerivation;
fn fixed_token_checked_adapter_fixture() -> (checked_trees::CheckedTrees, ProviderPlan) {
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
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize fixed-token checked-adapter fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse fixed-token checked-adapter fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve fixed-token checked-adapter fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type fixed-token checked-adapter fixture");
    let plans = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        panic!(
            "fixed-token checked-adapter fixture must derive one provider plan, got {}",
            plans.len()
        )
    };
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
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
            methods: vec![effects::provider_plan::ServiceMethod {
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
            requirement_lifetime_partition: Vec::new(),
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
    let selected = effects::SelectedProviderPlanFacts::from_selection(
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
    let selected = effects::SelectedProviderPlanFacts::from_selection(
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

/// The provider realizes `+` and spells `+` inside its own body; the caller
/// invokes the realization machine directly. Only the provider-internal
/// spelling is a boundary-operator use.
fn self_spelling_checked_adapter_fixture() -> (checked_trees::CheckedTrees, ProviderPlan) {
    let source = r#"
        data CheckedMath {}
        boundary operator + CheckedMath::add(left: f64, right: f64) -> f64;

        data CheckedMathProvider {}
        machine CheckedMathProvider::add(left: f64, right: f64) -> f64
        satisfies CheckedMath::add
        {
            transition { _ -> (left + right) }
        }

        machine run(left: f64, right: f64) -> f64 {
            transition { _ -> (CheckedMathProvider::add(left, right)) }
        }
    "#;
    let typed = super::typed_fixture(source);
    let plans = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        panic!(
            "self-spelling checked-adapter fixture must derive one provider plan, got {}",
            plans.len()
        )
    };
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check self-spelling checked-adapter fixture");
    (checked, plan.clone())
}

#[test]
fn spelling_the_operator_inside_the_provider_redispatches_while_a_direct_call_delegates() {
    let (checked, plan) = self_spelling_checked_adapter_fixture();
    let provider_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "CheckedMathProvider::add")
        .expect("the realization machine")
        .symbol;
    assert!(checked.facts.operators.named_uses.is_empty());
    let uses = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .collect::<Vec<_>>();
    let [(use_handle, use_before)] = uses.as_slice() else {
        panic!(
            "exactly one spelled use: the direct call in `run` is an ordinary call, got {}",
            uses.len()
        )
    };
    let checked_trees::CheckedValueOrigin::StateStatement { machine_symbol, .. } =
        use_before.origin
    else {
        panic!("the spelled use originates in a machine state")
    };
    assert_eq!(
        machine_symbol, provider_machine,
        "the only spelled use is the provider's own `+`"
    );
    assert_eq!(use_before.provider_plan_report_fingerprint, 0);

    let original = Arc::new(checked);
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&plan),
        std::slice::from_ref(&plan.name),
    )
    .expect("select the self-spelling provider");
    let binding = bind_selected_provider_plan_facts(
        &original,
        std::slice::from_ref(&plan),
        selected,
        &[],
        &[],
    )
    .expect("the provider-internal spelling redispatches through the selected plan");
    let (bound, _, _) = binding.into_parts();
    let bound_use = bound.facts.operators.uses.get(*use_handle);
    assert_eq!(
        bound_use.provider_plan_report_fingerprint,
        plan.report_fingerprint()
    );
    assert_eq!(
        bound_use.provider_plan_commitment.as_bytes(),
        plan.identity_digest().as_bytes()
    );
}
