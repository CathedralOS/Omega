use super::{Sources, compile_to_checked_with_packages, root_inputs};
use build_time_evaluation::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};

#[test]
fn projected_array_values_retain_shape_at_every_destination() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (element, initializer, wrong) in [
        ("[u8; 0]", "[]", "[bool; 0]"),
        ("[u8; 0]", "[]", "[u64; 0]"),
        ("[u8; 0]", "[]", "u8"),
        ("[u8; 2]", "[7, 9]", "[u8; 1]"),
        ("[u8; 2]", "[7, 9]", "[u64; 2]"),
        ("[bool; 2]", "[true, false]", "[u8; 2]"),
        ("[[u8; 2]; 0]", "[]", "[[u8; 3]; 0]"),
        ("[[u8; 0]; 1]", "[[]]", "[[bool; 0]; 1]"),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!(
                "module settings; data Sizes {{}}
             const Sizes::ROWS: [{element}; 1] = [{initializer}];
             const Sizes::CUBES: [[{element}; 1]; 1] = [[{initializer}]];"
            ),
        );
        for projection in ["settings::Sizes::ROWS[0]", "settings::Sizes::CUBES[0][0]"] {
            for destination in [element, wrong] {
                for consumer in [
                    format!("machine read() -> {destination} {{ {projection} }}"),
                    format!("machine read() {{ let value: {destination} = {projection}; }}"),
                    format!(
                        "machine read() {{ let mut value: {destination} = {projection}; value = {projection}; }}"
                    ),
                    format!(
                        "machine take(value: {destination}) {{}} machine read() {{ take({projection}); }}"
                    ),
                    format!(
                        "machine take(value: {destination}) -> u8 {{ 1 }} machine read() -> u8 {{ take({projection}) }}"
                    ),
                    format!(
                        "data Holder {{ value: {destination}; }} machine read() {{ let value: Holder = Holder {{ value: {projection} }}; }}"
                    ),
                    format!("machine read() -> [{destination}; 1] {{ [{projection}] }}"),
                    format!(
                        "machine take(value: [{destination}; 1]) {{}} machine read() {{ take([{projection}]); }}"
                    ),
                ] {
                    Sources::write(root.join("main.omg"), &format!("use settings; {consumer}"));
                    let result = compile_to_checked_with_packages(
                        &root.join("main.omg"),
                        None,
                        root_inputs(&root),
                    );
                    if destination == element {
                        result.expect("exact projected array type checks");
                    } else {
                        assert!(result.is_err(), "{element} -> {destination}: {consumer}");
                        let diagnostics = result.expect_err("rejected destination");
                        assert!(
                            diagnostics.iter().any(|diagnostic| diagnostic
                                .message
                                .contains("constant array projection")),
                            "{element} -> {destination}: {consumer}: {diagnostics:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn projected_arrays_keep_element_identity_when_lent_as_slices() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (length, elements) in [(2, "7, 9"), (0, "")] {
        Sources::write(
            root.join("settings.omg"),
            &format!(
                "module settings; data Sizes {{}} const Sizes::ROWS: [[u8; {length}]; 1] = [[{elements}]];"
            ),
        );
        for element in ["u8", "bool", "u64"] {
            Sources::write(
                root.join("main.omg"),
                &format!(
                    "use settings; machine take(value: &[{element}]) {{}}
             machine read() {{ take(settings::Sizes::ROWS[0]); }}"
                ),
            );
            let result =
                compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root));
            if element == "u8" {
                result.expect("matching shared slice argument retains ordinary lending admission");
            } else {
                assert!(result.is_err(), "lending cannot change the element carrier");
                assert!(
                    result
                        .expect_err("rejected slice")
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("constant array projection"))
                );
            }
        }
    }
}

#[test]
fn array_projection_assignment_cannot_change_an_existing_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; data Sizes {} const Sizes::ROWS: [[u8; 0]; 1] = [[]];",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings; machine read() { let mut value: [bool; 0] = []; value = settings::Sizes::ROWS[0]; }",
    );
    let result = compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root));
    assert!(
        result.is_err(),
        "assignment cannot change a projected array carrier"
    );
    assert!(
        result
            .expect_err("rejected assignment")
            .iter()
            .any(|diagnostic| diagnostic.message.contains("constant array projection"))
    );
}

#[test]
fn scalar_projections_nested_in_value_call_arrays_keep_their_type() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; data Sizes {} const Sizes::VALUES: [u8; 1] = [7];",
    );
    for element in ["u8", "u64", "bool"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; machine take(value: [{element}; 1]) -> u8 {{ 1 }}
             machine read() -> u8 {{ take([settings::Sizes::VALUES[0]]) }}"
            ),
        );
        let result =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root));
        if element == "u8" {
            result.expect("matching scalar projection array checks");
        } else {
            assert!(
                result.is_err(),
                "nesting a typed scalar cannot widen or change its class"
            );
            assert!(
                result
                    .expect_err("rejected scalar element")
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("constant array projection"))
            );
        }
    }
}

#[test]
fn matching_array_projection_values_evaluate_with_exact_module_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, first) in [("first", 7), ("second", 9)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; data Sizes {{}}
             const Sizes::ROWS: [[u8; 2]; 1] = [[{first}, 2]];
             const Sizes::EMPTY: [[u8; 0]; 1] = [[]];"
            ),
        );
    }
    Sources::write(
        root.join("main.omg"),
        "use first; use second;
         machine read() -> [u8; 2] { first::Sizes::ROWS[0] }
         machine other() -> [u8; 2] { second::Sizes::ROWS[0] }
         machine empty() -> [u8; 0] { first::Sizes::EMPTY[0] }",
    );
    let checked =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect("matching projected destinations");
    for (name, expected) in [
        ("read", vec![BuildTimeValue::Int(7), BuildTimeValue::Int(2)]),
        (
            "other",
            vec![BuildTimeValue::Int(9), BuildTimeValue::Int(2)],
        ),
        ("empty", vec![]),
    ] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .expect("projection consumer");
        let result = BuildTimeAdmissionPlan::infer(&checked.typed)
            .evaluate_machine_symbol_for_invocation_measured(
                &checked.typed,
                machine.symbol,
                vec![],
                BuildTimeInvocationCustody::Symbol(machine.symbol),
            )
            .expect("matching array projection evaluates");
        assert_eq!(result.value(), &BuildTimeValue::Array(expected));
    }
}
