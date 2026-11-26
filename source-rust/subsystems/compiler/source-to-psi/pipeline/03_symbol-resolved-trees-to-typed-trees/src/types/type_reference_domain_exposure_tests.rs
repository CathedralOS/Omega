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
    for name in ["consumer", "one", "two"] {
        sources.add_with_metadata(
            format!("app/{name}.omg").into(),
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
        symbols.register_source_file_import(
            SourceId(importer),
            None,
            &format!("{import}.omg"),
            import,
        );
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
fn domain_exposure_typed_pool_selects_only_the_qualified_file() {
    let (source, typed, symbols) = fixture(&["one", "two"], true);
    for (name, expected) in [
        ("|one|::u64::Mark", symbols[0]),
        ("|two|::u64::Mark", symbols[1]),
    ] {
        let candidates = select_domain_candidates_where(
            &source,
            &typed,
            name,
            Some(reference(0)),
            SymbolHandle::invalid(),
            |_| true,
        );
        assert_eq!(
            candidates
                .iter()
                .map(|domain| domain.symbol)
                .collect::<Vec<_>>(),
            [expected]
        );
        let filtered = select_domain_candidates_where(
            &source,
            &typed,
            name,
            Some(reference(0)),
            SymbolHandle::invalid(),
            |domain| domain.symbol != expected,
        );
        assert!(filtered.is_empty());
    }
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

#[test]
fn domain_exposure_typed_pool_keeps_local_and_imported_domains_distinct() {
    let (source, typed, symbols) = fixture_importing_into(1, &["two"], true);
    for (name, expected) in [("u64::Mark", symbols[0]), ("|two|::u64::Mark", symbols[1])] {
        let candidates = select_domain_candidates_where(
            &source,
            &typed,
            name,
            Some(reference(1)),
            SymbolHandle::invalid(),
            |_| true,
        );
        assert_eq!(
            candidates
                .iter()
                .map(|domain| domain.symbol)
                .collect::<Vec<_>>(),
            [expected]
        );
    }
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

#[test]
fn a_formal_and_its_membership_contract_select_the_same_qualified_indices() {
    use crate::source_state::types::{TypeConstraintNode, TypeReferenceNode};

    let resolved = crate::resolve_fixture(
        "domain u16::Positive requires self > 0u16;
         data Count = u16 in Positive;
         data Segment<T> [linear] { value: T; }
         domain<T> Segment<T>::Stored<T>;
         data Kept = Segment<Count> in Stored<Count>;
         domain<T> Kept::Side<T>;
         machine take(x: Segment<Count> in Side<Count> & Stored<Count>) -> Kept in Side<Count> { x }
         machine take_alias(x: Kept in Side<Count>) -> Kept in Side<Count> { x }",
    );
    let typed = crate::lower_symbol_resolved_trees(&resolved).unwrap();
    let machine = typed
        .machines()
        .iter()
        .find(|m| m.name.as_str() == "take")
        .unwrap();
    let state = &typed.machine_states(machine)[0];
    let reference = typed.state_parameters(state)[0].type_reference;
    let types = &typed.type_reference_table;
    let TypeReferenceNode::Constrained { constraints, .. } = types.type_reference(reference) else {
        panic!("the formal carries both memberships");
    };
    let declared: Vec<_> = types
        .constraints(*constraints)
        .iter()
        .map(|constraint| {
            let TypeConstraintNode::Domain(domain) = constraint else {
                panic!("expected a domain");
            };
            (domain.symbol, domain.semantic_id)
        })
        .collect();
    assert_eq!(declared.len(), 2);
    for machine in typed.machines().iter() {
        let state = &typed.machine_states(machine)[0];
        let required: Vec<_> = typed
            .state_contracts(state)
            .iter()
            .flat_map(|contract| typed.proof_facts.span_or_empty(contract.facts))
            .map(|fact| {
                let membership = fact.as_membership().unwrap();
                (membership.domain_symbol, membership.semantic_domain)
            })
            .collect();
        assert_eq!(required.len(), declared.len(), "{}", machine.name.as_str());
        for membership in &declared {
            assert!(
                required.contains(membership),
                "{} lost {membership:?} in {required:?}",
                machine.name.as_str()
            );
        }
    }
}
