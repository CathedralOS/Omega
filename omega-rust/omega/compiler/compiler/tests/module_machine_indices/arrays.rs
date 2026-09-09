use super::{
    AuthoredDeclarationSelectionExposure, PackageCompilationInputs, PackageDependencyBinding,
    PackageSourceBinding, Sources, assert_same_machine_types, compile,
    compile_to_checked_with_packages, identity, machine_types, root_inputs, selections,
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

#[test]
fn module_arrays_retain_exact_owners_and_canonical_values() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (array_type, first, second) in [
        ("[u8; 2]", "[1, 2]", "[2, 1]"),
        (
            "[[bool; 2]; 2]",
            "[[true, false], [false, true]]",
            "[[false, true], [true, false]]",
        ),
        ("[i64; 2]", "[-2, -1]", "[-1, -2]"),
        ("[u64; 0]", "[]", "[]"),
    ] {
        for (module, value) in [("first", first), ("second", second)] {
            Sources::write(
                root.join(format!("{module}.omg")),
                &format!(
                    "module {module}; const SIZE: {array_type} = {value}; {}",
                    keep("keep", "SIZE")
                ),
            );
        }
        for imports in ["use first; use second;", "use second; use first;"] {
            Sources::write(
                root.join("main.omg"),
                &format!(
                    "{imports} {} const SIZE: {array_type} = {first}; const OTHER: {array_type} = {second}; {} {} {} {} {}",
                    declarations(array_type, first),
                    keep("keep", "SIZE"),
                    keep("first_use", "first::SIZE"),
                    keep("second_use", "second::SIZE"),
                    keep("first_oracle", "Oracle::SAME"),
                    keep("second_oracle", "OTHER")
                ),
            );
            let checked = compile(&root, root_inputs(&root));
            for (machine, oracle) in [
                ("keep", "first_oracle"),
                ("first::keep", "first_oracle"),
                ("second::keep", "second_oracle"),
                ("first_use", "first_oracle"),
                ("second_use", "second_oracle"),
            ] {
                assert_same_machine_types(&checked, machine, oracle);
            }
            for constant in ["first::SIZE", "second::SIZE"] {
                let uses = selections(&checked, constant, identity(1));
                assert_eq!(uses.len(), 6);
                for (position, selection) in uses.iter().enumerate() {
                    assert!(
                        uses[..position]
                            .iter()
                            .all(|prior| prior.source_span() != selection.source_span())
                    );
                }
            }
        }
    }
}

#[test]
fn module_array_imports_preserve_ambiguity_and_runtime_shadow_rejection() {
    let tree = Sources::new();
    let root = tree.package("root");
    for module in ["first", "second"] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; const SIZE: [u8; 2] = [1, 2];"),
        );
    }
    for imports in [
        "use first::SIZE; use second::SIZE;",
        "use second::SIZE; use first::SIZE;",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} {} {}",
                declarations("[u8; 2]", "[1, 2]"),
                keep("keep", "SIZE")
            ),
        );
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("equal values cannot resolve competing imported declarations");
    }
    for (imports, machine) in [
        (
            "use first::SIZE;",
            "machine keep(SIZE: [u8; 2], value: Indexed<SIZE>) {}",
        ),
        (
            "use first;",
            "machine keep(first: u64, value: Indexed<first::SIZE>) {}",
        ),
        (
            "use first::SIZE;",
            "machine keep(value: Indexed<Oracle::SAME>) { let SIZE: [u8; 2] = [1, 2]; let local: Indexed<SIZE> = value; }",
        ),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{imports} {} {machine}", declarations("[u8; 2]", "[1, 2]")),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("runtime array operands cannot acquire module constant identity");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(
                    "machine index operand must select a constant in its original lexical scope"
                )),
            "{diagnostics:?}"
        );
    }
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use first::SIZE; {} {}",
            declarations("[u8; 2]", "[1, 2]"),
            keep("keep", "SIZE")
        ),
    );
    compile(&root, root_inputs(&root));
}

#[test]
fn module_array_indices_preserve_package_and_public_interface_authority() {
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
        "module settings; pub const SIZE: [u8; 2] = [1, 2];",
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
    let indirect =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use middle::bridge; use leaf::settings; {} pub {}",
            declarations("[u8; 2]", "[1, 2]").replace("data Indexed", "pub data Indexed"),
            keep("keep", "leaf::settings::SIZE")
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, indirect)
        .expect_err("a loaded transitive array constant cannot be selected");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let direct = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    let checked = compile(&root, direct.clone());
    let uses = selections(&checked, "settings::SIZE", identity(3));
    assert_eq!(uses.len(), 3);
    assert_eq!(
        uses.iter()
            .filter(|selection| selection.exposure()
                == AuthoredDeclarationSelectionExposure::PublicInterface)
            .count(),
        2
    );
    assert_eq!(
        uses.iter()
            .filter(|selection| selection.exposure()
                == AuthoredDeclarationSelectionExposure::PrivateImplementation)
            .count(),
        1
    );
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; const SIZE: [u8; 2] = [1, 2];",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, direct)
        .expect_err("direct package reach does not expose a private array constant");
}

#[test]
fn module_array_indices_reject_malformed_values_and_wrong_carriers() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declared, value) in [
        ("[u8; 2]", "[1]"),
        ("[u8; 2]", "[1, 2, 3]"),
        ("[u8; 2]", "[1, 256]"),
        ("[u8; 2]", "[1, true]"),
        ("[u64; 2]", "[1, 2]"),
        ("[u8; 2]", "[1u64, 2]"),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!("module settings; const SIZE: {declared} = {value};"),
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; {} {}",
                declarations("[u8; 2]", "[1, 2]"),
                keep("keep", "settings::SIZE")
            ),
        );
        assert!(
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .is_err(),
            "{declared} = {value}"
        );
    }
}

#[test]
fn module_array_indices_in_concrete_data_fields_match_static_oracles() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; const SIZE: [u8; 2] = [1, 2];",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {} data Holder {{ value: Indexed<settings::SIZE>; }}
         machine read(holder: &Holder) -> Indexed<Oracle::SAME> {{ holder.value }}",
            declarations("[u8; 2]", "[1, 2]")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_eq!(selections(&checked, "settings::SIZE", identity(1)).len(), 1);
}
