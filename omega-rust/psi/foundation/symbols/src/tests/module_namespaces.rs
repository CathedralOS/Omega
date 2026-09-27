use std::path::PathBuf;
use std::sync::Arc;

use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceId, SourceMap, SourceOrigin, SourceSpan, Span};

use crate::{
    SourceScopedTopLevelBinding, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable,
    SymbolTableBuilder,
};

fn namespace_table(
    package_markers: [u8; 3],
    bindings: Vec<SourceScopedTopLevelBinding>,
) -> (SymbolTable, [SymbolHandle; 3], Arc<SourceMap>) {
    let mut sources = SourceMap::default();
    for (source_ordinal, marker) in package_markers.into_iter().enumerate() {
        let root = PathBuf::from(format!("package-{marker}"));
        sources.add_with_metadata(
            root.join(format!("source-{source_ordinal}.omg")),
            String::from("Item"),
            root,
            Some(PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero owner")),
            SourceOrigin::User,
        );
    }
    let sources = Arc::new(sources);
    let mut builder =
        SymbolTableBuilder::with_sources_and_top_level_bindings(Some(sources.clone()), bindings);
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        (0..3).map(|source_ordinal| {
            (
                SymbolKind::Data,
                SymbolNameRef::OwnedSource {
                    value: if source_ordinal == 0 { "Local" } else { "Item" },
                    source_span: reference(source_ordinal),
                },
            )
        }),
    ))
    .collect::<Vec<_>>();
    (
        builder.finish(),
        declarations.try_into().expect("three declarations"),
        sources,
    )
}

fn reference(source_ordinal: usize) -> SourceSpan {
    SourceSpan::new(SourceId(source_ordinal), Span::new(0, 4))
}

fn register_module(symbols: &mut SymbolTable, source_ordinal: usize, path: &[&str]) {
    symbols
        .register_source_module(
            SourceId(source_ordinal),
            path.iter().map(|name| (*name, reference(source_ordinal))),
        )
        .expect("one authored namespace per source");
}

fn data_reference(
    symbols: &SymbolTable,
    source_ordinal: usize,
    name: &str,
) -> Option<SymbolHandle> {
    symbols.find_top_level_by_name_and_kinds_from_source(
        name,
        &[SymbolKind::Data],
        reference(source_ordinal),
    )
}

#[test]
fn unmoduled_exact_imports_select_their_checked_instance_independent_of_order() {
    use source::DependencyScope::{Build, Product};
    for reversed_sources in [false, true] {
        for reversed_declarations in [false, true] {
            let helper_source = if reversed_sources { 1 } else { 2 };
            let mut sources = SourceMap::default();
            for source_ordinal in 0..3 {
                let package_marker = if source_ordinal == 0 { 1 } else { 2 };
                let root = PathBuf::from(format!("package-{package_marker}"));
                sources.add_with_metadata(
                    root.join(if source_ordinal == 0 {
                        "build.omg"
                    } else {
                        "generated.omg"
                    }),
                    String::from("Item"),
                    root,
                    Some(PackageKeyIdentity::from_digest([package_marker; 32]).unwrap()),
                    SourceOrigin::User,
                );
            }
            let mut files = sources.files().cloned().collect::<Vec<_>>();
            for (source_ordinal, file) in files.iter_mut().enumerate() {
                file.dependency_scope = if source_ordinal == 0 || source_ordinal == helper_source {
                    Build
                } else {
                    Product
                };
            }
            let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
                Some(Arc::new(SourceMap::from_files(files))),
                vec![SourceScopedTopLevelBinding::module_import(
                    SourceId(0),
                    SourceId(helper_source),
                    "dependency::generated",
                    1,
                )],
            );
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let order = if reversed_declarations {
                [2, 1]
            } else {
                [1, 2]
            };
            let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
                root,
                order.iter().map(|&source_ordinal| {
                    (
                        SymbolKind::Data,
                        SymbolNameRef::OwnedSource {
                            value: "Item",
                            source_span: reference(source_ordinal),
                        },
                    )
                }),
            ))
            .collect::<Vec<_>>();
            let symbols = builder.finish();
            let expected = declarations[order
                .iter()
                .position(|&source| source == helper_source)
                .unwrap()];
            assert_eq!(data_reference(&symbols, 0, "Item"), Some(expected));
            assert_eq!(
                symbols.lookup_signature_free_top_level_from_source_matching(
                    "Item",
                    &[SymbolKind::Data],
                    reference(0),
                    |_| true,
                ),
                crate::SymbolLookup::Unique(expected)
            );
        }
    }
}

#[test]
fn module_import_selects_namespace_without_importing_its_leaves() {
    let bindings = vec![SourceScopedTopLevelBinding::module_import(
        SourceId(0),
        SourceId(1),
        "alpha::types",
        0,
    )];
    let (mut symbols, declarations, _) = namespace_table([1, 1, 1], bindings);
    register_module(&mut symbols, 1, &["alpha", "types"]);
    register_module(&mut symbols, 2, &["beta"]);
    assert_eq!(data_reference(&symbols, 0, "Item"), None);
    assert_eq!(
        data_reference(&symbols, 0, "types::Item"),
        Some(declarations[1])
    );
    assert!(
        symbols
            .validate_source_module_import(SourceId(0), "alpha::types")
            .is_ok()
    );
}

#[test]
fn mismatched_unused_import_rejects_the_loaded_sources_namespace() {
    let bindings = vec![SourceScopedTopLevelBinding::module_import(
        SourceId(0),
        SourceId(1),
        "wrong::Item",
        0,
    )];
    let (mut symbols, _, _) = namespace_table([1, 1, 1], bindings);
    register_module(&mut symbols, 1, &["actual"]);
    assert!(
        symbols
            .validate_source_module_import(SourceId(0), "wrong::Item")
            .is_err()
    );
}

fn attached_table(
    names: [&str; 3],
    bindings: Vec<SourceScopedTopLevelBinding>,
) -> (SymbolTable, [SymbolHandle; 3]) {
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(None, bindings);
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        names.into_iter().enumerate().map(|(ordinal, name)| {
            (
                SymbolKind::Const,
                SymbolNameRef::OwnedSource {
                    value: name,
                    source_span: reference(ordinal),
                },
            )
        }),
    ))
    .collect::<Vec<_>>();
    let mut symbols = builder.finish();
    register_module(&mut symbols, 1, &["settings"]);
    register_module(&mut symbols, 2, &["settings"]);
    (
        symbols,
        declarations
            .try_into()
            .expect("three attached declarations"),
    )
}

#[test]
fn attached_path_precedence_preserves_local_ambiguity_and_eligibility() {
    let (symbols, declarations) =
        attached_table(["Sizes::SIZE", "Sizes::SIZE", "Other::SIZE"], Vec::new());
    let lookup = |symbols: &SymbolTable, source| {
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Sizes::SIZE",
            &[SymbolKind::Const],
            reference(source),
        )
    };
    assert_eq!(lookup(&symbols, 0), Some(declarations[0]));
    assert_eq!(lookup(&symbols, 1), Some(declarations[1]));
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Sizes::SIZE",
            &[SymbolKind::Const],
            SourceSpan::new(SourceId(1), Span::new(0, 0)),
        ),
        None,
        "source-free spelling cannot acquire the new authored local precedence",
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source_matching(
            "Sizes::SIZE",
            &[SymbolKind::Const],
            reference(1),
            |candidate| candidate != declarations[1]
        ),
        Some(declarations[0])
    );
    let (symbols, _) = attached_table(["Sizes::SIZE"; 3], Vec::new());
    assert_eq!(lookup(&symbols, 1), None);
}

fn domain_reference(
    symbols: &SymbolTable,
    source_ordinal: usize,
    name: &str,
) -> Option<SymbolHandle> {
    symbols.find_top_level_by_name_and_kinds_from_source(
        name,
        &[SymbolKind::Domain],
        reference(source_ordinal),
    )
}

#[test]
fn contested_carrier_qualified_spellings_keep_the_not_found_result() {
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(None, Vec::new());
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let mut handles = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
            (
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "u64::Distance",
                    source_span: reference(1),
                },
            ),
            (
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "u64::Distance",
                    source_span: reference(2),
                },
            ),
        ],
    ));
    handles.next().expect("u64 carrier");
    let first = handles.next().expect("units u64::Distance domain");
    handles.next().expect("foreign u64::Distance domain");
    let mut symbols = builder.finish();
    register_module(&mut symbols, 1, &["units"]);
    register_module(&mut symbols, 2, &["foreign"]);
    symbols.register_source_import(SourceId(0), "units");
    symbols.register_source_import(SourceId(0), "foreign");
    assert_eq!(
        domain_reference(&symbols, 0, "u64::Distance"),
        None,
        "two exposed same-spelled domains stay contested rather than selecting one",
    );
    assert_eq!(
        domain_reference(&symbols, 0, "units::u64::Distance"),
        Some(first),
        "the exact qualified path still selects its declaration",
    );
}

fn generic_domain_table() -> (SymbolTable, SymbolHandle, SymbolHandle) {
    let mut sources = SourceMap::default();
    for name in ["main.omg", "local.omg", "bounds.omg"] {
        sources.add(PathBuf::from(name), "Below".to_owned());
    }
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let mut declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
            (
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "Below",
                    source_span: reference(1),
                },
            ),
            (
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "Below",
                    source_span: reference(2),
                },
            ),
        ],
    ));
    declarations.next().expect("builtin carrier");
    let local = declarations.next().expect("local generic family");
    let foreign = declarations.next().expect("foreign generic family");
    let mut symbols = builder.finish();
    register_module(&mut symbols, 2, &["bounds"]);
    (symbols, local, foreign)
}

#[test]
fn qualified_generic_domain_does_not_select_a_root_or_module_local_leaf() {
    for module_local in [false, true] {
        let (mut symbols, local, foreign) = generic_domain_table();
        if module_local {
            register_module(&mut symbols, 1, &["local"]);
        }
        assert!(symbols.domain_name_reaches(foreign, "bounds::Below", reference(1)));
        assert!(
            !symbols.domain_name_reaches(local, "bounds::Below", reference(1)),
            "the foreign module qualifier must not reach a same-leaf generic family"
        );
        assert!(
            symbols.domain_name_reaches(local, "u64::Below", reference(1)),
            "a builtin carrier qualifier still reaches the local generic family"
        );
        assert!(symbols.domain_name_reaches(local, "Below", reference(1)));
        assert_eq!(
            domain_reference(&symbols, 1, "bounds::Below"),
            Some(foreign)
        );
    }
}
