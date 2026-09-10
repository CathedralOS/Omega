use super::{
    Sources, assert_same_machine_types, compile, compile_to_checked_with_packages, identity,
    root_inputs, selections,
};

fn keep(name: &str, carrier: &str, index: &str) -> String {
    format!(
        "machine {name}(value: {carrier}<{index}>) -> {carrier}<{index}> {{ let local: {carrier}<{index}> = value; local }}"
    )
}

#[test]
fn match_indices_select_one_arm_and_retain_canonical_type_identity() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "pub data Flag<const Enabled: bool> {{ value: u8; }}
             const ENABLED: bool = true; {} {}",
            keep(
                "keep",
                "Flag",
                "(match ENABLED { true -> true, false -> (1u64 / 0 == 0) })"
            ),
            keep("oracle", "Flag", "true"),
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "oracle");
}

#[test]
fn match_indices_compose_integer_boolean_and_anonymous_results() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, index, expected) in [
        (
            "bool",
            "(match true { true -> false, false -> true })",
            "false",
        ),
        ("u8", "(match true { true -> 7, false -> 9 })", "7"),
        (
            "u8",
            "(match ENABLED { true -> 7, true -> (1u8 / 0), _ -> 9 })",
            "7",
        ),
        ("u8", "(match LIMIT + 1 { 4 -> 7, _ -> (1u8 / 0) })", "7"),
        (
            "u8",
            "(match LIMIT { 3 -> 7, (1u8 / 0) -> 9, _ -> 11 })",
            "7",
        ),
        (
            "u8",
            "(match ENABLED { true -> (match false { true -> 9, false -> 7 }), false -> 11 })",
            "7",
        ),
        (
            "u8",
            "((match ENABLED { true -> 7 / 2, false -> 0 }) * 2)",
            "7",
        ),
        ("u8", "(1 + (match ENABLED { true -> 6, false -> 8 }))", "7"),
        (
            "bool",
            "((match ENABLED { true -> 7 / 2, false -> 0 }) == 3.5)",
            "true",
        ),
        ("u8", "(match 7 / 2 { 3.5 -> 7, _ -> 9 })", "7"),
        (
            "u64",
            "(match 18446744073709551615u64 { 18446744073709551615 -> 7, _ -> 9 })",
            "7",
        ),
        (
            "i64",
            "(match -9223372036854775808i64 { -9223372036854775808 -> -7, _ -> 9 })",
            "-7",
        ),
        (
            "bool",
            "(false || (match ENABLED { true -> true, _ -> false }))",
            "true",
        ),
        (
            "bool",
            "(match ENABLED { true -> true, false -> (match (1u8 / 0 == 0) { true -> true, false -> false }) })",
            "true",
        ),
        (
            "bool",
            "(false && (1 / (match true { true -> 2, false -> 3 }) == 0))",
            "false",
        ),
        (
            "bool",
            "(1 / (match true { true -> 2, false -> 3 }) == 0.5)",
            "true",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "pub data Indexed<const Value: {carrier}> {{ value: u8; }}
                 const ENABLED: bool = true; const LIMIT: u8 = 3; {} {}",
                keep("keep", "Indexed", index),
                keep("oracle", "Indexed", expected),
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
    }
}

#[test]
fn match_indices_retain_all_arm_module_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, enabled) in [("first", true), ("second", false)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; const ENABLED: bool = {enabled}; const OTHER: bool = false; {}",
                keep(
                    "keep",
                    "Flag",
                    "(match ENABLED { true -> true, false -> OTHER })"
                ),
            ),
        );
    }
    for imports in ["use first; use second;", "use second; use first;"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} pub data Flag<const Enabled: bool> {{ value: u8; }} {} {}",
                keep("enabled", "Flag", "true"),
                keep("disabled", "Flag", "false"),
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "first::keep", "enabled");
        assert_same_machine_types(&checked, "second::keep", "disabled");
        for path in [
            "first::ENABLED",
            "first::OTHER",
            "second::ENABLED",
            "second::OTHER",
        ] {
            let uses = selections(&checked, path, identity(1));
            assert_eq!(
                uses.len(),
                3,
                "each parameter/result/local keeps even skipped {path}"
            );
            for (position, occurrence) in uses.iter().enumerate() {
                assert!(
                    uses[..position]
                        .iter()
                        .all(|prior| prior.source_span() != occurrence.source_span())
                );
            }
        }
    }
}

#[test]
fn independent_match_terms_do_not_enumerate_branch_combinations() {
    let tree = Sources::new();
    let root = tree.package("root");
    let expression = std::iter::repeat_n("(match true { true -> 1, false -> 2 })", 20)
        .collect::<Vec<_>>()
        .join(" + ");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "pub data Indexed<const Value: u64> {{ value: u8; }} {} {}",
            keep("keep", "Indexed", &format!("({expression})")),
            keep("oracle", "Indexed", "20"),
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "oracle");
}

#[test]
fn match_indices_check_unselected_types_coverage_and_actual_landings() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, index, expected) in [
        ("bool", "(match ENABLED { true -> true })", "cover"),
        (
            "bool",
            "(match ENABLED { true -> true, false -> 1u64 })",
            "incompatible",
        ),
        (
            "bool",
            "(match ENABLED { true -> true, 1u64 -> false, _ -> true })",
            "incompatible",
        ),
        ("u8", "(match ENABLED { true -> 1, false -> 300 })", "land"),
        (
            "u8",
            "(match ENABLED { true -> 1, false -> 7 / 2 })",
            "land",
        ),
        ("u8", "(match ENABLED { true -> 1, false -> 2u64 })", "u64"),
        (
            "u8",
            "(match ENABLED { true -> 1u8 / 0, false -> 2 })",
            "Exact integer constant operation",
        ),
        (
            "u8",
            "(match ENABLED { true -> 1, false -> (300 + 1u8) })",
            "land",
        ),
        (
            "u8",
            "(match ENABLED { true -> 1, false -> (7 / 0) })",
            "defined exact numeric",
        ),
        (
            "u8",
            "(match ENABLED { true -> 1, false -> ((match true { true -> 300, false -> 0 }) + 1) })",
            "land",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "pub data Indexed<const Value: {carrier}> {{ value: u8; }}
                 const ENABLED: bool = true; {}",
                keep("keep", "Indexed", index),
            ),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err(
                    "selected success cannot erase static obligations or a demanded failure",
                );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{index}: {diagnostics:?}"
        );
    }
}

#[test]
fn match_indices_do_not_hide_runtime_names_or_authored_operators() {
    let tree = Sources::new();
    let root = tree.package("root");
    for machine in [
        "machine keep(OTHER: bool, value: Flag<(match ENABLED { true -> true, false -> OTHER })>) {}",
        "machine keep(input: bool) { let OTHER: bool = input; let value: Flag<(match ENABLED { true -> true, false -> OTHER })>; }",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "pub data Flag<const Enabled: bool> {{ value: u8; }} const ENABLED: bool = true; const OTHER: bool = false; {machine}"
            ),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("unselected runtime name cannot become a constant");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("original lexical scope")),
            "{diagnostics:?}"
        );
    }
    Sources::write(
        root.join("main.omg"),
        &format!(
            "pub data Flag<const Enabled: bool> {{ value: u8; }} const ENABLED: bool = true;
         operator == u64::equal(left: u64, right: u64) -> bool; {}",
            keep(
                "keep",
                "Flag",
                "(match ENABLED { true -> true, false -> (1u64 == 2u64) })"
            ),
        ),
    );
    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("unselected authored equality cannot acquire builtin meaning");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("requires exact authored selection")),
        "{diagnostics:?}"
    );
}

#[test]
fn unselected_match_arms_still_require_direct_public_package_access() {
    use super::{PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding};

    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::constants; pub machine bridge() -> bool { leaf::constants::OTHER }",
    );
    Sources::write(
        leaf.join("constants.omg"),
        "module constants; pub const OTHER: bool = false;",
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
    let declarations = "pub data Flag<const Enabled: bool> { value: u8; }";
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use middle::bridge; {declarations} {}",
            keep(
                "keep",
                "Flag",
                "(match true { true -> true, false -> constants::OTHER })"
            ),
        ),
    );
    let indirect =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    compile_to_checked_with_packages(&root.join("main.omg"), None, indirect)
        .expect_err("even a skipped value cannot select a loaded transitive dependency");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let direct = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use leaf::constants; {declarations} {} {}",
            keep(
                "keep",
                "Flag",
                "(match true { true -> true, false -> leaf::constants::OTHER })"
            ),
            keep("oracle", "Flag", "true"),
        ),
    );
    let checked = compile(&root, direct.clone());
    assert_same_machine_types(&checked, "keep", "oracle");
    assert_eq!(
        selections(&checked, "constants::OTHER", identity(3)).len(),
        3
    );
    Sources::write(
        leaf.join("constants.omg"),
        "module constants; const OTHER: bool = false;",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, direct)
        .expect_err("a skipped arm cannot gain access to a private dependency constant");
}

#[test]
fn skipped_anonymous_match_comparisons_cannot_hide_undefined_division() {
    let tree = Sources::new();
    let root = tree.package("root");
    for expression in [
        "(false && (1 / (match true { true -> 0, false -> 1 }) == 0))",
        "(false && ((match true { true -> 1, false -> 2 }) / 0 == 0))",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "pub data Flag<const Enabled: bool> {{ value: u8; }} {}",
                keep("keep", "Flag", expression),
            ),
        );
        let Err(diagnostics) =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        else {
            panic!("undefined anonymous arithmetic cannot acquire a comparison value");
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("anonymous")),
            "{diagnostics:?}"
        );
    }
}
