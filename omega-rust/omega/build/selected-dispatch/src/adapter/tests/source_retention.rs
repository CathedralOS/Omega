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
fn retaining_settlement_publishes_exact_restorable_edits_atomically() {
    let (checked, plans) = sourced_checked_fixture();
    let selected = selected_plan(&plans, "Echo");
    let (span, index, statement_before) = statement_call(&checked, "emit");
    let (expression, expression_before) = expression_call(&checked, "echo");
    let contents = checked.clone();
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    let edits =
        settle_selected_boundary_adapter_dispatch_with_source_edits(&mut settled, &selected)
            .expect("valid exact source custody permits publication");
    assert!(!Arc::ptr_eq(&original, &settled));
    assert_eq!(original.as_ref(), &contents);
    assert_eq!(settled.facts, original.facts);
    let source = edits
        .source_trees(&settled.typed)
        .expect("restore exact edits");
    assert!(matches!(source, std::borrow::Cow::Owned(_)));
    assert_eq!(
        &source.statement_table.statements(span)[index],
        &typed_trees::statement::StatementNode::Call(statement_before),
    );
    assert_eq!(
        source.expression_table.expression(expression),
        &ExpressionNode::Call(expression_before),
    );
}

#[test]
fn retaining_guard_failure_preserves_shared_arc_and_contents() {
    // This transform-only fixture deliberately has no source custody for its
    // attached field aliases. Selection preflight succeeds, but sealing the
    // retained source graph must fail before publishing the staged mutation.
    let (checked, plans) = checked_fixture();
    let selected = selected_plan(&plans, "Echo");
    let contents = checked.clone();
    let original = Arc::new(checked);
    let mut rejected = Arc::clone(&original);
    let diagnostics =
        settle_selected_boundary_adapter_dispatch_with_source_edits(&mut rejected, &selected)
            .expect_err("source-free aliases cannot seal an exact source journal");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.message.contains("source custody") })
    );
    assert!(Arc::ptr_eq(&original, &rejected));
    assert_eq!(rejected.as_ref(), &contents);
    assert_eq!(original.as_ref(), &contents);
}
