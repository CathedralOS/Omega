use super::{
    BUFFER, Sources, assert_same_machine_types, compile, compile_to_checked_with_packages,
    identity, keep, root_inputs, selections,
};

#[test]
fn module_scoped_scalars_keep_distinct_body_values_and_machine_indices() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, value) in [("first", 2), ("second", 3)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; pub data Limits {{}} pub const Limits::MAX: u64 = {value};
                 machine direct() -> u64 {{ Limits::MAX }} {}",
                keep("keep", "Limits::MAX + 1")
            ),
        );
    }
    for imports in ["use first; use second;", "use second; use first;"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} use first::Limits::MAX; {BUFFER}
                 data Limits {{}} const Limits::MAX: u64 = 1;
                 machine direct() -> u64 {{ Limits::MAX }}
                 machine shadow() -> u64 {{ let MAX: u64 = 7; MAX }}
                 {} {} {} {} {} {} {}",
                keep("root_use", "Limits::MAX + 1"),
                keep("first_use", "first::Limits::MAX + 1"),
                keep("second_use", "second::Limits::MAX + 1"),
                keep("imported", "MAX + 1"),
                keep("two", "2"),
                keep("three", "3"),
                keep("four", "4"),
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        for (machine, oracle) in [
            ("root_use", "two"),
            ("first::keep", "three"),
            ("second::keep", "four"),
            ("first_use", "three"),
            ("second_use", "four"),
            ("imported", "three"),
        ] {
            assert_same_machine_types(&checked, machine, oracle);
        }
        for (path, expected) in [
            ("direct", 1),
            ("first::direct", 2),
            ("second::direct", 3),
            ("shadow", 7),
        ] {
            let machine = checked
                .typed
                .machines()
                .iter()
                .find(|machine| checked.symbols.display_path(machine.symbol, "::") == path)
                .expect("body constant consumer");
            let value = build_time_evaluation::BuildTimeAdmissionPlan::infer(&checked.typed)
                .evaluate_machine_symbol_for_invocation_measured(
                    &checked.typed,
                    machine.symbol,
                    vec![],
                    build_time_evaluation::BuildTimeInvocationCustody::Symbol(machine.symbol),
                )
                .expect("constant body evaluates");
            assert_eq!(
                value.value(),
                &build_time_evaluation::BuildTimeValue::Int(expected),
                "{path}"
            );
        }
        for carrier in ["first::Limits", "second::Limits"] {
            assert_eq!(
                selections(&checked, carrier, identity(1)).len(),
                1,
                "scoped declaration retains its exact carrier"
            );
        }
    }
}

#[test]
fn module_scoped_scalar_initializers_are_checked_even_when_private_and_unused() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("main.omg"), "use settings;");
    for (carrier, initializer) in [
        ("u8", "300"),
        ("bool", "1"),
        ("u64", "true"),
        ("u8", "1u64"),
        ("f32", "true"),
        ("f64", "\"text\""),
        ("f32", "1.25f64"),
        ("f64", "1u64"),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!(
                "module settings; data Limits {{}} const Limits::BAD: {carrier} = {initializer};"
            ),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("unused private constants still owe declared type conformance");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("module scalar constant")),
            "{carrier} = {initializer}: {diagnostics:?}"
        );
    }
}

#[test]
fn module_scoped_scalars_preserve_boolean_and_floating_landings() {
    use build_time_evaluation::{
        BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue,
    };

    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("main.omg"), "use settings;");
    for (carrier, initializer, expected) in [
        ("bool", "true", BuildTimeValue::Bool(true)),
        ("i8", "-128", BuildTimeValue::Int(-128)),
        // The evaluator's integer slot carries the unsigned 64-bit pattern.
        ("u64", "18446744073709551615u64", BuildTimeValue::Int(-1)),
        ("f32", "0.1", BuildTimeValue::Float(f64::from(0.1_f32))),
        (
            "f32",
            "8388609.499999999999999",
            BuildTimeValue::Float(8388609.0),
        ),
        ("f32", "1.0e100", BuildTimeValue::Float(f64::INFINITY)),
        ("f32", "1.25f32", BuildTimeValue::Float(1.25)),
        ("f64", "1.25f64", BuildTimeValue::Float(1.25)),
        ("f32", "16777217", BuildTimeValue::Float(16777216.0)),
        ("f64", "16777217", BuildTimeValue::Float(16777217.0)),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!(
                "module settings; data Limits {{}} const Limits::VALUE: {carrier} = {initializer};
             machine selected() -> {carrier} {{ let value: {carrier} = Limits::VALUE; value }}"
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| {
                checked.symbols.display_path(machine.symbol, "::") == "settings::selected"
            })
            .expect("module consumer");
        let result = BuildTimeAdmissionPlan::infer(&checked.typed)
            .evaluate_machine_symbol_for_invocation_measured(
                &checked.typed,
                machine.symbol,
                vec![],
                BuildTimeInvocationCustody::Symbol(machine.symbol),
            )
            .expect("declared scalar value evaluates");
        assert_eq!(result.value(), &expected, "{carrier} = {initializer}");
    }
}

#[test]
fn module_scoped_scalars_require_valid_attachment_and_visibility() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("main.omg"), "use settings; data Limits {}");
    for (declarations, diagnostic) in [
        (
            "const Missing::MAX: u64 = 1;",
            "exact nongeneric data carrier",
        ),
        (
            "const Limits::MAX: u64 = 1;",
            "exact nongeneric data carrier",
        ),
        (
            "data Limits<T> {} const Limits::MAX: u64 = 1;",
            "module-owned generic data",
        ),
        (
            "data Limits {} pub const Limits::MAX: u64 = 1;",
            "public interface selects private data",
        ),
        (
            "data Limits { case MAX; } const Limits::MAX: u64 = 1;",
            "collides with the case",
        ),
        (
            "data Limits {} const Limits::MAX: u64 = 1; const Limits::MAX: u64 = 1;",
            "duplicate const",
        ),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!("module settings; {declarations}"),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("unused scoped constants still owe attachment validity");
        assert!(
            diagnostics
                .iter()
                .any(|actual| actual.message.contains(diagnostic)),
            "{declarations}: {diagnostics:?}"
        );
    }
}

#[test]
fn module_scoped_scalar_indices_reject_ambiguous_imports_and_runtime_roots() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["first", "second"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; data Limits {{}} const Limits::MAX: u64 = 2;"),
        );
    }
    for imports in ["use first::Limits::MAX;", "use second::Limits::MAX;"] {
        Sources::write(
            root.join("main.omg"),
            &format!("{imports} {BUFFER} {}", keep("keep", "MAX + 1")),
        );
        compile(&root, root_inputs(&root));
    }
    for imports in [
        "use first::Limits::MAX; use second::Limits::MAX;",
        "use second::Limits::MAX; use first::Limits::MAX;",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{imports} {BUFFER} {}", keep("keep", "MAX + 1")),
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("equal scalar values cannot resolve competing declarations");
    }
    for machine in [
        "machine keep(first: u64, value: Buffer<first::Limits::MAX + 1>) {}",
        "machine keep(first: u64, value: Buffer<3>) -> Buffer<first::Limits::MAX + 1> { value }",
        "machine keep(value: Buffer<3>) { let first: u64 = 0; let local: Buffer<first::Limits::MAX + 1> = value; }",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("use first; {BUFFER} {machine}"),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("runtime qualifier cannot acquire a scoped constant identity");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("original lexical scope")),
            "{machine}: {diagnostics:?}"
        );
    }
}

#[test]
fn scoped_string_spelling_does_not_admit_a_nominal_initializer() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("main.omg"), "use settings;");
    Sources::write(
        root.join("settings.omg"),
        "module settings; data string {} data Limits {} const Limits::TEXT: string = \"text\";",
    );
    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("a nominal string spelling does not make text a valid scalar initializer");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("module-owned nominal")),
        "{diagnostics:?}"
    );
}
