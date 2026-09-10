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

#[test]
fn public_float_constants_retain_landed_identity_and_exact_import_owner() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    TempTree::write(
        dependency.join("settings.omg"),
        r#"
        module settings;
        pub const SCALE: f32 = 1.5;
        pub const SAME: f32 = 1.500;
        pub const DIFFERENT: f32 = 2.5;
        pub const WIDE: f64 = 1.5;
        pub const WHOLE: f32 = 0x3;
        pub const THREE: f32 = 3.0;
        pub const WIDE_POSITIVE_ZERO: f64 = 0.0;
        pub const WIDE_NEGATIVE_ZERO: f64 = -0.0;
        pub const ROUNDED: f32 = 16777217.0;
        pub const LANDED: f32 = 16777216.0;
        pub const ABOVE_MIDPOINT: f32 = 1.00000005960464477539062500000000000000000000000000001;
        pub const NEXT: f32 = 1.00000011920928955078125;
        pub const ONE: f32 = 1.0;
        pub const POSITIVE_ZERO: f32 = 0.0;
        pub const NEGATIVE_ZERO: f32 = -0.0;
    "#,
    );
    TempTree::write(
        root.join("main.omg"),
        r#"
        use dep::settings::SCALE;
        machine narrow() -> f32 { SCALE }
        machine qualified() -> f32 { dep::settings::SCALE }
        machine rounded() -> f32 { dep::settings::ROUNDED }
    "#,
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "dependency", dependency),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "dep",
            identity(2),
        )],
    )
    .expect("one direct dependency");
    let checked = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect("public finite floating constants retain declaration identity without becoming generic atoms");
    let encoding = |name: &str| {
        let declaration = checked
            .const_declarations()
            .iter()
            .find(|declaration| {
                checked.symbols.display_path(declaration.symbol, "::")
                    == format!("settings::{name}")
            })
            .expect("exact public constant");
        assert_eq!(
            checked.symbols.symbol_package_identity(declaration.symbol),
            Some(identity(2))
        );
        assert!(declaration.is_public);
        declaration
            .canonical_value_encoding
            .as_deref()
            .expect("public declaration encoding")
    };
    assert_eq!(encoding("SCALE"), encoding("SAME"));
    assert_ne!(encoding("SCALE"), encoding("DIFFERENT"));
    assert_ne!(encoding("SCALE"), encoding("WIDE"));
    assert_eq!(encoding("WHOLE"), encoding("THREE"));
    assert_ne!(
        encoding("WIDE_POSITIVE_ZERO"),
        encoding("WIDE_NEGATIVE_ZERO")
    );
    assert_eq!(encoding("ROUNDED"), encoding("LANDED"));
    assert_eq!(encoding("ABOVE_MIDPOINT"), encoding("NEXT"));
    assert_ne!(encoding("ABOVE_MIDPOINT"), encoding("ONE"));
    assert_ne!(encoding("POSITIVE_ZERO"), encoding("NEGATIVE_ZERO"));
    for (name, expected) in [("narrow", 1.5), ("qualified", 1.5), ("rounded", 16777216.0)] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("constant consumer");
        let value = build_time_evaluation::BuildTimeAdmissionPlan::infer(&checked.typed)
            .evaluate_machine_symbol_for_invocation_measured(
                &checked.typed,
                machine.symbol,
                vec![],
                build_time_evaluation::BuildTimeInvocationCustody::Symbol(machine.symbol),
            )
            .expect("public constant body evaluates");
        assert_eq!(
            value.value(),
            &build_time_evaluation::BuildTimeValue::Float(expected),
            "{name}"
        );
    }
}

#[test]
fn public_float_declarations_do_not_admit_floating_generic_indices() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("settings.omg"),
        "module settings; pub const SCALE: f32 = 1.5;",
    );
    for argument in ["settings::SCALE", "1.5f32"] {
        TempTree::write(
            root.join("main.omg"),
            &format!(
                "use settings; data Pick<const V: f32> {{ marker: u8; }} machine keep(value: Pick<{argument}>) -> Pick<{argument}> {{ value }}"
            ),
        );
        let errors =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err(
                    "a public declaration encoding does not make Float a canonical index carrier",
                );
        assert!(
            errors.iter().any(
                |error| error.message.contains("not eligible as a const index")
                    || error
                        .message
                        .contains("expression is not a symbolic integer const expression")
            ),
            "{errors:?}"
        );
    }
}

#[test]
fn public_float_declarations_do_not_admit_machine_or_domain_indices() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("settings.omg"),
        "module settings; pub const SCALE: f32 = 1.5;",
    );
    for source in [
        "use settings; machine choose<const V: f32>() -> u64 { 7 } machine main() -> u64 { choose<settings::SCALE>() }",
        "use settings; machine choose<const V: f32>() {} machine main() { choose<settings::SCALE>(); }",
        "use settings; domain<T, const V: f32> T::Scaled<V>; machine keep(value: u64 in Scaled<settings::SCALE>) -> u64 in Scaled<settings::SCALE> { value }",
    ] {
        TempTree::write(root.join("main.omg"), source);
        let errors =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err(
                    "public Float identity is not a proof-static atom even for an unused binder",
                );
        assert!(
            errors.iter().any(
                |error| error.message.contains("no eligible canonical value")
                    || error.message.contains("not eligible as a const index")
            ),
            "{errors:?}"
        );
    }
}

#[test]
fn public_float_identity_requires_finite_literals_with_matching_landings() {
    let tree = TempTree::new();
    let root = tree.package("root");
    for (initializer, diagnostic) in [
        ("1e9999", "requires a finite landed value"),
        ("1.5f64", "conflicts with declared floating carrier"),
        ("1.0 + 0.5", "conflicts with declared floating carrier"),
    ] {
        TempTree::write(
            root.join("main.omg"),
            &format!("pub const VALUE: f32 = {initializer};"),
        );
        let errors =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("declaration encoding preserves literal landing limits");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains(diagnostic)),
            "{errors:?}"
        );
    }
}
