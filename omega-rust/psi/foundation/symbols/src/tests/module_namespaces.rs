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
