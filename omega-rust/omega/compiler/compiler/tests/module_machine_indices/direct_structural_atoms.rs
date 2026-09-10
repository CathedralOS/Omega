use super::*;

fn keep(index: &str) -> String {
    format!(
        "machine keep(value: Pick<{index}>) -> Pick<{index}> {{ let local: Pick<{index}> = value; local }}"
    )
}

fn occurrence_selects(
    checked: &CheckedCompilation,
    root: &Path,
    source: &str,
    expression: &str,
    target: &str,
) {
    let path = root.join("main.omg").canonicalize().unwrap();
    let uses = selections(checked, target, identity(1))
        .into_iter()
        .filter(|selection| {
            checked
                .symbols
                .source_file(selection.source_span())
                .is_some_and(|source| source.path == path)
        })
        .collect::<Vec<_>>();
    for (start, _) in source.match_indices(expression) {
        let end = start + expression.len();
        assert!(
            uses.iter().any(|selection| {
                let span = selection.source_span().span;
                (start..end).contains(&(span.start))
            }),
            "each authored {expression} must retain selection of {target}: {uses:?}"
        );
    }
}

#[test]
fn direct_records_preserve_equal_value_identity_and_each_authored_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Leaf { count: u64; } pub data Value { leaf: Leaf; enabled: bool; }",
    );
    let first = "(Value { leaf: Leaf { count: 1 }, enabled: true })";
    let reordered = "(Value { enabled: true, leaf: Leaf { count: 1 } })";
    let source = format!(
        "use settings::Value; use settings::Leaf; data Pick<const V: settings::Value> {{ marker: u8; }}
         machine first(value: Pick<{first}>) -> Pick<{first}> {{ let local: Pick<{first}> = value; local }}
         machine reordered(value: Pick<{reordered}>) -> Pick<{reordered}> {{ let local: Pick<{reordered}> = value; local }}"
    );
    Sources::write(root.join("main.omg"), &source);
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "first", "reordered");
    for expression in [first, reordered] {
        for target in [
            "settings::Value",
            "settings::Value::leaf",
            "settings::Value::enabled",
            "settings::Leaf",
            "settings::Leaf::count",
        ] {
            occurrence_selects(&checked, &root, &source, expression, target);
        }
    }
}

#[test]
fn direct_case_and_array_atoms_retain_constructor_occurrences() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Value { case Empty; case Some(count: u64); }",
    );
    for (carrier, expression) in [
        ("settings::Value", "(Value::Some { count: 1 })"),
        (
            "[settings::Value; 2]",
            "([Value::Empty, Value::Some { count: 1 }])",
        ),
    ] {
        let source = format!(
            "use settings::Value; data Pick<const V: {carrier}> {{ marker: u8; }} {}",
            keep(expression)
        );
        Sources::write(root.join("main.omg"), &source);
        let checked = compile(&root, root_inputs(&root));
        let types = machine_types(&checked, "keep");
        assert_eq!(
            checked.typed.normalized_type_identity(types[0]),
            checked.typed.normalized_type_identity(types[1])
        );
        occurrence_selects(
            &checked,
            &root,
            &source,
            expression,
            "settings::Value::Some",
        );
        occurrence_selects(
            &checked,
            &root,
            &source,
            expression,
            "settings::Value::Some::count",
        );
        if carrier.starts_with('[') {
            occurrence_selects(
                &checked,
                &root,
                &source,
                expression,
                "settings::Value::Empty",
            );
        }
    }
}

#[test]
fn direct_atom_constructor_selection_rejects_wrong_carrier_and_allows_package_private_import() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Value { value: u64; }",
    );
    {
        let expression = "(Value { value: 1 })";
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; data Value {{ value: u64; }} data Pick<const V: settings::Value> {{ marker: u8; }} {}",
                keep(expression)
            ),
        );
        let errors =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("equal layout does not establish exact constructor identity");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("different nominal carrier")),
            "{errors:?}"
        );
    }
    Sources::write(
        root.join("settings.omg"),
        "module settings; data Value { value: u64; }",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings::Value; data Pick<const V: Value> {{ marker: u8; }} {}",
            keep("(Value { value: 1 })")
        ),
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn direct_rat_atoms_retain_canonicality_without_named_const_copy_permission() {
    let tree = Sources::new();
    let root = tree.package("root");
    let zero = "Nat::Zero";
    let one = "Nat::Succ { prev: Nat::Zero }";
    let two = "Nat::Succ { prev: Nat::Succ { prev: Nat::Zero } }";
    let four = format!("Nat::Succ {{ prev: Nat::Succ {{ prev: {two} }} }}");
    for (negative, positive, denominator, expected) in [
        (zero, one, two, None),
        (one, zero, two, None),
        (zero, one, zero, Some("denominator must be positive")),
        (one, one, two, Some("signed coordinates must be cancelled")),
        (zero, two, four.as_str(), Some("must be gcd-reduced")),
    ] {
        let expression = format!(
            "(Rat {{ num: IntPair {{ neg: {negative}, pos: {positive} }}, den: {denominator} }})"
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use omega::language::core::rat; data Pick<const V: Rat> {{ marker: u8; }} {}",
                keep(&expression)
            ),
        );
        let result =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root));
        if let Some(expected) = expected {
            let errors = result.expect_err("noncanonical direct rational index must reject");
            assert!(
                errors.iter().any(|error| error.message.contains(expected)),
                "{errors:?}"
            );
        } else {
            result.expect("canonical signed Rat expression is an index without a named constant");
        }
    }
}

#[test]
fn direct_atom_package_selection_requires_direct_dependency_and_public_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::settings; pub machine bridge() -> u64 { 1 }",
    );
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; pub data Value { value: u64; }",
    );
    let sources = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle),
        PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
    ];
    let mut dependencies = vec![
        PackageDependencyBinding::new(identity(1), "middle", identity(2)),
        PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
    ];
    let expression = "(Value { value: 1 })";
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use middle::bridge; use leaf::settings::Value; data Pick<const V: leaf::settings::Value> {{ marker: u8; }} {}",
            keep(expression)
        ),
    );
    let inputs = || {
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap()
    };
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs())
        .expect_err("transitive loading cannot authorize direct constructor selection");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use leaf::settings::Value; data Pick<const V: Value> {{ marker: u8; }} {}",
            keep("(Value { value: 1 })")
        ),
    );
    let inputs = || {
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap()
    };
    let checked = compile(&root, inputs());
    let uses = selections(&checked, "settings::Value", identity(3));
    assert!(
        uses.len() >= 4,
        "import, binder and both constructors select the exact dependency: {uses:?}"
    );
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; data Value { value: u64; }",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs())
        .expect_err("direct dependency does not expose private carrier");
}
