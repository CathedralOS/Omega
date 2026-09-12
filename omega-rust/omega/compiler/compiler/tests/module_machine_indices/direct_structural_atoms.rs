use super::*;
use compiler::CheckedCompileRequest;

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
        let errors = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
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
        let result = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
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
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("direct dependency does not expose private carrier");
}

#[test]
fn qualified_record_constructors_preserve_nominal_identity_and_authored_occurrences() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Leaf { count: u64; } pub data Value { leaf: Leaf; enabled: bool; }",
    );
    let narrow = "(Value { leaf: Leaf { count: 1 }, enabled: true })";
    let qualified = "(settings::Value { enabled: true, leaf: settings::Leaf { count: 1 } })";
    let source = format!(
        "use settings::Value; use settings::Leaf; data Pick<const V: settings::Value> {{ marker: u8; }}
         machine narrow(value: Pick<{narrow}>) -> Pick<{narrow}> {{ let local: Pick<{narrow}> = value; local }}
         machine qualified(value: Pick<{qualified}>) -> Pick<{qualified}> {{ let local: Pick<{qualified}> = value; local }}"
    );
    Sources::write(root.join("main.omg"), &source);
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "narrow", "qualified");
    for expression in [narrow, qualified] {
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
fn qualified_case_constructors_preserve_nominal_identity_and_authored_occurrences() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Value { case Empty; case Some(count: u64); }",
    );
    let narrow = "(Value::Some { count: 1 })";
    let qualified = "(settings::Value::Some { count: 1 })";
    let source = format!(
        "use settings::Value; data Pick<const V: settings::Value> {{ marker: u8; }}
         machine narrow(value: Pick<{narrow}>) -> Pick<{narrow}> {{ let local: Pick<{narrow}> = value; local }}
         machine qualified(value: Pick<{qualified}>) -> Pick<{qualified}> {{ let local: Pick<{qualified}> = value; local }}"
    );
    Sources::write(root.join("main.omg"), &source);
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "narrow", "qualified");
    for expression in [narrow, qualified] {
        for target in [
            "settings::Value",
            "settings::Value::Some",
            "settings::Value::Some::count",
        ] {
            occurrence_selects(&checked, &root, &source, expression, target);
        }
    }
}

#[test]
fn qualified_constructor_selection_preserves_same_leaf_ambiguity_and_nominal_mismatch() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["left", "right"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; pub data Value {{ count: u64; }}"),
        );
    }
    let prefix =
        "use left::Value; use right::Value; data Pick<const V: left::Value> { marker: u8; }";
    for expression in ["(Value { count: 1 })", "(right::Value { count: 1 })"] {
        Sources::write(
            root.join("main.omg"),
            &format!("{prefix} {}", keep(expression)),
        );
        let errors = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("qualification must not erase ambiguity or nominal mismatch");
        let expected = if expression.starts_with("(right::") {
            "different nominal carrier"
        } else {
            "ambiguous constructor"
        };
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{errors:?}"
        );
    }
    Sources::write(
        root.join("main.omg"),
        &format!("{prefix} {}", keep("(left::Value { count: 1 })")),
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn qualified_constructor_selection_requires_direct_package_and_public_carrier() {
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
        "module settings; pub data Value { count: u64; }",
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
    let source = "use middle::bridge; use leaf::settings; machine make() -> leaf::settings::Value { leaf::settings::Value { count: 1 } }";
    Sources::write(root.join("main.omg"), source);
    let inputs =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a transitive dependency cannot authorize qualified construction");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let inputs = || {
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap()
    };
    let checked = compile(&root, inputs());
    let uses = selections(&checked, "settings::Value", identity(3));
    assert!(
        uses.len() >= 2,
        "return type and constructor retain exact package selection: {uses:?}"
    );
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; data Value { count: u64; }",
    );
    let errors = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("direct package access does not authorize a private constructor");
    assert!(
        errors.iter().any(|error| error.message.contains("private")),
        "{errors:?}"
    );
}

#[test]
fn qualified_constructor_ambiguous_records_cannot_fall_back_to_a_unique_case() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["left", "right"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}::Choice; pub data Value {{}}"),
        );
    }
    Sources::write(
        root.join("main.omg"),
        "use left::Choice; use right::Choice;
         data Choice { case Value; }
         machine make() -> Choice { Choice::Value {} }",
    );
    let result = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    });
    assert!(
        result.is_err(),
        "ambiguous imported records must not become the unique root case"
    );
    assert_constructor_ambiguity(&result.unwrap_err());
}

#[test]
fn qualified_constructor_ambiguous_case_owners_cannot_fall_back_to_a_unique_record() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["left", "right"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; pub data Choice {{ case Value; }}"),
        );
    }
    Sources::write(root.join("Choice.omg"), "module Choice; pub data Value {}");
    Sources::write(
        root.join("main.omg"),
        "use left::Choice; use right::Choice; use Choice;
         machine make() -> Choice::Value { Choice::Value {} }",
    );
    let result = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    });
    assert!(
        result.is_err(),
        "ambiguous imported case owners must not become the unique record"
    );
    assert_constructor_ambiguity(&result.unwrap_err());
}

fn assert_constructor_ambiguity(errors: &[diagnostics::Diagnostic]) {
    assert!(
        errors.iter().any(|error| {
            error.source_span.is_some()
                && [
                    "ambiguous constructor `Choice::Value`",
                    "left::Choice::Value",
                    "right::Choice::Value",
                    "source imports:",
                    "left::Choice",
                    "right::Choice",
                ]
                .iter()
                .all(|expected| error.message.contains(expected))
        }),
        "constructor rejection must retain the competing declarations and source import context: {errors:?}"
    );
}

#[test]
fn qualified_constructor_ambiguous_prefixes_without_the_case_do_not_compete() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["left", "right"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; pub data Choice {{ case Other; }}"),
        );
    }
    Sources::write(root.join("Choice.omg"), "module Choice; pub data Value {}");
    Sources::write(
        root.join("main.omg"),
        "use left::Choice; use right::Choice; use Choice;
         machine make() -> Choice::Value { Choice::Value {} }",
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn qualified_constructor_case_eligibility_precedes_same_leaf_owner_ambiguity() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, case) in [("left", "Value"), ("right", "Other")] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; pub data Choice {{ case {case}; }}"),
        );
    }
    Sources::write(
        root.join("main.omg"),
        "use left::Choice; use right::Choice;
         machine make() -> left::Choice { Choice::Value {} }",
    );
    let checked = compile(&root, root_inputs(&root));
    assert!(!selections(&checked, "left::Choice::Value", identity(1)).is_empty());
}

#[test]
fn qualified_bare_cases_preserve_value_construction_obligations() {
    for (declaration, body, expected) in [
        (
            "pub data Choice { case Empty; case Some(value: u32); }",
            "settings::Choice::Empty",
            None,
        ),
        (
            "pub data Choice { case Empty; case Some(value: u32); }",
            "settings::Choice::Some",
            Some("has a payload"),
        ),
        (
            "pub data Choice { value: u32 [1..=9]; case Empty; }",
            "settings::Choice::Empty",
            Some("omits gated field"),
        ),
        (
            "pub data Choice { case Empty; }",
            "settings::Choice::Missing",
            Some("not a declared local, parameter, field, or type"),
        ),
        (
            "pub data Choice { case Empty; }",
            "settings::Choice",
            Some("type"),
        ),
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        Sources::write(
            root.join("settings.omg"),
            &format!("module settings; {declaration}"),
        );
        Sources::write(
            root.join("main.omg"),
            &format!("use settings; machine make() -> settings::Choice {{ {body} }}"),
        );
        let result = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        match expected {
            None => {
                result.expect("selected payload-free case is a value");
            }
            Some(expected) => {
                let errors = result.expect_err("case construction must retain its obligations");
                assert!(
                    errors.iter().any(|error| error.message.contains(expected)),
                    "{errors:?}"
                );
            }
        }
    }
}

#[test]
fn qualified_bare_case_checks_preserve_payload_case_membership() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        "data Choice { case Empty; case Some(value: u32); }
         machine matches(value: &Choice) -> bool { value in Choice::Some }
         machine braces() -> Choice { Choice::Some {} }",
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn qualified_bare_cases_do_not_turn_equality_into_membership_or_erase_nominal_identity() {
    for (source, expected) in [
        (
            "use settings; machine compare() -> bool { settings::Choice::Some == settings::Choice::Some }",
            "has a payload",
        ),
        (
            "use settings; data Other { case Empty; } machine make() -> Other { settings::Choice::Empty }",
            "terminal expression",
        ),
        (
            "use settings; machine make(settings: u32) -> settings::Choice { settings::Choice::Empty }",
            "static path through runtime binding `settings`",
        ),
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        Sources::write(
            root.join("settings.omg"),
            "module settings; pub data Choice { case Empty; case Some(value: u32); }",
        );
        Sources::write(root.join("main.omg"), source);
        let errors = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("value expressions preserve constructor and lexical ownership");
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{errors:?}"
        );
    }
}

#[test]
fn qualified_bare_case_selection_requires_direct_package_and_public_carrier() {
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
        "module settings; pub data Value { case Empty; }",
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
    let source = "use middle::bridge; use leaf::settings::Value; machine make() -> leaf::settings::Value { Value::Empty }";
    Sources::write(root.join("main.omg"), source);
    let inputs =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("a transitive dependency cannot authorize qualified construction");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let inputs = || {
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap()
    };
    let checked = compile(&root, inputs());
    let uses = selections(&checked, "settings::Value", identity(3));
    assert!(
        uses.len() >= 2,
        "return type and constructor retain exact package selection: {uses:?}"
    );
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; data Value { case Empty; }",
    );
    let errors = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs()),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("direct package access does not authorize a private constructor");
    assert!(
        errors.iter().any(|error| error.message.contains("private")),
        "{errors:?}"
    );
}
