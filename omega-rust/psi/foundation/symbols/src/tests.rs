/*
    Fixtures shared by the symbol-table tests: entry, console and running
    inventory symbol tables built from sourced declarations.
*/

mod generated_extension {
    use super::sourced_symbol_table;
    use crate::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use source::SourceOrigin;

    #[test]
    fn generated_extension_retains_exact_rows_and_transitive_origins() {
        let (base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
        let mut candidate = base.clone();
        assert!(candidate.is_generated_extension_of(&base));
        let instance =
            candidate.insert_generated_root_from(authored[0], SymbolKind::Data, "Instance");
        let fields = candidate.insert_generated_children(instance, [(SymbolKind::Field, "value")]);
        let field = SymbolTableBuilder::child_handles(fields).next().unwrap();
        candidate.insert_generated_root_from(field, SymbolKind::Data, "Nested");
        assert!(candidate.is_generated_extension_of(&base));
        let retained = candidate.clone();
        candidate.insert_generated_root_from(instance, SymbolKind::Data, "Later");
        assert!(candidate.is_generated_extension_of(&retained));
        assert!(candidate.is_generated_extension_of(&base));
    }

    #[test]
    fn generated_extension_rejects_changed_retained_spelling() {
        let (base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
        let mut candidate = base.clone();
        candidate.rename(authored[0], "substituted");
        assert!(!candidate.is_generated_extension_of(&base));
    }

    #[test]
    fn generated_extension_rejects_appended_authored_children() {
        let (base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
        let mut candidate = base.clone();
        candidate.append_children(
            authored[0],
            [(SymbolKind::Local, SymbolNameRef::Borrowed("new_local"))],
        );
        assert!(!candidate.is_generated_extension_of(&base));
    }

    #[test]
    fn generated_extension_rejects_changed_existing_child_range() {
        let (mut base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
        let instance = base.insert_generated_root_from(authored[0], SymbolKind::Data, "Instance");
        let mut candidate = base.clone();
        candidate.insert_generated_children(instance, [(SymbolKind::Field, "late")]);
        assert!(!candidate.is_generated_extension_of(&base));
    }

    #[test]
    fn generated_extension_rejects_unknown_generated_declaration() {
        let (base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
        let mut candidate = base.clone();
        candidate.insert_generated_root_from(authored[0], SymbolKind::Unknown, "unknown");
        assert!(!candidate.is_generated_extension_of(&base));
    }

    #[test]
    fn generated_extension_rejects_rebound_source_vocabulary() {
        let base = super::main_entry_symbols();
        let candidate = base
            .clone()
            .begin_extension(
                None,
                vec![crate::SourceScopedTopLevelBinding::new(
                    source::SourceId(0),
                    source::SourceId(1),
                    "main",
                )],
            )
            .finish();
        assert!(!candidate.is_generated_extension_of(&base));
    }

    #[test]
    fn generated_extension_appends_locals_without_rebinding_lexical_children() {
        let (mut base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
        let earlier = base.append_children(
            authored[0],
            [(SymbolKind::Local, SymbolNameRef::Borrowed("earlier"))],
        );
        let earlier = SymbolTableBuilder::child_handles(earlier).next().unwrap();
        let instance =
            base.insert_generated_root_from(authored[0], SymbolKind::Machine, "instance");
        base.insert_generated_children(instance, [(SymbolKind::Local, "existing")]);
        let mut candidate = base.clone();
        for parent in [authored[0], instance] {
            let old_children = candidate.child_handles(parent).unwrap().collect::<Vec<_>>();
            let additions = candidate
                .append_generated_children_from(parent, [(earlier, SymbolKind::Local, "captured")]);
            let captured = SymbolTableBuilder::child_handles(additions).next().unwrap();
            assert_eq!(candidate.get(parent), base.get(parent));
            assert_eq!(candidate.get(captured).parent, parent);
            assert_eq!(candidate.get(captured).generated_from, earlier);
            assert_eq!(candidate.authored_origin(captured), earlier);
            assert_eq!(
                candidate.child_handles(parent).unwrap().collect::<Vec<_>>(),
                old_children
                    .into_iter()
                    .chain([captured])
                    .collect::<Vec<_>>()
            );
            assert!(candidate.is_generated_extension_of(&base));
            let retained = candidate.clone();
            candidate.append_generated_children_from(
                parent,
                [(captured, SymbolKind::Local, "later_capture")],
            );
            assert!(candidate.is_generated_extension_of(&retained));
            assert!(candidate.is_generated_extension_of(&base));
        }
    }
}
mod hierarchy {
    use crate::{SourceScopedTopLevelBinding, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{SourceMap, SourceOrigin, SourceResolutionStratum, SourceSpan, Span};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn appended_lexical_children_preserve_declarations_and_iteration_order() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let machine = builder
            .insert_children(root, [(SymbolKind::Machine, SymbolNameRef::Static("main"))])
            .start();
        let states = builder.insert_children(
            machine,
            [
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::State, SymbolNameRef::Static("later")),
            ],
        );
        let mut states = SymbolTableBuilder::child_handles(states);
        let entry = states.next().unwrap();
        let later = states.next().unwrap();
        let parameters = builder.insert_children(
            entry,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("first")),
                (SymbolKind::Parameter, SymbolNameRef::Static("second")),
            ],
        );
        let mut symbols = builder.finish();
        let original = symbols.clone();
        let first_locals = symbols.append_children(
            entry,
            [
                (SymbolKind::Local, SymbolNameRef::Static("alpha")),
                (SymbolKind::Local, SymbolNameRef::Static("beta")),
            ],
        );
        let sibling_local = symbols
            .append_children(
                later,
                [(SymbolKind::Local, SymbolNameRef::Static("sibling"))],
            )
            .start();
        let last_local = symbols
            .append_children(entry, [(SymbolKind::Local, SymbolNameRef::Static("gamma"))])
            .start();
        let continuation = symbols
            .append_children(
                machine,
                [(SymbolKind::State, SymbolNameRef::Static("continuation"))],
            )
            .start();
        let expected = SymbolTableBuilder::child_handles(parameters)
            .chain(SymbolTableBuilder::child_handles(first_locals))
            .chain([last_local])
            .collect::<Vec<_>>();
        assert_eq!(
            symbols.child_handles(entry).unwrap().collect::<Vec<_>>(),
            expected
        );
        for (ordinal, child) in expected.iter().copied().enumerate() {
            assert_eq!(
                symbols.child_handles(entry).unwrap().nth(ordinal),
                Some(child)
            );
            assert_eq!(symbols.get(child).parent, entry);
        }
        let mut exhausted = symbols.child_handles(entry).unwrap();
        assert_eq!(exhausted.nth(usize::MAX), None);
        assert_eq!(exhausted.next(), None);
        let mut mixed = symbols.child_handles(entry).unwrap();
        assert_eq!(mixed.next(), Some(expected[0]));
        assert_eq!(mixed.nth(2), Some(expected[3]));
        assert_eq!(mixed.next(), Some(expected[4]));
        assert_eq!(mixed.next(), None);
        assert_eq!(symbols.find_child_by_name(entry, "gamma"), Some(last_local));
        assert_eq!(symbols.find_child_by_name(entry, "sibling"), None);
        assert_eq!(
            symbols.find_child_by_name(later, "sibling"),
            Some(sibling_local)
        );
        assert_eq!(
            symbols.find_child_by_name(machine, "continuation"),
            Some(continuation)
        );
        for (handle, _) in original.names().iter() {
            assert_eq!(symbols.names().get(handle), original.names().get(handle));
        }
        assert_eq!(
            symbols
                .symbols()
                .nodes()
                .iter()
                .take(original.symbols().nodes().len())
                .collect::<Vec<_>>(),
            original.symbols().nodes().iter().collect::<Vec<_>>()
        );
        assert_ne!(symbols, original);
        let before_empty = symbols.clone();
        assert!(symbols.append_children(entry, []).is_empty());
        assert_eq!(symbols, before_empty);
    }

    #[test]
    fn supplemental_children_join_generated_extensions_and_invalidate_root_lookup() {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let machine = builder
            .insert_children(root, [(SymbolKind::Machine, SymbolNameRef::Static("main"))])
            .start();
        let mut symbols = builder.finish();
        assert_eq!(symbols.root_children_named("generated").count(), 0);
        let local = symbols
            .append_children(
                machine,
                [(SymbolKind::Local, SymbolNameRef::Static("local"))],
            )
            .start();
        let before = symbols.clone();
        let mut extension = symbols.begin_extension(None, Vec::new());
        let generated = extension
            .insert_top_level([(SymbolKind::Machine, SymbolNameRef::Static("generated"))])[0];
        let synthetic = extension
            .append_children(
                machine,
                [(SymbolKind::State, SymbolNameRef::Static("synthetic"))],
            )
            .start();
        let symbols = extension.finish();
        assert_eq!(
            symbols.root_children_named("generated").collect::<Vec<_>>(),
            [generated]
        );
        assert_eq!(
            symbols.child_handles(machine).unwrap().collect::<Vec<_>>(),
            [local, synthetic]
        );
        assert_eq!(symbols.get(machine), before.get(machine));
        assert_eq!(symbols.get(local), before.get(local));
        assert_eq!(symbols.child_handles(root).unwrap().nth(1), Some(generated));
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
        let mut symbols = builder.finish();
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
        symbols
            .register_source_module(base_source, [("base", reference(base_source))])
            .expect("one base namespace");
        for (name, expected) in [
            ("Value", declarations[1]),
            ("ExtensionOnly", declarations[2]),
        ] {
            assert_eq!(
                symbols.find_top_level_by_name_and_kinds_from_source(
                    name,
                    &[SymbolKind::Data],
                    reference(second_extension),
                ),
                Some(expected),
            );
        }
    }
    #[test]
    fn declaration_ownership_preserves_physical_names_and_fork_isolation() {
        use crate::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let children = builder.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("member")),
                (SymbolKind::Conformance, SymbolNameRef::Static("first")),
                (SymbolKind::Conformance, SymbolNameRef::Static("second")),
            ],
        );
        let mut children = SymbolTableBuilder::child_handles(children);
        let member = children.next().unwrap();
        let first = children.next().unwrap();
        let second = children.next().unwrap();
        let mut symbols = builder.finish();
        symbols.record_declaration_owner(member, first).unwrap();
        symbols.record_declaration_owner(member, first).unwrap();
        assert!(symbols.record_declaration_owner(member, second).is_err());
        assert_eq!(symbols.get(member).parent, root);
        assert_eq!(symbols.display_path(member, "::"), "member");
        let retained = symbols.clone();
        symbols.clear_declaration_owner(member);
        assert!(!symbols.get(member).declaration_owner.is_valid());
        assert_eq!(retained.get(member).declaration_owner, first);
        assert_eq!(retained.get(member).parent, root);
    }
}
mod module_namespaces {
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
        let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
            Some(sources.clone()),
            bindings,
        );
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
    fn logical_root_import_qualifies_siblings_without_bare_or_transitive_exposure() {
        let (symbols, declarations, _) = namespace_table(
            [1, 2, 1],
            vec![SourceScopedTopLevelBinding::logical_module_import(
                SourceId(0),
                SourceId(1),
                "dependency",
                1,
            )],
        );
        assert!(
            symbols
                .validate_source_module_import(SourceId(0), "dependency")
                .is_ok()
        );
        assert_eq!(
            symbols.source_module_import_target(SourceId(0), "dependency"),
            None
        );
        assert!(symbols.is_source_package_qualifier(reference(0), "dependency"));
        assert_eq!(
            data_reference(&symbols, 0, "dependency::Item"),
            Some(declarations[1])
        );
        assert_eq!(data_reference(&symbols, 0, "Item"), Some(declarations[2]));
        assert_eq!(data_reference(&symbols, 2, "dependency::Item"), None);
    }

    #[test]
    fn logical_leaf_import_selects_the_declaring_sibling_not_the_anchor_file() {
        let (mut symbols, declarations, _) = namespace_table(
            [1, 2, 2],
            vec![SourceScopedTopLevelBinding::logical_module_import(
                SourceId(0),
                SourceId(2),
                "dependency::Item",
                1,
            )],
        );
        register_module(&mut symbols, 2, &["child"]);
        assert!(
            symbols
                .validate_source_module_import(SourceId(0), "dependency::Item")
                .is_ok()
        );
        assert_eq!(
            symbols.source_module_import_target(SourceId(0), "dependency::Item"),
            Some(declarations[1])
        );
        assert_eq!(data_reference(&symbols, 0, "Item"), Some(declarations[1]));
        assert!(symbols.source_reference_may_select_symbol(reference(0), declarations[1]));
        assert!(!symbols.source_reference_may_select_symbol(reference(0), declarations[2]));
    }

    #[test]
    fn logical_leaf_import_refuses_missing_and_ambiguous_root_declarations() {
        for path in ["dependency::Missing", "dependency::Item"] {
            let (symbols, _, _) = namespace_table(
                [1, 2, 2],
                vec![SourceScopedTopLevelBinding::logical_module_import(
                    SourceId(0),
                    SourceId(1),
                    path,
                    1,
                )],
            );
            assert!(
                symbols
                    .validate_source_module_import(SourceId(0), path)
                    .is_err()
            );
            assert_eq!(symbols.source_module_import_target(SourceId(0), path), None);
        }
    }

    #[test]
    fn logical_import_does_not_borrow_another_checked_scope_or_inactive_source() {
        use source::DependencyScope::{Build, Product};
        for inactive in [false, true] {
            let (_, _, original) = namespace_table([1, 2, 2], Vec::new());
            let mut files = original.files().cloned().collect::<Vec<_>>();
            files[0].dependency_scope = Build;
            files[1].dependency_scope = Build;
            files[2].dependency_scope = Product;
            let mut sources = SourceMap::from_files(files);
            if inactive {
                sources.retain_selected_sources(|file| file.source_id != SourceId(1));
            }
            let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
                Some(Arc::new(sources)),
                vec![SourceScopedTopLevelBinding::logical_module_import(
                    SourceId(0),
                    SourceId(1),
                    "dependency::Item",
                    1,
                )],
            );
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let item = SymbolTableBuilder::child_handles(builder.insert_children(
                root,
                [(
                    SymbolKind::Data,
                    SymbolNameRef::OwnedSource {
                        value: "Item",
                        source_span: reference(2),
                    },
                )],
            ))
            .next()
            .unwrap();
            let symbols = builder.finish();
            assert!(
                symbols
                    .validate_source_module_import(SourceId(0), "dependency::Item")
                    .is_err()
            );
            assert_eq!(data_reference(&symbols, 0, "dependency::Item"), None);
            assert!(!symbols.source_reference_may_select_symbol(reference(0), item));
        }
    }

    #[test]
    fn logical_root_domain_exposure_is_direct_narrow_and_file_local() {
        for path in ["dependency", "dependency::u64::Near", "dependency::Item"] {
            let (_, _, sources) = namespace_table([1, 2, 2], Vec::new());
            let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
                Some(sources),
                vec![SourceScopedTopLevelBinding::logical_module_import(
                    SourceId(0),
                    SourceId(1),
                    path,
                    1,
                )],
            );
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
                root,
                [
                    (SymbolKind::BuiltinType, SymbolNameRef::Static("u64")),
                    (
                        SymbolKind::Domain,
                        SymbolNameRef::OwnedSource {
                            value: "u64::Near",
                            source_span: reference(1),
                        },
                    ),
                    (
                        SymbolKind::Domain,
                        SymbolNameRef::OwnedSource {
                            value: "u64::Far",
                            source_span: reference(1),
                        },
                    ),
                    (
                        SymbolKind::Domain,
                        SymbolNameRef::OwnedSource {
                            value: "u64::Child",
                            source_span: reference(2),
                        },
                    ),
                    (
                        SymbolKind::Data,
                        SymbolNameRef::OwnedSource {
                            value: "Item",
                            source_span: reference(1),
                        },
                    ),
                ],
            ))
            .collect::<Vec<_>>();
            let mut symbols = builder.finish();
            register_module(&mut symbols, 2, &["nested"]);
            assert!(
                symbols
                    .validate_source_module_import(SourceId(0), path)
                    .is_ok()
            );
            assert_eq!(
                symbols.source_domain_is_exposed(reference(0), declarations[1]),
                path != "dependency::Item"
            );
            assert_eq!(
                symbols.source_domain_is_exposed(reference(0), declarations[2]),
                path == "dependency"
            );
            assert!(!symbols.source_domain_is_exposed(reference(0), declarations[3]));
            assert_eq!(
                domain_reference(&symbols, 0, "u64::Near"),
                (path != "dependency::Item").then_some(declarations[1])
            );
            assert_eq!(
                domain_reference(&symbols, 0, "u64::Far"),
                (path == "dependency").then_some(declarations[2])
            );
        }
        let (_, _, sources) = namespace_table([1, 2, 1], Vec::new());
        let mut builder =
            SymbolTableBuilder::with_sources_and_top_level_bindings(Some(sources), Vec::new());
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let domain = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [(
                SymbolKind::Domain,
                SymbolNameRef::OwnedSource {
                    value: "Distance",
                    source_span: reference(1),
                },
            )],
        ))
        .next()
        .unwrap();
        let symbols = builder.finish();
        assert!(!symbols.source_domain_is_exposed(reference(0), domain));
        assert!(!symbols.domain_name_reaches(domain, "Distance", reference(0)));
    }

    #[test]
    fn syntax_only_module_import_stays_in_its_checked_package() {
        let (mut symbols, declarations, _) = namespace_table([1, 1, 2], Vec::new());
        register_module(&mut symbols, 0, &["generated"]);
        register_module(&mut symbols, 1, &["main"]);
        register_module(&mut symbols, 2, &["main"]);
        symbols.register_source_import(SourceId(0), "main");
        assert_eq!(
            data_reference(&symbols, 0, "main::Item"),
            Some(declarations[1]),
            "loaded foreign namespaces cannot compete with a package-local import",
        );

        let (mut symbols, declarations, _) = namespace_table(
            [1, 1, 2],
            vec![SourceScopedTopLevelBinding::module_import(
                SourceId(0),
                SourceId(2),
                "dependency::main",
                1,
            )],
        );
        register_module(&mut symbols, 0, &["generated"]);
        register_module(&mut symbols, 1, &["main"]);
        register_module(&mut symbols, 2, &["main"]);
        assert_eq!(
            data_reference(&symbols, 0, "dependency::main::Item"),
            Some(declarations[2]),
            "loader-certified imports retain their exact foreign package",
        );
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
                    file.dependency_scope =
                        if source_ordinal == 0 || source_ordinal == helper_source {
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

    /*
        An exact-source import of a package's root file exposes the units that
        package's build handed off, in the importer's checked instance only, while
        the package's own authored file still cannot select them.
    */
    #[test]
    fn unmoduled_exact_import_exposes_the_dependency_generated_stratum() {
        use source::DependencyScope::{Build, Product};
        use source::SourceResolutionStratum::{Base, DependencyActivationExtension};
        let consumer = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let kit = PackageKeyIdentity::from_digest([2; 32]).unwrap();
        let mut sources = SourceMap::default();
        let rows = [
            ("consumer", "main.omg", consumer, Product, Base),
            ("kit", "main.omg", kit, Product, Base),
            (
                "kit",
                ".omega/generated/generated.omg",
                kit,
                Product,
                DependencyActivationExtension,
            ),
            (
                "kit",
                ".omega/generated/generated.omg",
                kit,
                Build,
                DependencyActivationExtension,
            ),
        ];
        for (root, file, identity, scope, stratum) in rows {
            let root = PathBuf::from(root);
            sources.add_checked_instance(
                root.join(file),
                String::from("Item"),
                root,
                Some(identity),
                SourceOrigin::User,
                stratum,
                scope,
            );
        }
        let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
            Some(Arc::new(sources)),
            vec![SourceScopedTopLevelBinding::module_import(
                SourceId(0),
                SourceId(1),
                "kit::main",
                1,
            )],
        );
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [2, 3].into_iter().map(|source_ordinal| {
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
        assert_eq!(data_reference(&symbols, 0, "Item"), Some(declarations[0]));
        assert_eq!(
            symbols.lookup_top_level_by_name_and_kinds_from_source_matching(
                "Item",
                &[SymbolKind::Data],
                reference(0),
                |_| true,
            ),
            crate::SymbolLookup::Unique(declarations[0])
        );
        assert_eq!(data_reference(&symbols, 1, "Item"), None);
    }

    #[test]
    fn unmoduled_local_owner_precedes_same_package_imported_spelling() {
        let bindings = [0, 1]
            .into_iter()
            .map(|source_ordinal| {
                SourceScopedTopLevelBinding::module_import(
                    SourceId(source_ordinal),
                    SourceId(2),
                    "sibling",
                    0,
                )
            })
            .collect();
        let (symbols, declarations, _) = namespace_table([1, 1, 1], bindings);
        assert_eq!(data_reference(&symbols, 1, "Item"), Some(declarations[1]));
        assert_eq!(data_reference(&symbols, 0, "Item"), Some(declarations[2]));
        assert_eq!(
            symbols.lookup_signature_free_top_level_from_source_matching(
                "Item",
                &[SymbolKind::Data],
                reference(1),
                |_| true,
            ),
            crate::SymbolLookup::Unique(declarations[1])
        );
    }

    #[test]
    fn unmoduled_package_discovery_keeps_checked_occurrences_separate() {
        let (ambiguous, _, _) = namespace_table(
            [1, 1, 1],
            vec![SourceScopedTopLevelBinding::module_import(
                SourceId(0),
                SourceId(0),
                "other",
                0,
            )],
        );
        assert!(matches!(
            ambiguous.lookup_top_level_by_name_and_kinds_from_source_matching(
                "Item",
                &[SymbolKind::Data],
                reference(0),
                |_| true,
            ),
            crate::SymbolLookup::Ambiguous { .. }
        ));
        let (_, _, sources) = namespace_table([1, 1, 1], Vec::new());
        let mut files = sources.files().cloned().collect::<Vec<_>>();
        for (ordinal, file) in files.iter_mut().enumerate() {
            file.origin = SourceOrigin::Toolchain;
            file.dependency_scope = if ordinal == 2 {
                source::DependencyScope::Product
            } else {
                source::DependencyScope::Build
            };
        }
        let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
            Some(Arc::new(SourceMap::from_files(files))),
            vec![SourceScopedTopLevelBinding::module_import(
                SourceId(0),
                SourceId(0),
                "other",
                0,
            )],
        );
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [1, 2].into_iter().map(|ordinal| {
                (
                    SymbolKind::Data,
                    SymbolNameRef::OwnedSource {
                        value: "Item",
                        source_span: reference(ordinal),
                    },
                )
            }),
        ))
        .collect::<Vec<_>>();
        let symbols = builder.finish();
        assert_eq!(data_reference(&symbols, 0, "Item"), Some(declarations[0]));
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

    #[test]
    fn declared_carrier_does_not_reach_another_sources_generic_family() {
        let mut sources = SourceMap::default();
        sources.add(PathBuf::from("program.omg"), "Lease".to_owned());
        sources.add(PathBuf::from("run.omg"), "Run".to_owned());
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let mut declarations = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [
                (
                    SymbolKind::Data,
                    SymbolNameRef::OwnedSource {
                        value: "Lease",
                        source_span: reference(0),
                    },
                ),
                (
                    SymbolKind::Data,
                    SymbolNameRef::OwnedSource {
                        value: "Run",
                        source_span: reference(1),
                    },
                ),
                (
                    SymbolKind::Domain,
                    SymbolNameRef::OwnedSource {
                        value: "Live",
                        source_span: reference(1),
                    },
                ),
            ],
        ));
        declarations.next().expect("lease data");
        declarations.next().expect("run data");
        let live = declarations.next().expect("run live family");
        let symbols = builder.finish();
        assert!(
            !symbols.domain_name_reaches(live, "Lease::Live", reference(0)),
            "a sum case must not compete with another source's generic family"
        );
        assert!(
            symbols.domain_name_reaches(live, "Run::Live", reference(0)),
            "the carrier declared beside the family still abbreviates it"
        );
    }

    #[test]
    fn warmed_lookup_environment_tracks_module_registration_and_rename() {
        let (mut symbols, declarations, _) = namespace_table(
            [1, 2, 2],
            vec![SourceScopedTopLevelBinding::module_import(
                SourceId(0),
                SourceId(1),
                "dependency::api",
                1,
            )],
        );
        assert_eq!(data_reference(&symbols, 0, "Item"), Some(declarations[1]));
        register_module(&mut symbols, 1, &["api"]);
        assert_eq!(data_reference(&symbols, 0, "Item"), None);
        assert_eq!(
            data_reference(&symbols, 0, "dependency::api::Item"),
            Some(declarations[1])
        );

        let module = symbols.source_module(SourceId(1));
        let original = symbols.clone();
        symbols.rename(module, "renamed");
        assert_eq!(data_reference(&symbols, 0, "dependency::api::Item"), None);
        assert_eq!(
            data_reference(&symbols, 0, "dependency::renamed::Item"),
            Some(declarations[1])
        );
        assert_eq!(
            data_reference(&original, 0, "dependency::api::Item"),
            Some(declarations[1])
        );
        assert_eq!(
            data_reference(&original, 0, "dependency::renamed::Item"),
            None
        );
    }

    #[test]
    fn callable_roster_retains_all_visible_module_declarations() {
        let (_, _, sources) = namespace_table([1, 1, 2], Vec::new());
        let mut builder =
            SymbolTableBuilder::with_sources_and_top_level_bindings(Some(sources), Vec::new());
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            (0..3).map(|_| {
                (
                    SymbolKind::Machine,
                    SymbolNameRef::OwnedSource {
                        value: "Item",
                        source_span: reference(0),
                    },
                )
            }),
        ))
        .collect::<Vec<_>>();
        let mut symbols = builder.finish();
        register_module(&mut symbols, 0, &["math"]);
        for name in ["Item", "math::Item"] {
            assert_eq!(
                symbols.top_level_candidates_from_source(
                    name,
                    &[SymbolKind::Machine],
                    reference(0)
                ),
                declarations
            );
            assert!(matches!(
                symbols.lookup_signature_free_top_level_from_source_matching(
                    name,
                    &[SymbolKind::Machine],
                    reference(0),
                    |_| true
                ),
                crate::SymbolLookup::Ambiguous { .. }
            ));
        }
        assert!(
            symbols
                .top_level_candidates_from_source(
                    "math::Item",
                    &[SymbolKind::Machine],
                    reference(2)
                )
                .is_empty()
        );
    }
}
mod package_scope_separation {
    /*
        Cross-package duplicate law: declarations in distinct packages share a
        resolution scope only where an authored import edge exposes the name.
        A toolchain or dependency closure therefore coexists with a package's
        own same-named declarations, while an authored `use` keeps the rule that
        imports do not shadow authored declarations.
    */

    use std::path::PathBuf;
    use std::sync::Arc;

    use semantic_vocabulary::PackageKeyIdentity;
    use source::{SourceId, SourceMap, SourceOrigin, SourceSpan, Span};

    use crate::{
        SourceScopedTopLevelBinding, SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable,
        SymbolTableBuilder,
    };

    fn package_identity(byte: u8) -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([byte; 32]).expect("nonzero package identity")
    }

    /*
        One package file declaring `ExtentRootProvider` beside one dependency or
        toolchain file declaring the same leaf name.
    */
    fn collision_table(
        dependency_identity: Option<PackageKeyIdentity>,
        dependency_origin: SourceOrigin,
        bindings: Vec<SourceScopedTopLevelBinding>,
    ) -> (SymbolTable, [SymbolHandle; 2]) {
        let mut sources = SourceMap::default();
        let fixture_root = PathBuf::from("package-fixture");
        sources.add_with_metadata(
            fixture_root.join("main.omg"),
            String::from("ExtentRootProvider"),
            fixture_root,
            Some(package_identity(41)),
            SourceOrigin::User,
        );
        let dependency_root = PathBuf::from("dependency-core");
        sources.add_with_metadata(
            dependency_root.join("extent.omg"),
            String::from("ExtentRootProvider"),
            dependency_root,
            dependency_identity,
            dependency_origin,
        );
        let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
            Some(Arc::new(sources)),
            bindings,
        );
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [SourceId(0), SourceId(1)].map(|source| {
                (
                    SymbolKind::Trait,
                    SymbolNameRef::Source(SourceSpan::new(source, Span::new(0, 18))),
                )
            }),
        ))
        .collect::<Vec<_>>();
        (
            builder.finish(),
            declarations.try_into().expect("two declarations"),
        )
    }

    #[test]
    fn broad_module_import_does_not_expose_unqualified_leaves() {
        let (mut symbols, [fixture, dependency]) = collision_table(
            Some(package_identity(77)),
            SourceOrigin::User,
            vec![SourceScopedTopLevelBinding::module_import(
                SourceId(0),
                SourceId(1),
                "dep::extent",
                1,
            )],
        );
        symbols
            .register_source_module(
                SourceId(1),
                [("extent", SourceSpan::new(SourceId(1), Span::new(0, 6)))],
            )
            .expect("module registration");
        assert!(symbols.source_scopes_separate(fixture, dependency));
    }
}
mod paths {
    use super::{main_console_symbols, main_entry_symbols};
    use crate::SymbolHandle;

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
}
mod provenance {
    use super::sourced_symbol_table;
    use crate::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use semantic_vocabulary::PackageKeyIdentity;
    use source::{SourceMap, SourceOrigin, SourceSpan, Span};
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn generated_children_retain_individual_origins_and_contiguous_parentage() {
        let package = PackageKeyIdentity::from_digest([8; 32]).unwrap();
        let (mut symbols, authored) = sourced_symbol_table([
            (SourceOrigin::User, Some(package)),
            (SourceOrigin::Toolchain, None),
        ]);
        let parent = symbols.insert_generated_root_from(authored[0], SymbolKind::Machine, "clone");
        let children = symbols.insert_generated_children_from(
            parent,
            [
                (authored[0], SymbolKind::Parameter, "first"),
                (authored[1], SymbolKind::Local, "second"),
            ],
        );
        let handles = SymbolTableBuilder::child_handles(children).collect::<Vec<_>>();
        assert_eq!(symbols.get(parent).children, children);
        assert_eq!(handles[1].arena_index(), handles[0].arena_index() + 1);
        for (child, origin) in handles.iter().copied().zip(authored.iter().copied()) {
            assert_eq!(symbols.get(child).parent, parent);
            assert_eq!(symbols.get(child).generated_from, origin);
            assert_eq!(symbols.authored_origin(child), origin);
            assert_eq!(
                symbols.symbol_package_identity(child),
                symbols.symbol_package_identity(origin)
            );
            assert_eq!(
                symbols.symbol_source_origin(child),
                symbols.symbol_source_origin(origin)
            );
        }
        let next = symbols.insert_generated_root_from(parent, SymbolKind::Machine, "next");
        let descendant = SymbolTableBuilder::child_handles(
            symbols
                .insert_generated_children_from(next, [(handles[1], SymbolKind::Local, "second")]),
        )
        .next()
        .unwrap();
        assert_eq!(symbols.get(descendant).generated_from, handles[1]);
        assert_eq!(symbols.authored_origin(descendant), authored[1]);
    }

    #[test]
    #[should_panic(expected = "existing derivation origins")]
    fn generated_children_reject_absent_derivation_origin() {
        let (mut symbols, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
        let parent = symbols.insert_generated_root_from(authored[0], SymbolKind::Machine, "clone");
        symbols.insert_generated_children_from(
            parent,
            [(
                crate::SymbolHandle::invalid(),
                SymbolKind::Parameter,
                "value",
            )],
        );
    }

    #[test]
    fn product_package_identity_excludes_the_build_checked_instance() {
        let package_identity =
            PackageKeyIdentity::from_digest([6; 32]).expect("nonzero package identity");
        let mut sources = SourceMap::default();
        let spans = [
            source::DependencyScope::Product,
            source::DependencyScope::Build,
        ]
        .map(|scope| {
            let source_id = sources
                .add_checked_instance(
                    PathBuf::from("package/shared.omg"),
                    String::from("launch"),
                    PathBuf::from("package"),
                    Some(package_identity),
                    SourceOrigin::User,
                    source::SourceResolutionStratum::Base,
                    scope,
                )
                .source_id;
            SourceSpan::new(source_id, Span::new(0, 6))
        });
        let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources)));
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let declarations = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            spans.map(|span| (SymbolKind::Machine, SymbolNameRef::Source(span))),
        ))
        .collect::<Vec<_>>();
        let implicit_states = declarations
            .iter()
            .map(|declaration| {
                SymbolTableBuilder::child_handles(builder.insert_children(
                    *declaration,
                    [(SymbolKind::State, SymbolNameRef::Static("entry"))],
                ))
                .next()
                .expect("implicit state")
            })
            .collect::<Vec<_>>();
        let mut symbols = builder.finish();
        for ((declaration, implicit_state), expected) in declarations
            .into_iter()
            .zip(implicit_states)
            .zip([Some(package_identity), None])
        {
            assert_eq!(
                symbols.symbol_package_identity(declaration),
                Some(package_identity)
            );
            assert_eq!(
                symbols.symbol_product_package_identity(declaration),
                expected
            );
            let generated =
                symbols.insert_generated_root_from(declaration, SymbolKind::Machine, "specialized");
            assert_eq!(symbols.symbol_product_package_identity(generated), expected);
            assert_eq!(
                symbols.symbol_product_package_identity(implicit_state),
                expected
            );
            let generated_child = SymbolTableBuilder::child_handles(
                symbols.insert_generated_children(generated, [(SymbolKind::State, "entry")]),
            )
            .next()
            .expect("generated state");
            assert_eq!(
                symbols.symbol_product_package_identity(generated_child),
                expected
            );
        }
        assert_eq!(
            symbols.symbol_product_package_identity(symbols.root()),
            None
        );
        assert_eq!(
            symbols.symbol_product_package_identity(crate::SymbolHandle::invalid()),
            None
        );
    }

    #[test]
    fn product_package_identity_requires_retained_source_metadata() {
        for retained_sources in [None, Some(Arc::new(SourceMap::default()))] {
            let mut builder = SymbolTableBuilder::with_sources(retained_sources);
            let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
            let declaration = SymbolTableBuilder::child_handles(builder.insert_children(
                root,
                [(
                    SymbolKind::Machine,
                    SymbolNameRef::Source(SourceSpan::new(source::SourceId(5), Span::new(0, 6))),
                )],
            ))
            .next()
            .expect("declaration with unavailable source");
            assert_eq!(
                builder
                    .finish()
                    .symbol_product_package_identity(declaration),
                None
            );
        }
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
        assert_eq!(symbols.symbol_product_package_identity(authored[0]), None);
        assert_eq!(symbols.symbol_product_package_identity(authored[1]), None);
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
    fn generated_symbols_keep_module_scope_without_hiding_same_owner_duplicates() {
        let (mut symbols, authored) =
            sourced_symbol_table([(SourceOrigin::User, None), (SourceOrigin::User, None)]);
        for (declaration, module) in authored.iter().zip(["first", "second"]) {
            let span = symbols
                .symbol_source_span(*declaration)
                .expect("authored span");
            symbols
                .register_source_module(span.source_id, [(module, span)])
                .expect("module");
        }
        assert!(symbols.source_scopes_separate(authored[0], authored[1]));
        let generated = authored
            .iter()
            .map(|declaration| {
                symbols.insert_generated_root_from(
                    *declaration,
                    SymbolKind::Machine,
                    "read$specialized",
                )
            })
            .collect::<Vec<_>>();
        assert!(symbols.source_scopes_separate(generated[0], generated[1]));
        assert!(symbols.source_scopes_separate(authored[0], generated[1]));
        let duplicate = symbols.insert_generated_root_from(
            authored[0],
            SymbolKind::Machine,
            "read$specialized",
        );
        assert!(!symbols.source_scopes_separate(generated[0], duplicate));
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

    /*
        A BUILD operand may name a public toolchain declaration; ordinary product
        declaration selection may not.

        The toolchain injects its core into every product with no package identity
        and no manifest, so neither a same-package test nor an alias can ever reach
        it. Widening the ordinary lookup instead would change which declaration an
        ordinary call selects.
    */
    #[test]
    fn only_a_build_subject_reaches_a_public_toolchain_declaration() {
        let (symbols, declarations) = sourced_symbol_table([
            (
                SourceOrigin::User,
                Some(PackageKeyIdentity::from_digest([7; 32]).unwrap()),
            ),
            (SourceOrigin::Toolchain, None),
        ]);
        let occurrence = symbols
            .symbol_source_span(declarations[0])
            .expect("the authored declaration retains its source");
        let toolchain = declarations[1];
        let name = symbols.name(toolchain).to_owned();

        assert_eq!(
            symbols.find_build_subject_declaration_from_source(
                &name,
                occurrence,
                [(toolchain, true)],
                &[]
            ),
            Some(toolchain),
            "a build operand names the injected core declaration it selects a provider for"
        );
        assert_eq!(
            symbols.find_product_declaration_from_source(&name, occurrence, [(toolchain, true)]),
            None,
            "ordinary product selection keeps the same-package rule"
        );
        assert_eq!(
            symbols.find_build_subject_declaration_from_source(
                &name,
                occurrence,
                [(toolchain, false)],
                &[]
            ),
            None,
            "a private toolchain declaration is not a build operand's to name"
        );
    }

    #[test]
    fn build_family_selection_requires_explicit_token_membership() {
        let mut builder = SymbolTableBuilder::default();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let members = SymbolTableBuilder::child_handles(builder.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("Math::add")),
                (SymbolKind::Machine, SymbolNameRef::Static("Math::add")),
            ],
        ))
        .collect::<Vec<_>>();
        let symbols = builder.finish();
        let candidates = [(members[0], false), (members[1], false)];
        let find = |token_bound_machines: &[crate::SymbolHandle]| {
            symbols.find_build_subject_declaration_from_source(
                "Math::add",
                SourceSpan::default(),
                candidates,
                token_bound_machines,
            )
        };
        assert_eq!(
            find(&[]),
            None,
            "ordinary machine overloads are not a family operand"
        );
        assert_eq!(
            find(&members[..1]),
            None,
            "an unrelated same-path candidate remains ambiguous"
        );
        assert_eq!(find(&members), Some(members[0]));
    }
}
mod source_scoped_bindings {
    use crate::{SourceScopedTopLevelBinding, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use source::{SourceMap, SourceSpan, Span};
    use std::path::PathBuf;
    use std::sync::Arc;

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
}

use crate::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTable, SymbolTableBuilder};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;

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
        source_spans.map(|source_span| (SymbolKind::Machine, SymbolNameRef::Source(source_span))),
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
