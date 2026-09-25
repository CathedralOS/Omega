use super::{main_entry_symbols, main_running_inventory_symbols};
use crate::{
    SourceScopedTopLevelBinding, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable,
    SymbolTableBuilder,
};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin, SourceResolutionStratum, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn invalid_symbol_resolves_to_dummy() {
    let symbols = SymbolTable::new();
    let invalid = SymbolHandle::invalid();

    assert_eq!(symbols.get(invalid).kind, SymbolKind::Unknown);
    assert_eq!(symbols.name(invalid), "");
}

#[test]
fn stores_symbols_with_parent_handles() {
    let symbols = main_entry_symbols();
    let root = symbols.root();
    let machine = symbols
        .find_child_by_name(root, "main")
        .expect("main should resolve");
    let state = symbols
        .find_child_by_name(machine, "entry")
        .expect("entry should resolve");

    assert_eq!(symbols.get(machine).parent, root);
    assert_eq!(symbols.get(state).parent, machine);
    assert_eq!(symbols.get(root).children.count(), 1);
    assert_eq!(symbols.get(machine).children.count(), 1);
    assert_eq!(symbols.name(state), "entry");
}

#[test]
fn generated_hierarchy_appends_without_moving_authored_symbols() {
    let mut symbols = main_entry_symbols();
    let authored_root = symbols.root();
    let authored_machine = symbols
        .find_child_by_name(authored_root, "main")
        .expect("authored machine");
    let authored_entry = symbols
        .find_child_by_name(authored_machine, "entry")
        .expect("authored entry");

    let generated = symbols.insert_generated_root_from(
        authored_machine,
        SymbolKind::Machine,
        "map$specialized",
    );
    let children = symbols.insert_generated_children(
        generated,
        [(SymbolKind::State, "entry"), (SymbolKind::State, "next")],
    );
    let generated_children = symbols
        .child_handles(generated)
        .expect("generated children")
        .collect::<Vec<_>>();

    assert_eq!(generated_children.len(), 2);
    assert_eq!(children.count(), 2);
    assert_eq!(symbols.name(generated), "map$specialized");
    assert_eq!(symbols.name(generated_children[0]), "entry");
    assert_eq!(symbols.get(generated_children[0]).parent, generated);
    assert_eq!(symbols.name(authored_machine), "main");
    assert_eq!(symbols.name(authored_entry), "entry");
    assert_eq!(symbols.get(authored_entry).parent, authored_machine);
}

#[test]
fn authored_extension_appends_without_mutating_base_symbol_identity() {
    let mut sources = SourceMap::default();
    let base_source = sources
        .add(PathBuf::from("base.omg"), String::from("Base"))
        .source_id;
    let shadow_source = sources
        .add(PathBuf::from("shadow.omg"), String::from("Base"))
        .source_id;
    let reference_source = sources
        .add(PathBuf::from("reference.omg"), String::from("Base"))
        .source_id;
    let extension_source = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("generated.omg"),
            String::from("Extension"),
            PathBuf::from("."),
            None,
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(Arc::new(sources.clone())),
        vec![SourceScopedTopLevelBinding::new(
            reference_source,
            shadow_source,
            "Base",
        )],
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let base_declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(base_source, Span::new(0, 4))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(shadow_source, Span::new(0, 4))),
            ),
        ],
    ))
    .collect::<Vec<_>>();
    let [base, shadow] = base_declarations.as_slice() else {
        panic!("two base declarations")
    };
    let base_member = SymbolTableBuilder::child_handles(builder.insert_children(
        *base,
        [(SymbolKind::Field, SymbolNameRef::Static("member"))],
    ))
    .next()
    .expect("base member");
    let base_snapshot = (*base, *shadow, base_member);

    let mut extension = builder
        .finish()
        .begin_extension(Some(Arc::new(sources)), Vec::new());
    let generated = extension.insert_top_level([(
        SymbolKind::Data,
        SymbolNameRef::Source(SourceSpan::new(extension_source, Span::new(0, 9))),
    )]);
    let [generated] = generated.as_slice() else {
        panic!("one extension symbol")
    };
    let symbols = extension.finish();

    assert_eq!((*base, *shadow, base_member), base_snapshot);
    assert_eq!(symbols.find_child_by_name(root, "Base"), Some(*base));
    assert_eq!(
        symbols.find_child_by_name(root, "Extension"),
        Some(*generated)
    );
    assert_eq!(
        symbols.find_child_by_name(*base, "member"),
        Some(base_member)
    );
    assert_eq!(symbols.get(base_member).parent, *base);
    assert_eq!(symbols.get(*generated).parent, root);
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Base",
            &[SymbolKind::Data],
            SourceSpan::new(reference_source, Span::new(0, 4)),
        ),
        Some(*shadow),
        "extension must preserve base source-scoped bindings",
    );
}

#[test]
fn current_activation_extension_is_invisible_to_base_but_sees_the_complete_extension() {
    let package_identity =
        PackageKeyIdentity::from_digest([9; 32]).expect("nonzero package identity");
    let mut sources = SourceMap::default();
    let base_source = sources
        .add_with_metadata(
            PathBuf::from("package/main.omg"),
            String::from("Value"),
            PathBuf::from("package"),
            Some(package_identity),
            SourceOrigin::User,
        )
        .source_id;
    let first_extension = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("package/.omega/generated/first.omg"),
            String::from("Value ExtensionOnly"),
            PathBuf::from("package"),
            Some(package_identity),
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let second_extension = sources
        .add_with_metadata_and_resolution_stratum(
            PathBuf::from("package/.omega/generated/second.omg"),
            String::from("use"),
            PathBuf::from("package"),
            Some(package_identity),
            SourceOrigin::User,
            SourceResolutionStratum::CurrentActivationExtension,
        )
        .source_id;
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(base_source, Span::new(0, 5))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(first_extension, Span::new(0, 5))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(first_extension, Span::new(6, 19))),
            ),
        ],
    ))
    .collect::<Vec<_>>();
    let symbols = builder.finish();
    let reference = |source_id| SourceSpan::new(source_id, Span::new(0, 1));

    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Value",
            &[SymbolKind::Data],
            reference(base_source),
        ),
        Some(declarations[0]),
    );
    assert!(!symbols.source_reference_can_see_symbol(reference(base_source), declarations[1],));
    assert!(symbols.source_reference_can_see_symbol(reference(second_extension), declarations[0],));
    assert!(symbols.source_reference_can_see_symbol(reference(second_extension), declarations[1],));
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Value",
            &[SymbolKind::Data],
            reference(second_extension),
        ),
        Some(declarations[1]),
        "an extension unit must prefer its shared extension stratum over Base",
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "ExtensionOnly",
            &[SymbolKind::Data],
            reference(second_extension),
        ),
        Some(declarations[2]),
    );
    assert_eq!(
        symbols.find_top_level_by_name_and_kinds_from_source(
            "ExtensionOnly",
            &[SymbolKind::Data],
            SourceSpan::default(),
        ),
        Some(declarations[2]),
        "source-free focused consumers retain their permissive lookup behavior",
    );
    assert!(symbols.source_reference_can_see_symbol(SourceSpan::default(), declarations[1],));
    assert!(symbols.source_scopes_separate(declarations[0], declarations[1]));
    assert_eq!(
        symbols.symbol_package_identity(declarations[1]),
        Some(package_identity),
        "resolution stratum must not alter package provenance",
    );
}

#[test]
fn finds_child_by_name_and_kind_when_siblings_share_names() {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    builder.insert_children(
        root,
        [
            (SymbolKind::Data, SymbolNameRef::Borrowed("main")),
            (SymbolKind::Machine, SymbolNameRef::Borrowed("main")),
        ],
    );
    let symbols = builder.finish();
    let machine = symbols
        .find_child_by_name_and_kind(root, "main", SymbolKind::Machine)
        .expect("machine main should resolve by kind");

    assert_eq!(symbols.get(machine).kind, SymbolKind::Machine);
    assert_eq!(
        symbols.find_child_by_name_and_kind(root, "main", SymbolKind::State),
        None
    );
}

#[test]
fn builder_stores_symbols_without_definition_tree() {
    let mut builder = SymbolTableBuilder::new();
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let children = builder.insert_children(
        root,
        [
            (SymbolKind::Machine, SymbolNameRef::Borrowed("main")),
            (SymbolKind::Data, SymbolNameRef::Borrowed("Inventory")),
        ],
    );
    let mut child_handles = SymbolTableBuilder::child_handles(children);
    let main = child_handles.next().expect("main should be present");
    let inventory = child_handles.next().expect("inventory should be present");

    builder.insert_children(
        main,
        [(SymbolKind::State, SymbolNameRef::Borrowed("entry"))],
    );
    let symbols = builder.finish();
    let entry = symbols
        .find_child_by_name(main, "entry")
        .expect("entry should resolve");

    assert_eq!(symbols.root(), root);
    assert_eq!(symbols.get(main).parent, root);
    assert_eq!(symbols.get(inventory).parent, root);
    assert_eq!(symbols.get(entry).parent, main);
    assert_eq!(symbols.name(inventory), "Inventory");
}

#[test]
fn child_ranges_are_exact_per_parent() {
    let symbols = main_running_inventory_symbols();
    let root = symbols.root();
    let root_children = symbols
        .child_handles(root)
        .expect("root children should resolve")
        .map(|child| symbols.name(child).to_owned())
        .collect::<Vec<_>>();
    let main = symbols
        .find_child_by_name(root, "main")
        .expect("main should resolve");
    let main_children = symbols
        .child_handles(main)
        .expect("main children should resolve")
        .map(|child| symbols.name(child).to_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        root_children,
        vec!["main".to_owned(), "Inventory".to_owned()]
    );
    assert_eq!(
        main_children,
        vec!["entry".to_owned(), "running".to_owned()]
    );
}

/// A loaded source exposes nothing merely by being loaded: a reference
/// selects declarations from its own source, its checked package instance,
/// and the sources its file imported.
#[test]
fn reference_selects_own_package_and_imported_declarations_only() {
    let mut sources = SourceMap::default();
    let main = sources
        .add_with_metadata(
            PathBuf::from("app/main.omg"),
            String::from("Nat"),
            PathBuf::from("app"),
            None,
            SourceOrigin::User,
        )
        .source_id;
    let sibling = sources
        .add_with_metadata(
            PathBuf::from("app/helper.omg"),
            String::from("Nat"),
            PathBuf::from("app"),
            None,
            SourceOrigin::User,
        )
        .source_id;
    let entry_contract = sources
        .add_with_metadata(
            PathBuf::from("toolchain/std/targets/host/entry.omg"),
            String::from("use"),
            PathBuf::from("toolchain/std"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let bundled = sources
        .add_with_metadata(
            PathBuf::from("toolchain/core/nat.omg"),
            String::from("Nat"),
            PathBuf::from("toolchain/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let imported = sources
        .add_with_metadata(
            PathBuf::from("toolchain/core/slice.omg"),
            String::from("Nat"),
            PathBuf::from("toolchain/core"),
            None,
            SourceOrigin::Toolchain,
        )
        .source_id;
    let bindings = vec![SourceScopedTopLevelBinding::module_import(
        main,
        imported,
        "omega::language::core::slice",
        3,
    )];
    let mut builder =
        SymbolTableBuilder::with_sources_and_top_level_bindings(Some(Arc::new(sources)), bindings);
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        [
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(main, Span::new(0, 3))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(sibling, Span::new(0, 3))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(bundled, Span::new(0, 3))),
            ),
            (
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(imported, Span::new(0, 3))),
            ),
        ],
    ))
    .collect::<Vec<_>>();
    let symbols = builder.finish();
    let reference = |source_id| SourceSpan::new(source_id, Span::new(0, 1));

    let [own, package_sibling, loaded_only, imported_declaration] = declarations.as_slice() else {
        panic!("four declarations");
    };
    assert!(symbols.source_reference_may_select_symbol(reference(main), *own));
    assert!(symbols.source_reference_may_select_symbol(reference(main), *package_sibling));
    assert!(symbols.source_reference_may_select_symbol(reference(main), *imported_declaration));
    assert!(
        !symbols.source_reference_may_select_symbol(reference(main), *loaded_only),
        "a bundled source reached only through another source's imports exposes nothing"
    );
    assert!(
        !symbols.source_reference_may_select_symbol(reference(entry_contract), *own),
        "the toolchain contract does not select the program's declarations either"
    );
    assert!(
        !symbols.source_reference_may_select_symbol(reference(entry_contract), *loaded_only),
        "one bundled root does not expose another bundled root without an import"
    );
    assert!(
        symbols.source_reference_may_select_symbol(SourceSpan::default(), *loaded_only),
        "source-free consumers keep the whole program"
    );
}
