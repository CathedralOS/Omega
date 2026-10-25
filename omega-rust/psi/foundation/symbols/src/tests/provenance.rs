use super::sourced_symbol_table;
use crate::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use semantic_vocabulary::PackageKeyIdentity;
use source::{SourceMap, SourceOrigin, SourceSpan, Span};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn inactive_backing_is_not_a_lookup_candidate_or_product_operand() {
    let owner = PackageKeyIdentity::from_digest([9; 32]).expect("owner identity");
    let mut sources = SourceMap::default();
    let declarations = ["first", "second"].map(|name| {
        sources
            .add_with_metadata(
                PathBuf::from(format!("kit/{name}.omg")),
                String::from("shared Item"),
                PathBuf::from("kit"),
                Some(owner),
                SourceOrigin::User,
            )
            .source_span(Span::new(7, 11))
    });
    let consumer = PackageKeyIdentity::from_digest([14; 32]).expect("consumer owner");
    let importer = sources
        .add_with_metadata(
            PathBuf::from("consumer/main.omg"),
            String::from("lookup"),
            PathBuf::from("consumer"),
            Some(consumer),
            SourceOrigin::User,
        )
        .source_span(Span::new(0, 6));
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources.clone())));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let handles = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        declarations.map(|span| (SymbolKind::Data, SymbolNameRef::Source(span))),
    ))
    .collect::<Vec<_>>();
    let mut symbols = builder.finish();
    for span in declarations {
        symbols
            .register_source_module(
                span.source_id,
                [("shared", SourceSpan::new(span.source_id, Span::new(0, 6)))],
            )
            .expect("one module per source");
    }
    let module = symbols.source_module(declarations[0].source_id);
    assert_eq!(symbols.source_module(declarations[1].source_id), module);

    let mut current = sources.fork_source_selection().expect("sealed owners");
    current.select_checked_instance(
        PathBuf::from("kit/second.omg"),
        String::from("shared Item"),
        PathBuf::from("kit"),
        Some(owner),
        SourceOrigin::User,
        source::SourceResolutionStratum::Base,
        source::DependencyScope::Product,
    );
    current.select_checked_instance(
        PathBuf::from("consumer/main.omg"),
        String::from("lookup"),
        PathBuf::from("consumer"),
        Some(consumer),
        SourceOrigin::User,
        source::SourceResolutionStratum::Base,
        source::DependencyScope::Product,
    );
    let stale_alias = symbols
        .clone()
        .begin_extension(
            Some(Arc::new(current.clone())),
            vec![crate::SourceScopedTopLevelBinding::module_import(
                importer.source_id,
                declarations[0].source_id,
                "leaf::shared",
                1,
            )],
        )
        .finish();
    assert!(
        stale_alias
            .validate_source_module_import(importer.source_id, "leaf::shared")
            .is_err()
    );
    assert_eq!(
        stale_alias.find_top_level_by_name_and_kinds_from_source(
            "leaf::shared::Item",
            &[SymbolKind::Data],
            importer,
        ),
        None,
        "an inactive imported file cannot lend its alias to an active sibling"
    );
    let selected = symbols
        .begin_source_activation(Arc::new(current), Vec::new())
        .expect("retained source coordinates")
        .finish();
    let occurrence = declarations[1];
    assert_eq!(selected.source_files().count(), 2);
    assert_eq!(
        selected.root_children_named("Item").collect::<Vec<_>>(),
        [handles[1]]
    );
    assert_eq!(selected.source_text(declarations[0]), "Item");
    assert_eq!(selected.source_module(declarations[0].source_id), module);
    assert_eq!(selected.source_module(occurrence.source_id), module);
    assert!(selected.source_reference_can_see_symbol(occurrence, module));
    assert_eq!(
        selected
            .child_handles(module)
            .expect("module children")
            .collect::<Vec<_>>(),
        [handles[1]]
    );
    assert_eq!(
        selected.find_top_level_by_name_and_kinds_from_source(
            "Item",
            &[SymbolKind::Data],
            occurrence,
        ),
        Some(handles[1])
    );
    assert_eq!(
        selected.find_top_level_by_name_and_kinds_from_source(
            "Item",
            &[SymbolKind::Data],
            SourceSpan::default(),
        ),
        Some(handles[1])
    );
    assert_eq!(
        selected.find_top_level_by_name_and_kinds_from_source(
            "Item",
            &[SymbolKind::Data],
            declarations[0],
        ),
        None
    );
    for candidates in [
        vec![(handles[0], true)],
        vec![(handles[0], true), (handles[1], true)],
    ] {
        let expected = (candidates.len() == 2).then_some(handles[1]);
        assert_eq!(
            selected.find_product_declaration_from_source(
                "shared::Item",
                occurrence,
                candidates.clone(),
            ),
            expected
        );
        assert_eq!(
            selected.find_build_subject_declaration_from_source(
                "shared::Item",
                occurrence,
                candidates,
                &[],
            ),
            expected
        );
    }
    assert_eq!(sources.files().count(), 3);
    assert!(sources.source_is_selected(declarations[0].source_id));

    let restored = selected
        .begin_source_activation(Arc::new(sources.clone()), Vec::new())
        .expect("same backing, a later declaration cohort")
        .finish();
    assert!(
        !restored.source_reference_can_see_symbol(declarations[0], handles[0]),
        "reloading bytes cannot resurrect a declaration removed from the predecessor"
    );
    assert_eq!(
        restored.root_children_named("Item").collect::<Vec<_>>(),
        [handles[1]]
    );
    let mut extension = restored.begin_extension(Some(Arc::new(sources.clone())), Vec::new());
    let current = extension
        .append_children(
            root,
            [(SymbolKind::Data, SymbolNameRef::Source(declarations[0]))],
        )
        .handle_at(0);
    let mut restored = extension.finish();
    restored
        .register_source_module(
            declarations[0].source_id,
            [(
                "shared",
                SourceSpan::new(declarations[0].source_id, Span::new(0, 6)),
            )],
        )
        .expect("the new cohort rejoins the logical namespace");
    let restored = restored
        .begin_source_activation(Arc::new(sources), Vec::new())
        .expect("keep both current declarations")
        .finish();
    assert_eq!(
        restored.root_children_named("Item").collect::<Vec<_>>(),
        [handles[1], current]
    );
    assert_eq!(
        restored.find_top_level_by_name_and_kinds_from_source(
            "Item",
            &[SymbolKind::Data],
            declarations[0],
        ),
        None,
        "both current declarations still collide in their shared module"
    );
    assert!(restored.source_reference_can_see_symbol(declarations[0], current));
}

#[test]
fn current_import_roster_replaces_historical_authority_even_with_active_endpoints() {
    let old_package = PackageKeyIdentity::from_digest([10; 32]).expect("old package");
    let new_package = PackageKeyIdentity::from_digest([11; 32]).expect("new package");
    let consumer = PackageKeyIdentity::from_digest([12; 32]).expect("consumer package");
    let mut sources = SourceMap::default();
    let declarations = [
        ("old", old_package),
        ("new", new_package),
        ("consumer", consumer),
    ]
    .map(|(name, owner)| {
        sources
            .add_with_metadata(
                PathBuf::from(format!("{name}/module.omg")),
                String::from("shared Item"),
                PathBuf::from(name),
                Some(owner),
                SourceOrigin::User,
            )
            .source_span(Span::new(7, 11))
    });
    let old_binding = crate::SourceScopedTopLevelBinding::module_import(
        declarations[2].source_id,
        declarations[0].source_id,
        "leaf::shared",
        1,
    );
    let new_binding = crate::SourceScopedTopLevelBinding::module_import(
        declarations[2].source_id,
        declarations[1].source_id,
        "leaf::shared",
        1,
    );
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(Arc::new(sources.clone())),
        vec![old_binding.clone()],
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let handles = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        declarations.map(|span| (SymbolKind::Data, SymbolNameRef::Source(span))),
    ))
    .collect::<Vec<_>>();
    let mut symbols = builder.finish();
    for span in &declarations[..2] {
        symbols
            .register_source_module(
                span.source_id,
                [("shared", SourceSpan::new(span.source_id, Span::new(0, 6)))],
            )
            .expect("dependency module");
    }
    let mut current = sources.fork_source_selection().expect("sealed owners");
    for file in sources.files().skip(1) {
        current.select_checked_instance(
            file.path.clone(),
            file.source.clone(),
            file.package_root.clone(),
            file.package_identity,
            file.origin,
            file.resolution_stratum,
            file.dependency_scope,
        );
    }
    // Even append-style resumption cannot let the inactive old binding reject
    // or shadow an otherwise valid admitted replacement.
    let extended = symbols
        .clone()
        .begin_extension(Some(Arc::new(current.clone())), vec![new_binding.clone()])
        .finish();
    extended
        .validate_source_module_import(declarations[2].source_id, "leaf::shared")
        .expect("current replacement is valid");
    assert_eq!(
        extended.find_top_level_by_name_and_kinds_from_source(
            "leaf::shared::Item",
            &[SymbolKind::Data],
            declarations[2],
        ),
        Some(handles[1])
    );
    // Keeping the historical target active for another reason must not keep
    // its old edge. Only the new activation's complete roster is authoritative.
    let old_file = sources
        .get(declarations[0].source_id)
        .expect("historical source");
    current.select_checked_instance(
        old_file.path.clone(),
        old_file.source.clone(),
        old_file.package_root.clone(),
        old_file.package_identity,
        old_file.origin,
        old_file.resolution_stratum,
        old_file.dependency_scope,
    );
    let selected = symbols
        .clone()
        .begin_source_activation(Arc::new(current.clone()), vec![new_binding])
        .expect("retained source backing")
        .finish();
    assert_eq!(
        selected.find_top_level_by_name_and_kinds_from_source(
            "leaf::shared::Item",
            &[SymbolKind::Data],
            declarations[2],
        ),
        Some(handles[1])
    );
    let without_binding = symbols
        .begin_source_activation(Arc::new(current), Vec::new())
        .expect("retained source backing")
        .finish();
    assert_eq!(
        without_binding.find_top_level_by_name_and_kinds_from_source(
            "leaf::shared::Item",
            &[SymbolKind::Data],
            declarations[2],
        ),
        None
    );
}

#[test]
fn source_activation_retains_authored_imports_without_borrowing_a_foreign_namespace() {
    let owner = PackageKeyIdentity::from_digest([31; 32]).expect("owner");
    let foreign = PackageKeyIdentity::from_digest([32; 32]).expect("foreign owner");
    let mut sources = SourceMap::default();
    let declarations = [
        ("kit/left.omg", "left Helper", "kit", owner, 5, 11),
        ("kit/right.omg", "right Local", "kit", owner, 6, 11),
        ("foreign/left.omg", "left Helper", "foreign", foreign, 5, 11),
    ]
    .map(|(path, text, root, package, start, end)| {
        sources
            .add_with_metadata(
                PathBuf::from(path),
                text.to_owned(),
                PathBuf::from(root),
                Some(package),
                SourceOrigin::User,
            )
            .source_span(Span::new(start, end))
    });
    let mut builder = SymbolTableBuilder::with_sources(Some(Arc::new(sources.clone())));
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let handles = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        declarations.map(|span| (SymbolKind::Data, SymbolNameRef::Source(span))),
    ))
    .collect::<Vec<_>>();
    let mut symbols = builder.finish();
    for (span, module) in declarations.into_iter().zip(["left", "right", "left"]) {
        symbols
            .register_source_module(
                span.source_id,
                [(
                    module,
                    SourceSpan::new(span.source_id, Span::new(0, module.len())),
                )],
            )
            .expect("one module");
    }
    symbols.register_source_import(declarations[1].source_id, "left::Helper");
    let lookup = |symbols: &crate::SymbolTable| {
        symbols.find_top_level_by_name_and_kinds_from_source(
            "Helper",
            &[SymbolKind::Data],
            declarations[1],
        )
    };
    assert_eq!(lookup(&symbols), Some(handles[0]));
    let activated = symbols
        .begin_source_activation(Arc::new(sources.clone()), Vec::new())
        .expect("same source custody, no loader bindings")
        .finish();
    assert_eq!(lookup(&activated), Some(handles[0]));
    let mut selected = sources;
    selected.retain_selected_sources(|file| file.source_id != declarations[0].source_id);
    let activated = activated
        .begin_source_activation(Arc::new(selected), Vec::new())
        .expect("remove the exact imported owner")
        .finish();
    assert_eq!(
        lookup(&activated),
        None,
        "a same-name foreign module supplies no authority"
    );
}

#[test]
fn inactive_bindings_cannot_change_declaration_collision_scopes() {
    let dependency = PackageKeyIdentity::from_digest([15; 32]).expect("dependency");
    let consumer = PackageKeyIdentity::from_digest([16; 32]).expect("consumer");
    let mut sources = SourceMap::default();
    let declarations = [
        (
            "authored",
            dependency,
            source::SourceResolutionStratum::Base,
        ),
        (
            "generated",
            dependency,
            source::SourceResolutionStratum::DependencyActivationExtension,
        ),
        ("consumer", consumer, source::SourceResolutionStratum::Base),
        (
            "sibling",
            dependency,
            source::SourceResolutionStratum::DependencyActivationExtension,
        ),
    ]
    .map(|(name, owner, stratum)| {
        sources
            .add_checked_instance(
                PathBuf::from(format!("{name}.omg")),
                "Item",
                PathBuf::from("."),
                Some(owner),
                SourceOrigin::User,
                stratum,
                source::DependencyScope::Product,
            )
            .source_span(Span::new(0, 4))
    });
    let bindings = vec![
        crate::SourceScopedTopLevelBinding::module_import(
            declarations[2].source_id,
            declarations[0].source_id,
            "leaf::authored",
            1,
        ),
        crate::SourceScopedTopLevelBinding::new(
            declarations[0].source_id,
            declarations[1].source_id,
            "Item",
        ),
    ];
    let mut builder = SymbolTableBuilder::with_sources_and_top_level_bindings(
        Some(Arc::new(sources.clone())),
        bindings,
    );
    let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
    let handles = SymbolTableBuilder::child_handles(builder.insert_children(
        root,
        declarations.map(|span| (SymbolKind::Data, SymbolNameRef::Source(span))),
    ))
    .collect::<Vec<_>>();
    let mut current = sources.fork_source_selection().expect("sealed owners");
    for file in sources.files().skip(1) {
        current.select_checked_instance(
            file.path.clone(),
            file.source.clone(),
            file.package_root.clone(),
            file.package_identity,
            file.origin,
            file.resolution_stratum,
            file.dependency_scope,
        );
    }
    let selected = builder
        .finish()
        .begin_extension(Some(Arc::new(current)), Vec::new())
        .finish();
    assert!(
        selected.source_scopes_separate(handles[2], handles[1]),
        "an inactive import cannot expose a generated sibling into the consumer"
    );
    assert!(
        !selected.source_scopes_separate(handles[1], handles[3]),
        "an inactive explicit binding cannot separate two current sibling declarations"
    );
}

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
        symbols.insert_generated_children_from(next, [(handles[1], SymbolKind::Local, "second")]),
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
    let duplicate =
        symbols.insert_generated_root_from(authored[0], SymbolKind::Machine, "read$specialized");
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

/// A BUILD operand may name a public toolchain declaration; ordinary product
/// declaration selection may not.
///
/// The toolchain injects its core into every product with no package identity
/// and no manifest, so neither a same-package test nor an alias can ever reach
/// it. Widening the ordinary lookup instead would change which declaration an
/// ordinary call selects.
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
