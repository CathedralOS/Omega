use super::{Sources, compile, identity, root_inputs, selections};

#[test]
fn module_record_constant_retains_nominal_carrier_in_machine_indices() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
         pub data Value [copy] { value: u64; }
         pub const VALUE: Value = Value { value: 1 };",
    );
    // The generic remains root-owned: this customer needs a selected nominal
    // constant and carrier, independently of module-owned template support.
    Sources::write(
        root.join("main.omg"),
        "use settings;
         data Pick<const V: settings::Value> { marker: u8; }
         machine keep(value: Pick<settings::VALUE>) -> Pick<settings::VALUE> { value }",
    );
    let checked = compile(&root, root_inputs(&root));
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == "keep")
        .expect("nominal index consumer");
    let [state] = checked.typed.machine_states(machine) else {
        panic!("one machine state")
    };
    let [parameter] = checked.typed.state_parameters(state) else {
        panic!("one indexed parameter")
    };
    assert_eq!(
        checked
            .typed
            .normalized_type_identity(parameter.type_reference),
        checked.typed.normalized_type_identity(state.return_type),
    );
    let uses = selections(&checked, "settings::VALUE", identity(1));
    assert_eq!(
        uses.len(),
        2,
        "each signature occurrence selects the constant"
    );
    assert_ne!(uses[0].source_span(), uses[1].source_span());
}

fn keep(name: &str, index: &str) -> String {
    format!(
        "machine {name}(value: Pick<{index}>) -> Pick<{index}> {{ let local: Pick<{index}> = value; local }}"
    )
}

#[test]
fn module_record_indices_preserve_field_order_and_nested_nominal_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings;
         pub data Leaf [copy] { count: u64; }
         pub data Value [copy] { leaf: Leaf; enabled: bool; }
         pub const VALUE: Value = Value { leaf: Leaf { count: 1 }, enabled: true };
         pub const REORDERED: Value = Value { enabled: true, leaf: Leaf { count: 1 } };
         pub const OTHER: Value = Value { leaf: Leaf { count: 2 }, enabled: true };",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; data Leaf [copy] {{ other: bool; }} data Value [copy] {{ other: u8; }}
         data Pick<const V: settings::Value> {{ marker: u8; }} {} {} {}",
            keep("keep", "settings::VALUE"),
            keep("reordered", "settings::REORDERED"),
            keep("other", "settings::OTHER")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    super::assert_same_machine_types(&checked, "keep", "reordered");
    assert_ne!(
        checked
            .typed
            .normalized_type_identity(super::machine_types(&checked, "keep")[0]),
        checked
            .typed
            .normalized_type_identity(super::machine_types(&checked, "other")[0])
    );
    for name in ["VALUE", "REORDERED", "OTHER"] {
        assert_eq!(
            selections(&checked, &format!("settings::{name}"), identity(1)).len(),
            3
        );
    }
}

#[test]
fn module_nominal_indices_reject_equal_layout_root_carriers_and_constructors() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Leaf [copy] { count: u64; } pub data Value [copy] { leaf: Leaf; }
         pub const VALUE: Value = Value { leaf: Leaf { count: 1 } };",
    );
    for declaration in [
        "const WRONG: Value = Value { leaf: Leaf { count: 1 } };",
        "const WRONG: settings::Value = Value { leaf: Leaf { count: 1 } };",
        "const WRONG: settings::Value = settings::Value { leaf: Leaf { count: 1 } };",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; data Leaf [copy] {{ count: u64; }} data Value [copy] {{ leaf: Leaf; }}
             {declaration} data Pick<const V: settings::Value> {{ marker: u8; }} {}",
                keep("keep", "WRONG")
            ),
        );
        let diagnostics = super::compile_to_checked_with_packages(
            &root.join("main.omg"),
            None,
            root_inputs(&root),
        )
        .expect_err(
            "equal record layout does not establish nominal carrier or constructor identity",
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("carrier")
                    || diagnostic.message.contains("requires")
                    || diagnostic.message.contains("canonical value")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn module_case_and_fixed_array_indices_share_structural_canonicalization() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Value [copy] { case Empty; case Some(count: u64); }
         pub const EMPTY: Value = Value::Empty;
         pub const SOME: Value = Value::Some { count: 1 };
         pub const VALUES: [Value; 2] = [Value::Empty, Value::Some { count: 1 }];",
    );
    for (carrier, index) in [
        ("settings::Value", "settings::EMPTY"),
        ("settings::Value", "settings::SOME"),
        ("[settings::Value; 2]", "settings::VALUES"),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; data Pick<const V: {carrier}> {{ marker: u8; }} {}",
                keep("keep", index)
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_eq!(selections(&checked, index, identity(1)).len(), 3);
    }
}

#[test]
fn module_nominal_indices_require_direct_public_package_selection() {
    use super::{PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding};
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::settings; pub machine bridge() -> u64 { 1 }",
    );
    let leaf_source = |visibility: &str| {
        format!(
            "module settings; pub data Value [copy] {{ value: u64; }} {visibility} const VALUE: Value = Value {{ value: 1 }};"
        )
    };
    Sources::write(leaf.join("settings.omg"), &leaf_source("pub"));
    let sources = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let mut dependencies = vec![
        PackageDependencyBinding::new(identity(1), "middle", identity(2)),
        PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
    ];
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use middle::bridge; data Pick<const V: settings::Value> {{ marker: u8; }} {}",
            keep("keep", "settings::VALUE")
        ),
    );
    let indirect =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    super::compile_to_checked_with_packages(&root.join("main.omg"), None, indirect)
        .expect_err("loading a transitive module grants no nominal selection");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let direct = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use leaf::settings; data Pick<const V: leaf::settings::Value> {{ marker: u8; }} {}",
            keep("keep", "leaf::settings::VALUE")
        ),
    );
    compile(&root, direct.clone());
    Sources::write(leaf.join("settings.omg"), &leaf_source(""));
    super::compile_to_checked_with_packages(&root.join("main.omg"), None, direct.clone())
        .expect_err("direct dependency does not expose private nominal constant");
    Sources::write(
        leaf.join("settings.omg"),
        &leaf_source("pub").replace("pub data", "data"),
    );
    super::compile_to_checked_with_packages(&root.join("main.omg"), None, direct)
        .expect_err("public constant does not expose a private nominal carrier");
}

#[test]
fn root_nominal_domain_indices_remain_available() {
    let tree = Sources::new();
    let root = tree.package("root");
    for owner in [
        "data Use { value: u64 in Indexed<VALUE>; }",
        "machine keep(value: u64 in Indexed<VALUE>) -> u64 in Indexed<VALUE> { let local: u64 in Indexed<VALUE> = value; local }",
        "machine keep(value: u64 in Indexed<VALUE>) -> u64 in Indexed<VALUE> { value as u64 in Indexed<VALUE> }",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "data Value [copy] {{ value: u64; }} const VALUE: Value = Value {{ value: 1 }};
            domain<T, const V: Value> T::Indexed<V>; {owner}"
            ),
        );
        compile(&root, root_inputs(&root));
    }
}

#[test]
fn nominal_constant_import_ambiguity_rejects_in_both_orders() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["first", "second"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; pub data Value [copy] {{ value: u64; }} pub const VALUE: Value = Value {{ value: 1 }};"
            ),
        );
    }
    for imports in [
        "use first::VALUE; use second::VALUE;",
        "use second::VALUE; use first::VALUE;",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} data Pick<const V: first::Value> {{ marker: u8; }} {}",
                keep("keep", "first::VALUE")
            ),
        );
        compile(&root, root_inputs(&root));
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} data Pick<const V: first::Value> {{ marker: u8; }} {}",
                keep("keep", "VALUE")
            ),
        );
        super::compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err(
                "ambiguous named constants do not select by traversal order or equal layout",
            );
    }
}

#[test]
fn module_scoped_nominal_indices_retain_their_attachment_owner() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Scope {} pub data Value [copy] { value: u64; }
        pub const Scope::VALUE: Value = Value { value: 1 };",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; data Scope {{}} data Pick<const V: settings::Value> {{ marker: u8; }} {}",
            keep("keep", "settings::Scope::VALUE")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_eq!(
        selections(&checked, "settings::Scope::VALUE", identity(1)).len(),
        3
    );
}
