use super::*;

fn workload(count: usize) -> (TypedTrees, Vec<(String, SymbolHandle)>) {
    let mut builder = crate::source_symbols::SymbolTableBuilder::new();
    let root = builder.insert_root(
        symbols::SymbolKind::Root,
        symbols::SymbolNameRef::Static("root"),
    );
    let names = (0..count)
        .map(|position| format!("Boundary{position}"))
        .collect::<Vec<_>>();
    let declarations = builder.insert_children(
        root,
        names.iter().map(|name| {
            (
                symbols::SymbolKind::Trait,
                symbols::SymbolNameRef::Borrowed(name),
            )
        }),
    );
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    let mut selected = Vec::new();
    for (name, symbol) in names.into_iter().zip(declarations.handles()) {
        program.tables.traits.append_to_span(
            &mut program.roots.traits,
            crate::source_state::trait_definition::TraitDefinition {
                symbol,
                is_boundary: true,
                ..Default::default()
            },
        );
        selected.push((name, symbol));
    }
    (program, selected)
}

#[test]
fn product_declaration_lookup_workload() {
    for count in [8, 512] {
        let started = std::time::Instant::now();
        let (program, declarations) = workload(count);
        let lookup =
            ProductDeclarationLookup::new(&program, &[ProductOperandKind::BoundarySubject]);
        for position in 0..4096 {
            let (path, expected) = &declarations[position % count];
            assert_eq!(
                lookup.resolve(
                    path,
                    source_text::SourceSpan::default(),
                    ProductOperandKind::BoundarySubject
                ),
                Some(*expected)
            );
        }
        eprintln!(
            "product lookups: declarations={count} requests=4096 elapsed={:?}",
            started.elapsed()
        );
    }
}

#[test]
fn product_declaration_names_follow_mutation_and_exact_rosters() {
    let (mut program, declarations) = workload(2);
    let occurrence = source_text::SourceSpan::default();
    let lookup = |program: &TypedTrees, path| {
        resolve_product_path(
            program,
            path,
            occurrence,
            ProductOperandKind::BoundarySubject,
        )
    };
    assert_eq!(lookup(&program, "Boundary0"), Some(declarations[0].1));
    let snapshot = program.clone();
    let retained =
        ProductDeclarationLookup::new(&snapshot, &[ProductOperandKind::BoundarySubject]);
    program
        .tables
        .traits
        .get_mut(program.roots.traits.start())
        .is_boundary = false;
    assert_eq!(lookup(&program, "Boundary0"), None);
    assert_eq!(lookup(&snapshot, "Boundary0"), Some(declarations[0].1));
    assert_eq!(
        retained.resolve("Boundary0", occurrence, ProductOperandKind::BoundarySubject),
        Some(declarations[0].1)
    );
    program
        .tables
        .traits
        .get_mut(program.roots.traits.start())
        .is_boundary = true;
    program.symbols.rename(declarations[0].1, "Renamed");
    assert_eq!(lookup(&program, "Boundary0"), None);
    assert_eq!(lookup(&program, "Renamed"), Some(declarations[0].1));
    let roots = program.roots.traits;
    program.roots.traits = arena::HandleSpan::from_parts(roots.start(), 1);
    assert_eq!(lookup(&program, "Boundary1"), None);
    program.roots.traits = roots;
    assert_eq!(lookup(&program, "Boundary1"), Some(declarations[1].1));
    program.symbols.rename(declarations[1].1, "Renamed");
    assert_eq!(
        lookup(&program, "Renamed"),
        None,
        "competing exact declarations stay ambiguous"
    );
    let limits = record_storage::RecordLimits::default();
    let restored: TypedTrees =
        record_storage::decode(&record_storage::encode(&program, limits).unwrap(), limits)
            .unwrap();
    assert_eq!(lookup(&restored, "Renamed"), None);
    assert_eq!(
        resolve_product_path(
            &restored,
            "Renamed",
            occurrence,
            ProductOperandKind::Provider
        ),
        None,
        "the same spelling cannot supply a different role"
    );
}

#[test]
fn product_declaration_authors_keep_exact_span_and_ambiguity() {
    let span = source_text::SourceSpan::new(
        source_text::SourceId(1),
        source_text::Span::new(4, 9),
    );
    let mut builder = crate::source_symbols::SymbolTableBuilder::new();
    let root = builder.insert_root(
        symbols::SymbolKind::Root,
        symbols::SymbolNameRef::Static("root"),
    );
    let declarations = builder.insert_children(
        root,
        ["first", "second"].map(|value| {
            (
                symbols::SymbolKind::Machine,
                symbols::SymbolNameRef::OwnedSource {
                    value,
                    source_span: span,
                },
            )
        }),
    );
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..TypedTrees::default()
    };
    program.push_machine(crate::source_state::machine::Machine {
        symbol: declarations.start(),
        ..Default::default()
    });
    assert_eq!(
        resolve_build_request_author(&program, span),
        Ok(declarations.start())
    );
    let snapshot = program.clone();
    program.push_machine(crate::source_state::machine::Machine {
        symbol: declarations.handle_at(1),
        ..Default::default()
    });
    assert!(
        resolve_build_request_author(&program, span)
            .unwrap_err()
            .contains("ambiguous authored machine custody")
    );
    assert_eq!(
        resolve_build_request_author(&snapshot, span),
        Ok(declarations.start())
    );
    program.roots.machines = arena::HandleSpan::empty();
    assert!(
        resolve_build_request_author(&program, span)
            .unwrap_err()
            .contains("no authored selecting machine")
    );
}

#[test]
fn product_file_aliases_select_only_the_boundary_in_the_imported_file() {
    use crate::source_symbols::{SourceScopedTopLevelBinding, SymbolTableBuilder};
    use source_text::{SourceSpan, Span};
    use symbols::{SymbolKind, SymbolNameRef};

    let mut sources = crate::source_inputs::SourceMap::default();
    let [author, first, second] = ["build.omg", "one.omg", "two.omg"].map(|file| {
        sources
            .add_with_metadata(
                std::path::Path::new("package").join(file),
                "Clock".to_owned(),
                "package".into(),
                semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
                source_admission::SourceOrigin::User,
            )
            .source_id
    });
    let span = |source| SourceSpan::new(source, Span::new(0, 5));
    let bindings = vec![
        SourceScopedTopLevelBinding::file_import(author, first, "one"),
        SourceScopedTopLevelBinding::file_import(author, second, "two"),
    ];
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(std::sync::Arc::new(sources)),
        bindings,
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let traits = builder.insert_children(
        root,
        [first, second].map(|file| (SymbolKind::Trait, SymbolNameRef::Source(span(file)))),
    );
    let mut program = TypedTrees {
        symbols: builder.finish(),
        ..Default::default()
    };
    for symbol in traits.handles() {
        program.tables.traits.append_to_span(
            &mut program.roots.traits,
            crate::source_state::trait_definition::TraitDefinition {
                symbol,
                is_boundary: true,
                ..Default::default()
            },
        );
    }

    let role = ProductOperandKind::BoundarySubject;
    let lookup = ProductDeclarationLookup::new(&program, &[role]);
    assert_eq!(
        lookup.resolve("|one|::Clock", span(author), role),
        Some(traits.handle_at(0))
    );
    assert_eq!(
        lookup.resolve("|two|::Clock", span(author), role),
        Some(traits.handle_at(1))
    );
    assert_eq!(lookup.resolve("|missing|::Clock", span(author), role), None);
}
