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
pub use table::{
    SourceScopedTopLevelBinding, SymbolNameStorageCounts, SymbolTable, SymbolTableAppender,
    SymbolTableBuilder, SymbolTableExtension,
};

#[cfg(test)]
mod tests {
    mod filtered_lookup;

    use std::path::PathBuf;
    use std::sync::Arc;

    use semantic_vocabulary::PackageKeyIdentity;
    use source::{SourceMap, SourceOrigin, SourceResolutionStratum, SourceSpan, Span};

    use super::{
        SourceScopedTopLevelBinding, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable,
        SymbolTableBuilder,
    };

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
    fn source_scoped_top_level_binding_selects_exact_declaration_source() {
        let mut sources = SourceMap::default();
        let main_source = sources
            .add(PathBuf::from("main.omg"), String::from("Build"))
            .source_id;
        let build_source = sources
            .add(PathBuf::from("build.omg"), String::from("Build"))
            .source_id;
        let prelude_source = sources
            .add(PathBuf::from("<build-prelude>"), String::from("Build"))
            .source_id;
        let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
            Some(Arc::new(sources)),
            vec![SourceScopedTopLevelBinding::new(
                build_source,
                prelude_source,
                "Build",
            )],
        );
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [
                (
                    SymbolKind::Data,
                    SymbolNameRef::Source(SourceSpan::new(main_source, Span::new(0, 5))),
                ),
                (
                    SymbolKind::Data,
                    SymbolNameRef::Source(SourceSpan::new(prelude_source, Span::new(0, 5))),
                ),
            ],
        ))
        .collect::<Vec<_>>();
        let symbols = builder.finish();
        let resolve = |source_id| {
            symbols
                .find_top_level_by_name_and_kinds_from_source(
                    "Build",
                    &[SymbolKind::Data],
                    SourceSpan::new(source_id, Span::new(0, 5)),
                )
                .expect("Build declaration")
        };

        assert_eq!(resolve(main_source), declarations[0]);
        assert_eq!(resolve(build_source), declarations[1]);
        assert_eq!(resolve(prelude_source), declarations[1]);
        assert_eq!(
            symbols.find_top_level_by_name_and_kinds_from_source(
                "Build",
                &[SymbolKind::Data],
                SourceSpan::default(),
            ),
            Some(declarations[0]),
            "source-free generated names must not inherit an authored source binding",
        );
        assert!(symbols.source_scopes_separate(declarations[0], declarations[1]));
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
        assert!(
            symbols.source_reference_can_see_symbol(reference(second_extension), declarations[0],)
        );
        assert!(
            symbols.source_reference_can_see_symbol(reference(second_extension), declarations[1],)
        );
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
    fn source_scoped_top_level_binding_fails_closed_without_its_target() {
        let mut sources = SourceMap::default();
        let main_source = sources
            .add(PathBuf::from("main.omg"), String::from("Build"))
            .source_id;
        let build_source = sources
            .add(PathBuf::from("build.omg"), String::from("Build"))
            .source_id;
        let absent_prelude_source = sources
            .add(PathBuf::from("<build-prelude>"), String::from("Other"))
            .source_id;
        let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
            Some(Arc::new(sources)),
            vec![SourceScopedTopLevelBinding::new(
                build_source,
                absent_prelude_source,
                "Build",
            )],
        );
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        builder.insert_children(
            root,
            [(
                SymbolKind::Data,
                SymbolNameRef::Source(SourceSpan::new(main_source, Span::new(0, 5))),
            )],
        );
        let symbols = builder.finish();

        assert_eq!(
            symbols.find_top_level_by_name_and_kinds_from_source(
                "Build",
                &[SymbolKind::Data],
                SourceSpan::new(build_source, Span::new(0, 5)),
            ),
            None,
        );
    }

    #[test]
    fn source_scoped_top_level_binding_fails_closed_with_ambiguous_targets() {
        let mut sources = SourceMap::default();
        let build_source = sources
            .add(PathBuf::from("build.omg"), String::from("Build"))
            .source_id;
        let prelude_source = sources
            .add(
                PathBuf::from("<build-prelude>"),
                String::from("Build Build"),
            )
            .source_id;
        let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
            Some(Arc::new(sources)),
            vec![SourceScopedTopLevelBinding::new(
                build_source,
                prelude_source,
                "Build",
            )],
        );
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        builder.insert_children(
            root,
            [
                (
                    SymbolKind::Data,
                    SymbolNameRef::Source(SourceSpan::new(prelude_source, Span::new(0, 5))),
                ),
                (
                    SymbolKind::Data,
                    SymbolNameRef::Source(SourceSpan::new(prelude_source, Span::new(6, 11))),
                ),
            ],
        );
        let symbols = builder.finish();

        assert_eq!(
            symbols.find_top_level_by_name_and_kinds_from_source(
                "Build",
                &[SymbolKind::Data],
                SourceSpan::new(build_source, Span::new(0, 5)),
            ),
            None,
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
