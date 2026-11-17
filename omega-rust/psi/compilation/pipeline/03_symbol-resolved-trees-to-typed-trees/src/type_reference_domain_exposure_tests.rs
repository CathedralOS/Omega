/* Typed-domain filtering retains every exposed candidate until carrier checking. */
use super::domain_constraints::select_domain_candidates_where;
use source::{SourceId, SourceSpan, Span};
use std::sync::Arc;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};

fn reference(source: usize) -> SourceSpan {
    SourceSpan::new(SourceId(source), Span::new(0, 4))
}

type Fixture = (
    symbol_resolved_trees::SymbolResolvedTrees,
    typed_trees::TypedTrees,
    Vec<SymbolHandle>,
);

fn fixture(imports: &[&str], modules: bool) -> Fixture {
    fixture_importing_into(0, imports, modules)
}

fn fixture_importing_into(importer: usize, imports: &[&str], modules: bool) -> Fixture {
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
        symbols.register_source_import(SourceId(importer), import);
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

/*
    Module `one` declares `u64::Mark` and imports `two`, which declares one
    too. The referencing module's own declaration does not outrank the
    imported one: both stay candidates and the spelling is ambiguous
    (`wiki/spec/language/modules.md#resolution`). Without the import, the
    module's own domain is the only one visible.
*/
#[test]
fn domain_exposure_typed_pool_keeps_the_local_domain_competing_with_an_import() {
    let candidates = |imports: &[&str]| {
        let (source, typed, _) = fixture_importing_into(1, imports, true);
        select_domain_candidates_where(
            &source,
            &typed,
            "u64::Mark",
            Some(reference(1)),
            SymbolHandle::invalid(),
            |_| true,
        )
        .iter()
        .map(|domain| domain.symbol)
        .collect::<Vec<_>>()
    };
    let (_, _, symbols) = fixture(&[], true);
    assert_eq!(candidates(&["two"]), symbols);
    assert_eq!(candidates(&[]), symbols[..1]);
}
