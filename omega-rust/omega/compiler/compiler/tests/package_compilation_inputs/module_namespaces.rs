use super::*;

fn root_inputs(root: &Path) -> PackageCompilationInputs {
    PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(
            identity(1),
            "root",
            root.to_path_buf(),
        )],
        Vec::new(),
    )
    .expect("one package")
}

#[test]
fn declaration_imports_join_the_loaded_module_not_the_file_name() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("combat.omg"),
        "module combat; data Damage { amount: u64; } machine damage() -> u64 { 7 }",
    );
    TempTree::write(
        root.join("main.omg"),
        r#"
        use combat::Damage;
        use combat::damage;
        data Attack { local: Damage; qualified: combat::Damage; }
        machine score() -> u64 { damage() }
        machine qualified_score() -> u64 { combat::damage() }
    "#,
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("module declarations, imported leaves, and qualified paths resolve");
    let paths = checked.authored_declaration_selections().iter().filter_map(|selection| {
        let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
            return None;
        };
        Some(checked.symbols.display_path(target.selected_symbol(), "::"))
    }).collect::<Vec<_>>();
    assert!(
        paths.iter().any(|path| path == "combat::Damage"),
        "{paths:?}"
    );
    assert!(
        paths.iter().any(|path| path.starts_with("combat::damage")),
        "{paths:?}"
    );
}

#[test]
fn same_leaf_declarations_in_distinct_modules_are_not_duplicates() {
    let tree = TempTree::new();
    let root = tree.package("root");
    for module in ["combat", "rooms"] {
        TempTree::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; data Point {{ value: u64; }} machine value() -> u64 {{ 7 }}"
            ),
        );
    }
    TempTree::write(
        root.join("main.omg"),
        r#"
        use combat;
        use rooms;
        data Pair { first: combat::Point; second: rooms::Point; }
        machine first() -> u64 { combat::value() }
        machine second() -> u64 { rooms::value() }
    "#,
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect("different modules own different Point declarations");
    TempTree::write(
        root.join("main.omg"),
        r#"
        use combat::Point;
        use rooms::Point;
        data Ambiguous { value: Point; }
    "#,
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("two imported Point declarations cannot select by traversal order");
}

#[test]
fn an_imported_file_cannot_fabricate_its_logical_module() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(root.join("main.omg"), "use combat::Damage; data Main {}");
    for text in [
        "module rooms; data Damage { value: u64; }",
        "data Damage { value: u64; }",
    ] {
        TempTree::write(root.join("combat.omg"), text);
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("even an unused import must name its actual module declaration");
    }
}

#[test]
fn a_module_is_not_a_value_receiver() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("combat.omg"),
        "module combat; machine damage() -> u64 { 7 }",
    );
    for call in ["combat::damage()", "combat.damage()"] {
        TempTree::write(
            root.join("main.omg"),
            &format!("use combat; machine score() -> u64 {{ {call} }}"),
        );
        let result =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root));
        if call.contains("::") {
            result.expect("static module selection resolves");
        } else {
            result.expect_err("a namespace does not become a runtime value receiver");
        }
    }
}

#[test]
fn a_module_is_not_a_nominal_data_type() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(root.join("combat.omg"), "module combat; data Damage {}");
    TempTree::write(
        root.join("main.omg"),
        "use combat; data Invalid { value: combat; }",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("a namespace cannot supply a nominal data definition");
}

#[test]
fn module_statement_calls_retain_static_selection() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(root.join("combat.omg"), "module combat; machine emit() {}");
    TempTree::write(
        root.join("main.omg"),
        "use combat; machine example() { combat::emit(); }",
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("statement calls select module-owned machines");
    assert!(checked.authored_declaration_selections().iter().any(|selection| {
        matches!(selection.target(), language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
            if checked.symbols.display_path(target.selected_symbol(), "::").starts_with("combat::emit"))
    }));
}

#[test]
fn package_aliases_and_visibility_survive_module_qualification() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "dependency", dependency.clone()),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("one direct dependency");
    TempTree::write(
        root.join("main.omg"),
        r#"
        use dep::combat::Damage;
        data Attack { value: dep::combat::Damage; }
    "#,
    );
    for public in [true, false] {
        TempTree::write(
            dependency.join("combat.omg"),
            &format!(
                "module combat; {}data Damage {{ value: u64; }}",
                if public { "pub " } else { "" }
            ),
        );
        let result = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs.clone());
        if public {
            let checked =
                result.expect("requester-local alias selects its public module declaration");
            assert!(checked.authored_declaration_selections().iter().any(|selection| {
                matches!(selection.target(), language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                    if checked.symbols.display_path(target.selected_symbol(), "::") == "combat::Damage"
                        && checked.symbols.symbol_package_identity(target.selected_symbol()) == Some(identity(2)))
            }));
        } else {
            let diagnostics =
                result.expect_err("qualification cannot publish a private declaration");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("private")),
                "{diagnostics:?}"
            );
        }
    }
    TempTree::write(
        root.join("main.omg"),
        "use dep::combat::Damage; data Unused {}",
    );
    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("an unused import still selects a private declaration");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("private")),
        "{diagnostics:?}"
    );
}

#[test]
fn identical_module_paths_in_different_packages_keep_exact_owners() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let first = tree.package("first");
    let second = tree.package("second");
    for package in [&first, &second] {
        TempTree::write(
            package.join("combat.omg"),
            "module combat; pub data Point { value: u64; }",
        );
    }
    TempTree::write(
        root.join("main.omg"),
        r#"
        use first::combat::Point;
        use second::combat::Point;
        data Pair { first: first::combat::Point; second: second::combat::Point; }
    "#,
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "first", first),
            PackageSourceBinding::new(identity(3), "second", second),
        ],
        vec![
            PackageDependencyBinding::new(identity(1), "first", identity(2)),
            PackageDependencyBinding::new(identity(1), "second", identity(3)),
        ],
    )
    .expect("two direct dependencies");
    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("package aliases distinguish otherwise equal module paths");
    for owner in [identity(2), identity(3)] {
        assert!(checked.authored_declaration_selections().iter().any(|selection| {
            matches!(selection.target(), language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target)
                if checked.symbols.display_path(target.selected_symbol(), "::") == "combat::Point"
                    && checked.symbols.symbol_package_identity(target.selected_symbol()) == Some(owner))
        }));
    }
}

#[test]
fn qualified_module_selection_requires_a_direct_dependency() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(
        root.join("main.omg"),
        "use middle::bridge; machine score() -> u64 { combat::damage() }",
    );
    TempTree::write(
        middle.join("bridge.omg"),
        "use leaf::combat; pub machine bridge() -> u64 { leaf::combat::damage() }",
    );
    TempTree::write(
        leaf.join("combat.omg"),
        "module combat; pub machine damage() -> u64 { 7 }",
    );
    let sources = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle),
        PackageSourceBinding::new(identity(3), "leaf", leaf),
    ];
    let mut dependencies = vec![
        PackageDependencyBinding::new(identity(1), "middle", identity(2)),
        PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
    ];
    let transitive =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .expect("transitive graph");
    compile_to_checked_with_packages(&root.join("main.omg"), None, transitive)
        .expect_err("loading a transitive module cannot grant selection authority");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let direct = PackageCompilationInputs::new_package(identity(1), sources, dependencies)
        .expect("direct graph");
    TempTree::write(
        root.join("main.omg"),
        "use middle::bridge; use leaf::combat; machine score() -> u64 { leaf::combat::damage() }",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, direct)
        .expect("explicit direct dependency admits the public module machine");
}
