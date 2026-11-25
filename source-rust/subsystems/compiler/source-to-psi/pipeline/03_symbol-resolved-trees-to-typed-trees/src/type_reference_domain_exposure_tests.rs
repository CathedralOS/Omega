/* Typed-domain filtering retains every exposed candidate until carrier checking. */
use super::domain_constraints::select_domain_candidates_where;
use crate::source_symbols::SymbolTableBuilder;
use source_text::{SourceId, SourceSpan, Span};
use std::sync::Arc;
use symbols::{SymbolHandle, SymbolKind, SymbolNameRef};

fn reference(source: usize) -> SourceSpan {
    SourceSpan::new(SourceId(source), Span::new(0, 4))
}

type Fixture = (
    crate::source_resolved::SymbolResolvedTrees,
    crate::source_state::TypedTrees,
    Vec<SymbolHandle>,
);

fn fixture(imports: &[&str], modules: bool) -> Fixture {
    fixture_importing_into(0, imports, modules)
}

fn fixture_importing_into(importer: usize, imports: &[&str], modules: bool) -> Fixture {
    let mut sources = crate::source_inputs::SourceMap::default();
    for ordinal in 0..3 {
        sources.add_with_metadata(
            format!("app/{ordinal}.omg").into(),
            "name".to_owned(),
            "app".into(),
            None,
            source_admission::SourceOrigin::User,
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
    let source = crate::source_resolved::SymbolResolvedTrees {
        symbols: symbols.clone(),
        ..Default::default()
    };
    let mut typed = crate::source_state::TypedTrees {
        symbols,
        ..Default::default()
    };
    for symbol in &declarations[1..] {
        typed.push_domain_definition(crate::source_state::domain::DomainDefinition {
            symbol: *symbol,
            name: crate::source_state::name::Identifier::generated("u64::Mark"),
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

#[test]
fn an_indexed_domain_matches_the_qualification_on_its_declared_carrier() {
    use crate::source_state::domain::domain_accepts_carrier;

    let resolved = crate::resolve_fixture(
        "data Word {}
         data Segment<T> { value: T; }
         domain<T> Segment<T>::Stored<T>;
         data Kept = Segment<Word> in Stored<Word>;
         domain<T> Kept::Side<T>;",
    );
    let typed = crate::lower_symbol_resolved_trees(&resolved).unwrap();
    let family = typed
        .domain_definitions()
        .iter()
        .find(|d| d.name.as_str() == "Kept::Side")
        .unwrap();
    let target = family.target_type;
    let bare = typed
        .type_reference_table
        .unconstrained_type_reference(target);
    assert_ne!(target, bare, "the alias expansion retains Stored");
    assert!(domain_accepts_carrier(
        &typed,
        family,
        target,
        &family.index_arguments
    ));
    assert!(!domain_accepts_carrier(
        &typed,
        family,
        bare,
        &family.index_arguments
    ));
}
