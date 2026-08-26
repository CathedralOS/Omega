#![forbid(unsafe_code)]

//! Target-neutral symbol identities, names, paths, and hierarchy storage.

mod builtin;
mod kind;
mod name;
mod path;
mod symbol;
mod table;

pub use builtin::{
    BUILTIN_TYPE_COUNT, BuiltinFunction, BuiltinType, BuiltinTypeAtom, builtin_function_symbols,
    builtin_type_member_symbols, builtin_type_symbols,
};
pub use kind::SymbolKind;
pub use name::{SymbolName, SymbolNameRef, SymbolNameStorageKind};
pub use path::SymbolPath;
pub use symbol::{Symbol, SymbolHandle, SymbolNameHandle, SymbolSpan};
pub use table::{SymbolNameStorageCounts, SymbolTable, SymbolTableBuilder};

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use psi_core::PackageKeyIdentity;
    use psi_source::{SourceMap, SourceOrigin, SourceSpan, Span};

    use super::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};

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
    fn resolves_managed_authored_symbol_package_identity() {
        let package_identity =
            PackageKeyIdentity::from_digest([1; 32]).expect("nonzero package identity");
        let (symbols, authored) =
            sourced_symbol_table([(SourceOrigin::User, Some(package_identity))]);

        assert_eq!(
            symbols.symbol_package_identity(authored[0]),
            Some(package_identity)
        );
    }

    #[test]
    fn source_free_structural_children_inherit_authored_parent_provenance() {
        let package_identity =
            PackageKeyIdentity::from_digest([4; 32]).expect("nonzero package identity");
        let mut sources = SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("package/main.omg"),
                String::from("machine"),
                PathBuf::from("package"),
                Some(package_identity),
                SourceOrigin::User,
            )
            .source_id;
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let machine = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [(
                SymbolKind::Machine,
                SymbolNameRef::Source(SourceSpan::new(source_id, Span::new(0, 7))),
            )],
        ))
        .next()
        .expect("authored machine");
        let state = SymbolTableBuilder::child_handles(builder.insert_children(
            machine,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        ))
        .next()
        .expect("implicit state");
        let symbols = builder.finish();

        assert_eq!(
            symbols.symbol_package_identity(state),
            Some(package_identity)
        );
        assert_eq!(
            symbols.symbol_source_origin(state),
            Some(SourceOrigin::User)
        );
    }

    #[test]
    fn owned_semantic_symbol_name_retains_authored_provenance() {
        let package_identity =
            PackageKeyIdentity::from_digest([5; 32]).expect("nonzero package identity");
        let mut sources = SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("package/main.omg"),
                String::from("Ready"),
                PathBuf::from("package"),
                Some(package_identity),
                SourceOrigin::User,
            )
            .source_id;
        let source_span = SourceSpan::new(source_id, Span::new(0, 5));
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let domain = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [(
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "Packet::Ready",
                    source_span,
                },
            )],
        ))
        .next()
        .expect("authored semantic domain");
        let symbols = builder.finish();

        assert_eq!(symbols.name(domain), "Packet::Ready");
        assert_eq!(symbols.symbol_source_span(domain), Some(source_span));
        assert_eq!(
            symbols.symbol_package_identity(domain),
            Some(package_identity)
        );
    }

    #[test]
    fn unmanaged_and_toolchain_symbols_have_no_package_identity() {
        let package_identity =
            PackageKeyIdentity::from_digest([2; 32]).expect("nonzero package identity");
        let (symbols, authored) = sourced_symbol_table([
            (SourceOrigin::User, None),
            (SourceOrigin::Toolchain, Some(package_identity)),
        ]);

        assert_eq!(symbols.symbol_package_identity(authored[0]), None);
        assert_eq!(symbols.symbol_package_identity(authored[1]), None);
    }

    #[test]
    fn generated_symbols_inherit_authored_package_identity() {
        let package_identity =
            PackageKeyIdentity::from_digest([3; 32]).expect("nonzero package identity");
        let (mut symbols, authored) =
            sourced_symbol_table([(SourceOrigin::User, Some(package_identity))]);
        let generated =
            symbols.insert_generated_root_from(authored[0], SymbolKind::Machine, "generated");
        let generated_child = SymbolTableBuilder::child_handles(
            symbols.insert_generated_children(generated, [(SymbolKind::State, "entry")]),
        )
        .next()
        .expect("generated state");

        assert_eq!(symbols.symbol_package_identity(symbols.root()), None);
        assert_eq!(
            symbols.symbol_package_identity(generated),
            Some(package_identity)
        );
        assert_eq!(
            symbols.symbol_package_identity(generated_child),
            Some(package_identity)
        );
        assert_eq!(
            symbols.symbol_source_origin(generated_child),
            Some(SourceOrigin::User)
        );
        assert!(symbols.same_symbol_source_package(authored[0], generated_child));
    }

    #[test]
    fn generated_toolchain_symbols_retain_toolchain_origin_without_package_identity() {
        let (mut symbols, authored) = sourced_symbol_table([(SourceOrigin::Toolchain, None)]);
        let generated = symbols.insert_generated_root_from(
            authored[0],
            SymbolKind::Machine,
            "generated_toolchain_machine",
        );

        assert_eq!(
            symbols.symbol_source_origin(generated),
            Some(SourceOrigin::Toolchain)
        );
        assert_eq!(symbols.symbol_package_identity(generated), None);
    }

    #[test]
    fn preexisting_generated_symbols_bind_exact_resolved_origin() {
        let package_identity =
            PackageKeyIdentity::from_digest([6; 32]).expect("nonzero package identity");
        let mut sources = SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("package/optional.omg"),
                String::from("Optional"),
                PathBuf::from("package"),
                Some(package_identity),
                SourceOrigin::User,
            )
            .source_id;
        let source_span = SourceSpan::new(source_id, Span::new(0, 8));
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = builder.insert_children(
            root,
            [
                (SymbolKind::Data, SymbolNameRef::Source(source_span)),
                (SymbolKind::Data, SymbolNameRef::Static("Optional<u64>")),
            ],
        );
        let mut declarations = SymbolTableBuilder::child_handles(declarations);
        let generic_base = declarations.next().expect("generic base declaration");
        let concrete_instance = declarations.next().expect("concrete generic instance");
        let mut symbols = builder.finish();

        symbols.bind_generated_symbol_origin(concrete_instance, generic_base);

        assert_eq!(
            symbols.symbol_package_identity(concrete_instance),
            Some(package_identity)
        );
        assert_eq!(
            symbols.symbol_provenance_source_span(concrete_instance),
            Some(source_span)
        );
        assert!(symbols.same_symbol_source_package(generic_base, concrete_instance));
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
    fn stores_paths_as_handle_spans() {
        let mut symbols = main_entry_symbols();
        let root = symbols.root();
        let machine = symbols
            .find_child_by_name(root, "main")
            .expect("main should resolve");
        let state = symbols
            .find_child_by_name(machine, "entry")
            .expect("entry should resolve");
        let path = symbols.path_from_members(root, [machine, state]);

        assert_eq!(path.root, root);
        assert_eq!(symbols.path_members(path), &[machine, state]);
    }

    #[test]
    fn resolves_child_paths_by_sibling_walk() {
        let mut symbols = main_entry_symbols();
        let root = symbols.root();
        let path = symbols.resolve_child_path(root, ["main", "entry"]);
        let names = symbols
            .path_members(path)
            .iter()
            .map(|symbol| symbols.name(*symbol))
            .collect::<Vec<_>>();

        assert_eq!(names, vec!["main", "entry"]);
        let missing_path = symbols.resolve_child_path(root, ["main", "missing"]);
        assert!(symbols.path_members(missing_path).is_empty());
    }

    #[test]
    fn resolves_descendant_without_storing_path_members() {
        let symbols = main_entry_symbols();
        let root = symbols.root();
        let entry = symbols
            .find_descendant_by_path(root, ["main", "entry"])
            .expect("entry should resolve");

        assert_eq!(symbols.name(entry), "entry");
        assert_eq!(
            symbols.find_descendant_by_path(root, ["main", "missing"]),
            None
        );
        assert_eq!(symbols.path_member_arena().len(), 0);
    }

    #[test]
    fn formats_symbol_display_path_from_parent_chain() {
        let symbols = main_console_symbols();
        let write_line = symbols
            .find_descendant_by_path(symbols.root(), ["main", "console", "write_line"])
            .expect("write_line should resolve");

        assert_eq!(
            symbols.display_path(write_line, "::"),
            "main::console::write_line"
        );
        assert_eq!(symbols.display_path(SymbolHandle::invalid(), "::"), "");
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

    fn main_entry_symbols() -> SymbolTable {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            [(SymbolKind::Machine, SymbolNameRef::Borrowed("main"))],
        );
        let mut children = SymbolTableBuilder::child_handles(children);
        let main = children.next().expect("main should be present");
        builder.insert_children(
            main,
            [(SymbolKind::State, SymbolNameRef::Borrowed("entry"))],
        );

        builder.finish()
    }

    fn sourced_symbol_table<const N: usize>(
        metadata: [(SourceOrigin, Option<PackageKeyIdentity>); N],
    ) -> (SymbolTable, Vec<SymbolHandle>) {
        let mut sources = SourceMap::default();
        let source_spans = metadata.map(|(origin, package_identity)| {
            let source_id = sources
                .add_with_metadata(
                    PathBuf::from(format!("source-{}.omg", sources.len())),
                    String::from("machine"),
                    PathBuf::from("package"),
                    package_identity,
                    origin,
                )
                .source_id;
            SourceSpan::new(source_id, Span::new(0, 7))
        });
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            source_spans
                .map(|source_span| (SymbolKind::Machine, SymbolNameRef::Source(source_span))),
        );
        let authored = SymbolTableBuilder::child_handles(children).collect();

        (builder.finish(), authored)
    }

    fn main_console_symbols() -> SymbolTable {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            [(SymbolKind::Machine, SymbolNameRef::Borrowed("main"))],
        );
        let mut children = SymbolTableBuilder::child_handles(children);
        let main = children.next().expect("main should be present");
        let children = builder.insert_children(
            main,
            [(SymbolKind::Object, SymbolNameRef::Borrowed("console"))],
        );
        let mut children = SymbolTableBuilder::child_handles(children);
        let console = children.next().expect("console should be present");
        builder.insert_children(
            console,
            [(SymbolKind::State, SymbolNameRef::Borrowed("write_line"))],
        );

        builder.finish()
    }

    fn main_running_inventory_symbols() -> SymbolTable {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Borrowed("main")),
                (SymbolKind::Data, SymbolNameRef::Borrowed("Inventory")),
            ],
        );
        let mut children = SymbolTableBuilder::child_handles(children);
        let main = children.next().expect("main should be present");
        builder.insert_children(
            main,
            [
                (SymbolKind::State, SymbolNameRef::Borrowed("entry")),
                (SymbolKind::State, SymbolNameRef::Borrowed("running")),
            ],
        );

        builder.finish()
    }
}
