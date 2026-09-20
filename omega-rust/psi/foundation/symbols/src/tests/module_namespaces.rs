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
fn unmoduled_exact_imports_preserve_local_precedence() {
    let (symbols, declarations, _) = namespace_table(
        [1, 1, 2],
        vec![SourceScopedTopLevelBinding::module_import(
            SourceId(0),
            SourceId(2),
            "dependency::source",
            1,
        )],
    );
    assert_eq!(data_reference(&symbols, 0, "Item"), Some(declarations[1]));
    assert_eq!(
        symbols.lookup_signature_free_top_level_from_source_matching(
            "Item",
            &[SymbolKind::Data],
            reference(0),
            |_| true,
        ),
        crate::SymbolLookup::Unique(declarations[1])
    );
}

#[test]
fn unmoduled_fallback_retains_same_scope_candidates_for_package_admission() {
    let (symbols, declarations, _) = namespace_table(
        [1, 2, 3],
        vec![SourceScopedTopLevelBinding::module_import(
            SourceId(0),
            SourceId(0),
            "local",
            0,
        )],
    );
    assert_eq!(
        symbols.lookup_signature_free_top_level_from_source_matching(
            "Item",
            &[SymbolKind::Data],
            reference(0),
            |candidate| candidate == declarations[2],
        ),
        crate::SymbolLookup::Unique(declarations[2]),
        "unmoduled discovery preserves candidates for the separate package-admission check"
    );
}

#[test]
fn unmoduled_missing_imported_name_does_not_select_another_scope() {
    let (_, _, sources) = namespace_table([1, 2, 2], Vec::new());
    let mut files = sources.files().cloned().collect::<Vec<_>>();
    files[0].dependency_scope = source::DependencyScope::Build;
    files[1].dependency_scope = source::DependencyScope::Build;
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(Arc::new(SourceMap::from_files(files))),
        vec![SourceScopedTopLevelBinding::module_import(
            SourceId(0),
            SourceId(1),
            "dependency::generated",
            1,
        )],
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    builder.insert_children(
        root,
        ["Local", "Other", "Item"]
            .iter()
            .enumerate()
            .map(|(source_ordinal, name)| {
                (
                    SymbolKind::Data,
                    SymbolNameRef::OwnedSource {
                        value: name,
                        source_span: reference(source_ordinal),
                    },
                )
            }),
    );
    let symbols = builder.finish();
    assert_eq!(data_reference(&symbols, 0, "Item"), None);
    assert_eq!(
        symbols.lookup_signature_free_top_level_from_source_matching(
            "Item",
            &[SymbolKind::Data],
            reference(0),
            |_| true,
        ),
        crate::SymbolLookup::NotFound
    );
}

#[test]
fn root_namespace_does_not_acquire_unimported_module_leaves() {
    let (mut symbols, declarations, _) = namespace_table([1, 1, 1], Vec::new());
    register_module(&mut symbols, 1, &["alpha"]);
    register_module(&mut symbols, 2, &["beta"]);

    assert_eq!(data_reference(&symbols, 0, "Item"), None);
    assert_eq!(
        data_reference(&symbols, 0, "alpha::Item"),
        Some(declarations[1])
    );
    assert_eq!(data_reference(&symbols, 1, "Item"), Some(declarations[1]));
    assert_eq!(data_reference(&symbols, 2, "Item"), Some(declarations[2]));
    assert!(symbols.source_scopes_separate(declarations[1], declarations[2]));
}

#[test]
fn signature_free_synthetic_paths_do_not_acquire_a_module_from_source_id() {
    let mut sources = SourceMap::default();
    for name in ["alpha.omg", "beta.omg"] {
        sources.add(PathBuf::from(name), "Token::issue".to_owned());
    }
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        (0..2).map(|source_ordinal| {
            (
                SymbolKind::Machine,
                SymbolNameRef::OwnedSource {
                    value: "Token::issue",
                    source_span: reference(source_ordinal),
                },
            )
        }),
    ))
    .collect::<Vec<_>>();
    let mut symbols = builder.finish();
    register_module(&mut symbols, 0, &["alpha"]);
    register_module(&mut symbols, 1, &["beta"]);
    assert_eq!(
        symbols.lookup_signature_free_top_level_from_source_matching(
            "Token::issue",
            &[SymbolKind::Machine],
            reference(0),
            |_| true,
        ),
        crate::SymbolLookup::Unique(declarations[0]),
    );
    for source_ordinal in 0..2 {
        assert!(matches!(
            symbols.lookup_signature_free_top_level_from_source_matching(
                "Token::issue",
                &[SymbolKind::Machine],
                SourceSpan::new(SourceId(source_ordinal), Span::new(0, 0)),
                |_| true,
            ),
            crate::SymbolLookup::Ambiguous { .. }
        ));
    }
}

#[test]
fn explicit_leaf_imports_select_exact_symbols_and_ambiguity_rejects() {
    let bindings = vec![SourceScopedTopLevelBinding::module_import(
        SourceId(0),
        SourceId(1),
        "alpha::Item",
        0,
    )];
    let (mut symbols, declarations, _) = namespace_table([1, 1, 1], bindings);
    register_module(&mut symbols, 1, &["alpha"]);
    register_module(&mut symbols, 2, &["beta"]);
    assert_eq!(data_reference(&symbols, 0, "Item"), Some(declarations[1]));
    symbols.register_source_import(SourceId(0), "beta::Item");
    assert_eq!(data_reference(&symbols, 0, "Item"), None);
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
fn equal_module_spelling_retains_exact_package_owners() {
    let bindings = vec![
        SourceScopedTopLevelBinding::module_import(
            SourceId(0),
            SourceId(1),
            "left::types::Item",
            1,
        ),
        SourceScopedTopLevelBinding::module_import(
            SourceId(0),
            SourceId(2),
            "right::types::Item",
            1,
        ),
    ];
    let (mut symbols, declarations, _) = namespace_table([1, 2, 3], bindings);
    register_module(&mut symbols, 1, &["types"]);
    register_module(&mut symbols, 2, &["types"]);
    assert_ne!(
        symbols.source_module(SourceId(1)),
        symbols.source_module(SourceId(2))
    );
    assert_eq!(
        data_reference(&symbols, 0, "left::types::Item"),
        Some(declarations[1])
    );
    assert_eq!(
        data_reference(&symbols, 0, "right::types::Item"),
        Some(declarations[2])
    );
    assert_eq!(data_reference(&symbols, 0, "Item"), None);
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

#[test]
fn namespace_extension_and_generated_symbols_retain_base_handles() {
    let (mut symbols, declarations, sources) = namespace_table([1, 1, 1], Vec::new());
    register_module(&mut symbols, 1, &["alpha"]);
    let module = symbols.source_module(SourceId(1));
    let root_children = symbols
        .child_handles(symbols.root())
        .expect("root children")
        .collect::<Vec<_>>();
    let generated =
        symbols.insert_generated_root_from(declarations[1], SymbolKind::Data, "Generated");
    assert_eq!(symbols.display_path(generated, "::"), "alpha::Generated");
    let extension = symbols.begin_extension(Some(sources), Vec::new());
    let mut symbols = extension.finish();
    register_module(&mut symbols, 2, &["alpha"]);
    assert_eq!(symbols.source_module(SourceId(2)), module);
    assert_eq!(symbols.symbol_module(generated), module);
    assert_eq!(
        symbols
            .child_handles(symbols.root())
            .expect("root children")
            .take(root_children.len())
            .collect::<Vec<_>>(),
        root_children
    );
    assert_eq!(symbols.display_path(declarations[1], "::"), "alpha::Item");
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
fn narrow_imports_expose_attached_leaves_without_implicit_siblings() {
    let (mut symbols, declarations) =
        attached_table(["Root::SIZE", "Sizes::SIZE", "Other::SIZE"], Vec::new());
    let lookup = |symbols: &SymbolTable, name: &str| {
        symbols.find_top_level_by_name_and_kinds_from_source(
            name,
            &[SymbolKind::Const],
            reference(0),
        )
    };
    assert_eq!(lookup(&symbols, "SIZE"), None);
    symbols.register_source_import(SourceId(0), "settings");
    assert_eq!(lookup(&symbols, "SIZE"), None);
    symbols.register_source_import(SourceId(0), "settings::Sizes::SIZE");
    assert_eq!(lookup(&symbols, "SIZE"), Some(declarations[1]));
    symbols.register_source_import(SourceId(0), "settings::Other::SIZE");
    assert_eq!(lookup(&symbols, "SIZE"), None);
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

#[test]
fn attached_leaves_do_not_become_source_free_or_legacy_vocabulary_names() {
    let (symbols, _) = attached_table(
        ["Root::SIZE", "Sizes::SIZE", "Other::SIZE"],
        vec![SourceScopedTopLevelBinding::new(
            SourceId(0),
            SourceId(1),
            "SIZE",
        )],
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "SIZE",
            &[SymbolKind::Const],
            SourceSpan::default()
        ),
        None
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "SIZE",
            &[SymbolKind::Const],
            reference(0)
        ),
        None
    );
}

/// One moduled domain `units::u64::Distance` declared in source 1 beside the
/// root `u64` carrier. Source 0 owns a same-named local `Meters` that shadows
/// the declaring source's `units::Meters`, so `Meters::...` spellings from
/// source 0 name a different carrier than `units::Meters::Depth` attached to.
fn domain_table(
    bindings: Vec<SourceScopedTopLevelBinding>,
    modules: &[&[&str]],
) -> (SymbolTable, SymbolHandle) {
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(None, bindings);
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let mut handles = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
            (
                SymbolKind::Data,
                SymbolNameRef::OwnedSource {
                    value: "Meters",
                    source_span: reference(0),
                },
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::OwnedSource {
                    value: "Meters",
                    source_span: reference(1),
                },
            ),
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
                    value: "Meters::Depth",
                    source_span: reference(1),
                },
            ),
        ],
    ));
    handles.next().expect("u64 carrier");
    handles.next().expect("local Meters carrier");
    handles.next().expect("units Meters carrier");
    let domain = handles.next().expect("u64::Distance domain");
    handles.next().expect("Meters::Depth domain");
    let mut symbols = builder.finish();
    for (ordinal, path) in modules.iter().enumerate() {
        register_module(&mut symbols, ordinal + 1, path);
    }
    (symbols, domain)
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
fn broad_module_import_exposes_carrier_qualified_domain_spelling() {
    let (mut symbols, domain) = domain_table(Vec::new(), &[&["units"]]);
    assert_eq!(domain_reference(&symbols, 0, "u64::Distance"), None);
    symbols.register_source_import(SourceId(0), "units");
    assert_eq!(
        domain_reference(&symbols, 0, "units::u64::Distance"),
        Some(domain),
        "the complete qualified path always selects its declaration",
    );
    assert_eq!(
        domain_reference(&symbols, 0, "u64::Distance"),
        Some(domain),
        "importing the declaring module exposes the carrier-qualified spelling",
    );
    assert_eq!(
        domain_reference(&symbols, 0, "Distance"),
        None,
        "a broad import never makes the leaf a bare local name",
    );
}

#[test]
fn narrow_domain_import_exposes_carrier_qualified_spelling() {
    let (mut symbols, domain) = domain_table(Vec::new(), &[&["units"]]);
    symbols.register_source_import(SourceId(0), "units::u64::Distance");
    assert_eq!(
        domain_reference(&symbols, 0, "u64::Distance"),
        Some(domain),
        "a narrow import of the exact declaration exposes its carrier-qualified spelling",
    );
    assert_eq!(domain_reference(&symbols, 0, "Distance"), Some(domain));
}

#[test]
fn carrier_qualified_spelling_needs_exact_carrier_and_exposing_import() {
    let (symbols, _) = domain_table(Vec::new(), &[&["units"]]);
    assert_eq!(
        domain_reference(&symbols, 0, "u64::Distance"),
        None,
        "loading the declaring source without an import exposes nothing",
    );
    let (mut symbols, domain) = domain_table(Vec::new(), &[&["units"], &["other"]]);
    symbols.register_source_import(SourceId(0), "other");
    assert_eq!(
        domain_reference(&symbols, 0, "u64::Distance"),
        None,
        "an unrelated module import does not expose the domain",
    );
    symbols.register_source_import(SourceId(0), "units");
    assert_eq!(
        domain_reference(&symbols, 0, "u64::Depth"),
        None,
        "the leaf must match the declared carrier-qualified name",
    );
    assert_eq!(
        domain_reference(&symbols, 0, "Meters::Distance"),
        None,
        "a same-leaf spelling whose carrier resolves to a different declaration rejects",
    );
    assert_eq!(domain_reference(&symbols, 0, "u64::Distance"), Some(domain),);
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

#[test]
fn generated_references_carry_no_carrier_qualified_exposure() {
    let (mut symbols, domain) = domain_table(Vec::new(), &[&["units"]]);
    symbols.register_source_import(SourceId(0), "units");
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "u64::Distance",
            &[SymbolKind::Domain],
            SourceSpan::new(SourceId(0), Span::new(0, 0)),
        ),
        None,
        "source-free references carry no lexical module or import exposure",
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "units::u64::Distance",
            &[SymbolKind::Domain],
            SourceSpan::new(SourceId(0), Span::new(0, 0)),
        ),
        Some(domain),
        "generated references still select the complete qualified path",
    );
}

#[test]
fn package_qualification_does_not_relax_exact_import_source_validation() {
    let bindings = vec![SourceScopedTopLevelBinding::module_import(
        SourceId(0),
        SourceId(1),
        "dep::beta::Item",
        1,
    )];
    let (mut symbols, declarations, _) = namespace_table([1, 2, 2], bindings);
    register_module(&mut symbols, 1, &["alpha"]);
    register_module(&mut symbols, 2, &["beta"]);
    assert_eq!(
        data_reference(&symbols, 0, "dep::beta::Item"),
        Some(declarations[2])
    );
    assert!(
        symbols
            .validate_source_module_import(SourceId(0), "dep::beta::Item")
            .is_err(),
        "the sibling declaration cannot satisfy an import bound to the wrong source"
    );
    assert_eq!(
        symbols.source_module_import_target(SourceId(0), "dep::beta::Item"),
        None
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

#[test]
fn generic_domain_leaf_exposure_keeps_exact_narrow_imports() {
    let (mut symbols, local, foreign) = generic_domain_table();
    register_module(&mut symbols, 1, &["local"]);
    assert!(!symbols.domain_name_reaches(foreign, "Below", reference(0)));
    symbols.register_source_import(SourceId(0), "bounds::Below");
    assert!(symbols.domain_name_reaches(foreign, "Below", reference(0)));
    assert!(!symbols.domain_name_reaches(local, "Below", reference(0)));
    assert!(symbols.domain_name_reaches(foreign, "bounds::Below", reference(0)));
    assert!(
        !symbols.domain_name_reaches(local, "bounds::Below", reference(0)),
        "a narrow import retains the selected owner"
    );
}
