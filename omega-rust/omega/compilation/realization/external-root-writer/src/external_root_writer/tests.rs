//! Retained external-root slot resolution and exact plan identity.

use super::{selected_external_root_provider_plan, selected_external_root_provider_plan_id};
use effects::provider_plan::{
    ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
};

/// One selected `Pair` provider with a single `run` method bound to a table
/// slot.
fn selection_plan(name: &str) -> ProviderPlan {
    ProviderPlan {
        name: name.to_owned(),
        provider_type: name.to_owned(),
        schema: ServiceSchema {
            trait_name: "Pair".to_owned(),
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
                name: "run".to_owned(),
                requirement_owner: "Pair".to_owned(),
                requirement_identity: "Pair::run".to_owned(),
                service_reach: vec!["Pair".to_owned()],
                ..Default::default()
            }],
        },
        rows: vec![ProviderPlanRow {
            method: "run".to_owned(),
            requirement_identity: "Pair::run".to_owned(),
            requirement_lifetime_partition: Vec::new(),
            binding: ProviderBinding::VtableSlot { index: 0 },
        }],
        ..Default::default()
    }
}

#[test]
fn external_root_bridge_requires_one_exact_retained_boundary_slot() {
    let mut first = selection_plan("FirstProvider");
    first.schema.trait_name = "first::Pair".into();
    let mut second = selection_plan("SecondProvider");
    second.schema.trait_name = "second::Pair".into();
    let facts = effects::SelectedProviderPlanFacts::from_selection(
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
fn external_root_bridge_rejects_compact_equal_exact_plan_substitution() {
    let plan = selection_plan("Provider");
    let facts = effects::SelectedProviderPlanFacts::from_selected_plans(vec![plan])
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
