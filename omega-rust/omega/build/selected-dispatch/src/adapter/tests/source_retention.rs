use super::*;

fn sourced_checked_fixture() -> (CheckedTrees, Vec<ProviderPlan>) {
    let mut sources = source::SourceMap::default();
    sources.add("selected-dispatch/main.omg".into(), SOURCE.into());
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize sourced dispatch fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse sourced dispatch fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
        &syntax,
        Arc::new(sources),
    )
    .expect("resolve sourced dispatch fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type sourced dispatch fixture");
    let plans = provider_planning::plans::derive_satisfies_plans(&typed, None);
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check sourced dispatch fixture");
    (checked, plans)
}

#[test]
fn selection_retains_the_canonical_source_without_a_restoration_journal() {
    let (checked, plans) = sourced_checked_fixture();
    let selected = selected_plan(&plans, "Echo");
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
    assert_eq!(settled.typed, original.typed);
    assert!(!settled.facts.boundary_adapter_dispatch.is_empty());
}

#[test]
fn source_free_selection_needs_no_synthetic_source_aliases() {
    let (checked, plans) = checked_fixture();
    let selected = selected_plan(&plans, "Echo");
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected).unwrap();
    assert_eq!(settled.typed, original.typed);
}
