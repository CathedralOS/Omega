use super::{
    Sources, assert_same_machine_types, compile, compile_to_checked_with_packages, identity,
    machine_types, root_inputs, selections,
};

fn declarations(array_type: &str, value: &str) -> String {
    format!(
        "data Sizes {{}} data Oracle {{}}
         const Sizes::SIZE: {array_type} = {value};
         const Oracle::SAME: {array_type} = {value};
         data Indexed<const Selected: {array_type}> {{ value: u8; }}"
    )
}

fn keep(name: &str, index: &str) -> String {
    format!(
        "machine {name}(value: Indexed<{index}>) -> Indexed<{index}> {{
            let local: Indexed<{index}> = value;
            let Sizes: u64 = 0;
            local
         }}"
    )
}

#[test]
fn array_indices_preserve_static_identity_and_each_lexical_occurrence() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (array_type, value) in [
        ("[u64; 2]", "[1, 2]"),
        ("[[bool; 2]; 2]", "[[true, false], [false, true]]"),
        ("[u8; 0]", "[]"),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!("module settings; {}", keep("keep", "Sizes::SIZE")),
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; {} {} {}",
                declarations(array_type, value),
                keep("keep", "Sizes::SIZE"),
                keep("oracle", "Oracle::SAME")
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
        assert_same_machine_types(&checked, "settings::keep", "oracle");
        let uses = selections(&checked, "Sizes::SIZE", identity(1));
        assert_eq!(uses.len(), 6, "root and module parameter, result and local");
        for (position, selection) in uses.iter().enumerate() {
            assert!(
                uses[..position]
                    .iter()
                    .all(|prior| prior.source_span() != selection.source_span())
            );
        }
    }
}

#[test]
fn array_indices_reject_runtime_roots_in_root_and_module_machine_scopes() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (array_type, value) in [
        ("[u64; 2]", "[1, 2]"),
        ("[[bool; 2]; 2]", "[[true, false], [false, true]]"),
    ] {
        for machine in [
            "machine keep(Sizes: u64, value: Indexed<Sizes::SIZE>) {}",
            "machine keep(Sizes: u64, value: Indexed<Oracle::SAME>) -> Indexed<Sizes::SIZE> { value }",
            "machine keep(value: Indexed<Oracle::SAME>) { let Sizes: u64 = 0; let local: Indexed<Sizes::SIZE> = value; }",
        ] {
            for module in [false, true] {
                for shadowed in [false, true] {
                    let machine = if shadowed {
                        machine.to_owned()
                    } else {
                        machine.replace("Sizes::SIZE", "Oracle::SAME")
                    };
                    Sources::write(
                        root.join("settings.omg"),
                        &format!("module settings; {}", if module { &machine } else { "" }),
                    );
                    Sources::write(
                        root.join("main.omg"),
                        &format!(
                            "use settings; {} {}",
                            declarations(array_type, value),
                            if module { "" } else { &machine }
                        ),
                    );
                    let result = compile_to_checked_with_packages(
                        &root.join("main.omg"),
                        None,
                        root_inputs(&root),
                    );
                    if shadowed {
                        let diagnostics = result.expect_err("a runtime root is not a constant");
                        assert!(
                            diagnostics.iter().any(|diagnostic| diagnostic.message.contains(
                                "machine index operand must select a constant in its original lexical scope"
                            )),
                            "{machine}: {diagnostics:?}"
                        );
                    } else {
                        result.expect("the same owner can select an unshadowed static array");
                    }
                }
            }
        }
    }
}

#[test]
fn different_array_values_keep_distinct_canonical_instances() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{} const Oracle::DIFFERENT: [u64; 2] = [2, 1]; {} {}",
            declarations("[u64; 2]", "[1, 2]"),
            keep("first", "Sizes::SIZE"),
            keep("second", "Oracle::DIFFERENT")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_ne!(
        checked
            .typed
            .normalized_type_identity(machine_types(&checked, "first")[0]),
        checked
            .typed
            .normalized_type_identity(machine_types(&checked, "second")[0])
    );
}
