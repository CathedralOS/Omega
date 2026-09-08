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

fn constant_selections(
    checked: &checked_trees::CheckedTrees,
) -> Vec<(String, Option<PackageKeyIdentity>)> {
    checked.authored_declaration_selections().iter().filter_map(|selection| {
        let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(target) = selection.target() else {
            return None;
        };
        let symbol = target.selected_symbol();
        let path = checked.symbols.display_path(symbol, "::");
        path.ends_with("::DAMAGE").then(|| (path, checked.symbols.symbol_package_identity(symbol)))
    }).collect()
}

#[test]
fn module_constants_retain_local_qualified_and_imported_declaration_identity() {
    let tree = TempTree::new();
    let root = tree.package("root");
    for (module, value) in [("combat", 7), ("rooms", 9)] {
        TempTree::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; const DAMAGE: u64 = {value}; machine local() -> u64 {{ DAMAGE }}"
            ),
        );
    }
    for imports in [
        "use combat::DAMAGE; use rooms;",
        "use rooms; use combat::DAMAGE;",
    ] {
        TempTree::write(
            root.join("main.omg"),
            &format!(
                "{imports}
             machine leaf() -> u64 {{ DAMAGE }}
             machine first() -> u64 {{ combat::DAMAGE }}
             machine second() -> u64 {{ rooms::DAMAGE }}"
            ),
        );
        let checked =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect("module constants remain distinct in both source discovery orders");
        let selections = constant_selections(&checked);
        for path in ["combat::DAMAGE", "rooms::DAMAGE"] {
            assert!(
                selections
                    .iter()
                    .any(|row| row == &(path.to_owned(), Some(identity(1)))),
                "{selections:?}"
            );
        }
    }
}

#[test]
fn public_module_constants_keep_exact_dependency_owners() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let first = tree.package("first");
    let second = tree.package("second");
    for (package, value) in [(&first, 7), (&second, 9)] {
        TempTree::write(
            package.join("combat.omg"),
            &format!("module combat; pub const DAMAGE: u64 = {value};"),
        );
    }
    TempTree::write(
        root.join("main.omg"),
        "use first::combat::DAMAGE; use second::combat;
         machine first_value() -> u64 { first::combat::DAMAGE }
         machine second_value() -> u64 { second::combat::DAMAGE }",
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
    .unwrap();
    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("qualified constants select exact direct dependency owners");
    let selections = constant_selections(&checked);
    for owner in [identity(2), identity(3)] {
        assert!(
            selections.contains(&("combat::DAMAGE".to_owned(), Some(owner))),
            "{selections:?}"
        );
    }
}

#[test]
fn private_module_constant_import_rejects_even_when_unused() {
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
    .unwrap();
    for body in ["DAMAGE", "0"] {
        TempTree::write(
            root.join("main.omg"),
            &format!("use dep::combat::DAMAGE; machine value() -> u64 {{ {body} }}"),
        );
        TempTree::write(
            dependency.join("combat.omg"),
            "module combat; pub const DAMAGE: u64 = 7;",
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, inputs.clone())
            .expect("public constant import has an otherwise valid source fixture");
        TempTree::write(
            dependency.join("combat.omg"),
            "module combat; const DAMAGE: u64 = 7;",
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, inputs.clone())
                .expect_err("using or merely importing a private constant cannot publish it");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("private")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn module_constant_selection_does_not_gain_transitive_dependency_authority() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(
        middle.join("bridge.omg"),
        "use leaf::combat; pub machine bridge() -> u64 { leaf::combat::DAMAGE }",
    );
    TempTree::write(
        leaf.join("combat.omg"),
        "module combat; pub const DAMAGE: u64 = 7;",
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
    TempTree::write(
        root.join("main.omg"),
        "use middle::bridge; machine value() -> u64 { combat::DAMAGE }",
    );
    let transitive =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    compile_to_checked_with_packages(&root.join("main.omg"), None, transitive)
        .expect_err("a loaded transitive constant retains its declaration selection boundary");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    TempTree::write(
        root.join("main.omg"),
        "use middle::bridge; use leaf::combat::DAMAGE; machine value() -> u64 { DAMAGE }",
    );
    let direct = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, direct)
        .expect("a direct edge authorizes the public constant");
    assert!(
        constant_selections(&checked).contains(&("combat::DAMAGE".to_owned(), Some(identity(3))))
    );
}

#[test]
fn ambiguous_module_constant_leaves_reject_in_both_import_orders() {
    let tree = TempTree::new();
    let root = tree.package("root");
    for (module, value) in [("combat", 7), ("rooms", 9)] {
        TempTree::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; const DAMAGE: u64 = {value};"),
        );
    }
    for imports in [
        "use combat::DAMAGE; use rooms::DAMAGE;",
        "use rooms::DAMAGE; use combat::DAMAGE;",
    ] {
        TempTree::write(
            root.join("main.omg"),
            &format!("{imports} machine value() -> u64 {{ DAMAGE }}"),
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("ambiguous constants cannot select by declaration traversal order");
    }
}

#[test]
fn lexical_values_shadow_module_constants_without_selecting_them() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("combat.omg"),
        "module combat; const DAMAGE: u64 = 7;
         machine parameter(DAMAGE: u64) -> u64 { DAMAGE }
         machine local() -> u64 { let DAMAGE: u64 = 9; DAMAGE }",
    );
    TempTree::write(root.join("main.omg"), "use combat; data Main {}");
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("lexical values precede constants in module lookup");
    assert!(
        constant_selections(&checked).is_empty(),
        "shadowed constant is not an authored selection"
    );
}
