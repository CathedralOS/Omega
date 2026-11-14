/* Typed-domain filtering retains every exposed candidate until carrier checking. */
use super::domain_constraints::select_domain_candidates_where;
use source::{SourceId, SourceSpan, Span};
use std::sync::Arc;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

fn reference(source: usize) -> SourceSpan {
    SourceSpan::new(SourceId(source), Span::new(0, 4))
}

fn fixture(
    imports: &[&str],
    modules: bool,
) -> (
    symbol_resolved_trees::SymbolResolvedTrees,
    typed_trees::TypedTrees,
    Vec<SymbolHandle>,
) {
    let mut sources = source::SourceMap::default();
    for ordinal in 0..3 {
        sources.add_with_metadata(
            format!("app/{ordinal}.omg").into(),
            "name".to_owned(),
            "app".into(),
            None,
            source::SourceOrigin::User,
        );
    }
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
            (
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "u64::Mark",
                    source_span: reference(1),
                },
            ),
            (
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "u64::Mark",
                    source_span: reference(2),
                },
            ),
        ],
    ))
    .collect::<Vec<_>>();
    let mut symbols = builder.finish();
    if modules {
        symbols
            .register_source_module(SourceId(1), [("one", reference(1))])
            .unwrap();
        symbols
            .register_source_module(SourceId(2), [("two", reference(2))])
            .unwrap();
    }
    for import in imports {
        symbols.register_source_import(SourceId(0), import);
    }
    let source = symbol_resolved_trees::SymbolResolvedTrees {
        symbols: symbols.clone(),
        ..Default::default()
    };
    let mut typed = typed_trees::TypedTrees {
        symbols,
        ..Default::default()
    };
    for symbol in &declarations[1..] {
        typed.push_domain_definition(typed_trees::domain::DomainDefinition {
            symbol: *symbol,
            name: typed_trees::name::Identifier::generated("u64::Mark"),
            is_public: true,
            ..Default::default()
        });
    }
    (source, typed, declarations[1..].to_vec())
}

#[test]
fn domain_exposure_typed_pool_preserves_broad_import_ambiguity() {
    let (source, typed, symbols) = fixture(&["one", "two"], true);
    let candidates = select_domain_candidates_where(
        &source,
        &typed,
        "u64::Mark",
        Some(reference(0)),
        SymbolHandle::invalid(),
        |_| true,
    );
    assert_eq!(
        candidates
            .iter()
            .map(|domain| domain.symbol)
            .collect::<Vec<_>>(),
        symbols
    );
    let candidates = select_domain_candidates_where(
        &source,
        &typed,
        "u64::Mark",
        Some(reference(0)),
        SymbolHandle::invalid(),
        |domain| domain.symbol == symbols[1],
    );
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].symbol, symbols[1]);
}

#[test]
fn domain_exposure_typed_pool_does_not_invent_a_source_scope() {
    let (source, typed, _) = fixture(&[], false);
    assert!(
        select_domain_candidates_where(
            &source,
            &typed,
            "Mark",
            None,
            SymbolHandle::invalid(),
            |_| true,
        )
        .is_empty()
    );
}

#[test]
fn domain_exposure_typed_pool_keeps_retained_selection_without_a_new_import() {
    let (source, typed, symbols) = fixture(&[], true);
    let candidates =
        select_domain_candidates_where(&source, &typed, "Mark", None, symbols[0], |_| true);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].symbol, symbols[0]);
}

#[test]
fn domain_exposure_typed_pool_does_not_expose_loaded_modules() {
    let (source, typed, _) = fixture(&[], true);
    assert!(
        select_domain_candidates_where(
            &source,
            &typed,
            "u64::Mark",
            Some(reference(0)),
            SymbolHandle::invalid(),
            |_| true,
        )
        .is_empty()
    );
}
