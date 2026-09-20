use super::{
    Arc, CheckedTrees, ProviderPlan, SOURCE, bind_fixture_fused_service_erasures, checked_fixture,
    selected_every_plan, settle_selected_boundary_adapter_dispatch, typed_with_core_service,
};
use provider_planning::ProviderPlanDerivation;
fn sourced_checked_fixture() -> (CheckedTrees, Vec<ProviderPlan>) {
    let mut typed = typed_with_core_service("selected-dispatch/main.omg", SOURCE);
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    bind_fixture_fused_service_erasures(&mut typed, &selected_every_plan(&plans));
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check sourced dispatch fixture");
    (checked, plans)
}

#[test]
fn selection_retains_the_canonical_source_without_a_restoration_journal() {
    let (checked, plans) = sourced_checked_fixture();
    let selected = selected_every_plan(&plans);
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
    assert_eq!(settled.typed, original.typed);
    assert!(!settled.facts.boundary_adapter_dispatch.is_empty());
}

#[test]
fn source_free_selection_needs_no_synthetic_source_aliases() {
    let (checked, plans) = checked_fixture();
    let selected = selected_every_plan(&plans);
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
    assert_eq!(settled.typed, original.typed);
}
