use super::{
    AuthoredDeclarationSelectionExposure, PackageCompilationInputs, PackageDependencyBinding,
    PackageSourceBinding, Sources, assert_same_machine_types, compile,
    compile_to_checked_with_packages, identity, machine_types, root_inputs, selections,
};

const FLAG: &str = "pub data Flag<const Enabled: bool> { value: u8; }";
const LEXICAL_REJECTION: &str =
    "machine index operand must select a constant in its original lexical scope";

fn keep(name: &str, index: &str) -> String {
    format!(
        "machine {name}(value: Flag<{index}>) -> Flag<{index}> {{ let local: Flag<{index}> = value; local }}"
    )
}

#[test]
fn boolean_machine_indices_retain_canonical_values_and_each_occurrence() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        &format!(
            "module settings; const SIZE: bool = false; {}",
            keep("keep", "SIZE")
        ),
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {FLAG} const SIZE: bool = true; {} {} {}",
            keep("keep", "SIZE"),
            keep("enabled", "true"),
            keep("disabled", "false")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "enabled");
    assert_same_machine_types(&checked, "settings::keep", "disabled");
    assert_ne!(
        checked
            .typed
            .normalized_type_identity(machine_types(&checked, "enabled")[0]),
        checked
            .typed
            .normalized_type_identity(machine_types(&checked, "disabled")[0])
    );
    for path in ["SIZE", "settings::SIZE"] {
        let uses = selections(&checked, path, identity(1));
        assert_eq!(uses.len(), 3);
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
fn boolean_runtime_bindings_cannot_be_folded_as_global_indices() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; const SIZE: bool = true;",
    );
    for declarations in [
        format!("{FLAG} const SIZE: bool = true;"),
        format!("use settings::SIZE; {FLAG}"),
    ] {
        for (literal, shadowed) in [
            (
                "machine keep(SIZE: bool, value: Flag<true>) -> Flag<true> { value }",
                "machine keep(SIZE: bool, value: Flag<SIZE>) -> Flag<SIZE> { value }",
            ),
            (
                "machine keep(input: bool, value: Flag<true>) -> Flag<true> { let SIZE: bool = input; let local: Flag<true> = value; local }",
                "machine keep(input: bool, value: Flag<true>) -> Flag<true> { let SIZE: bool = input; let local: Flag<SIZE> = value; local }",
            ),
        ] {
            Sources::write(root.join("main.omg"), &format!("{declarations} {literal}"));
            compile(&root, root_inputs(&root));
            Sources::write(root.join("main.omg"), &format!("{declarations} {shadowed}"));
            let diagnostics =
                compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                    .expect_err("runtime Boolean is not the same-named static value");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(LEXICAL_REJECTION)),
                "{diagnostics:?}"
            );
        }
    }
    Sources::write(
        root.join("main.omg"),
        &format!("use settings; {FLAG} {}", keep("keep", "settings::SIZE")),
    );
    compile(&root, root_inputs(&root));
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {FLAG} machine keep(settings: bool, value: Flag<true>) -> Flag<true> {{ value }}"
        ),
    );
    compile(&root, root_inputs(&root));
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {FLAG} machine keep(settings: bool, value: Flag<settings::SIZE>) -> Flag<settings::SIZE> {{ value }}"
        ),
    );
    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("qualified Boolean argument preserves its lexical root");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(LEXICAL_REJECTION)),
        "{diagnostics:?}"
    );
}

#[test]
fn later_boolean_local_does_not_capture_an_earlier_index() {
    let tree = Sources::new();
    let root = tree.package("root");
    let source = format!(
        "{FLAG} const SIZE: bool = true;
         machine keep(input: bool, value: Flag<true>) -> Flag<true> {{ let local: Flag<SIZE> = value; let SIZE: bool = input; local }} {}", keep("oracle", "true")
    );
    Sources::write(root.join("main.omg"), &source);
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "oracle");
    let uses = selections(&checked, "SIZE", identity(1));
    assert_eq!(uses.len(), 1);
    assert_eq!(
        uses[0].source_span().span.start,
        source.find("Flag<SIZE>").unwrap() + "Flag<".len()
    );
}

#[test]
fn boolean_signature_indices_keep_public_exposure_separate_from_body() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; const SIZE: bool = true;",
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {FLAG} pub machine keep(value: Flag<true>) -> Flag<true> {{ let local: Flag<settings::SIZE> = value; local }}"
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    let uses = selections(&checked, "settings::SIZE", identity(1));
    assert_eq!(uses.len(), 1);
    assert_eq!(
        uses[0].exposure(),
        AuthoredDeclarationSelectionExposure::PrivateImplementation
    );
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use settings; {FLAG} pub {}",
            keep("keep", "settings::SIZE")
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
        .expect_err("Boolean canonicalization does not publish a private declaration");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub const SIZE: bool = true;",
    );
    let checked = compile(&root, root_inputs(&root));
    let uses = selections(&checked, "settings::SIZE", identity(1));
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
}

#[test]
fn boolean_indices_require_direct_public_package_authority() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::settings; pub machine bridge() -> bool { leaf::settings::SIZE }",
    );
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; pub const SIZE: bool = true;",
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
            "use middle::bridge; {FLAG} {}",
            keep("keep", "settings::SIZE")
        ),
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, indirect)
        .expect_err("transitive loaded Boolean does not authorize selection");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    let direct = PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap();
    Sources::write(
        root.join("main.omg"),
        &format!(
            "use leaf::settings; {FLAG} {} {}",
            keep("keep", "leaf::settings::SIZE"),
            keep("oracle", "true")
        ),
    );
    let checked = compile(&root, direct.clone());
    assert_same_machine_types(&checked, "keep", "oracle");
    assert_eq!(selections(&checked, "settings::SIZE", identity(3)).len(), 3);
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; const SIZE: bool = true;",
    );
    compile_to_checked_with_packages(&root.join("main.omg"), None, direct)
        .expect_err("direct dependencies do not expose private Boolean constants");
}

#[test]
fn boolean_domain_indices_in_signatures_locals_and_casts_keep_lexical_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    let declarations = "const SIZE: bool = true; const DISABLED: bool = false; domain<T, const Enabled: bool> T::Tagged<Enabled>;";
    let machine = |name: &str, index: &str| {
        format!(
            "machine {name}(value: u64 in Tagged<{index}>) -> u64 in Tagged<{index}> {{ let local: u64 in Tagged<{index}> = (value as u64 in Tagged<{index}>); local }}"
        )
    };
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} {} {} {} {}",
            machine("keep", "SIZE"),
            machine("oracle", "true"),
            machine("disabled", "DISABLED"),
            machine("false_oracle", "false")
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    assert_same_machine_types(&checked, "keep", "oracle");
    assert_same_machine_types(&checked, "disabled", "false_oracle");
    assert_ne!(
        checked
            .typed
            .normalized_type_identity(machine_types(&checked, "keep")[0]),
        checked
            .typed
            .normalized_type_identity(machine_types(&checked, "disabled")[0])
    );
    assert_eq!(selections(&checked, "SIZE", identity(1)).len(), 4);
    assert_eq!(selections(&checked, "DISABLED", identity(1)).len(), 4);
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} machine keep(SIZE: bool, value: u64) -> u64 in Tagged<true> {{ value as u64 in Tagged<true> }}"
        ),
    );
    compile(&root, root_inputs(&root));
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} machine keep(SIZE: bool, value: u64) -> u64 in Tagged<true> {{ value as u64 in Tagged<SIZE> }}"
        ),
    );
    let diagnostics =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
            .expect_err("Boolean cast index cannot capture the global over a runtime parameter");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(LEXICAL_REJECTION)),
        "{diagnostics:?}"
    );
}

#[test]
fn boolean_and_integer_index_carriers_are_not_interchangeable() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, initializer, destination, literal) in
        [("bool", "true", "u64", "1"), ("u64", "1", "bool", "true")]
    {
        let declarations = format!(
            "const SIZE: {carrier} = {initializer}; pub data Flag<const Enabled: {destination}> {{ value: u8; }}"
        );
        Sources::write(
            root.join("main.omg"),
            &format!("{declarations} {}", keep("oracle", literal)),
        );
        compile(&root, root_inputs(&root));
        Sources::write(
            root.join("main.omg"),
            &format!("{declarations} {}", keep("keep", "SIZE")),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("named Boolean and integer indices preserve their declared carriers");
        let expected = format!("landed `{carrier}` result cannot initialize `{destination}`");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(&expected)),
            "{diagnostics:?}"
        );
    }
    let declarations = "domain<T, const N: u64> T::Indexed<N>;";
    Sources::write(
        root.join("main.omg"),
        &format!(
            "{declarations} machine keep(value: u64 in Indexed<1>) -> u64 in Indexed<1> {{ value }}"
        ),
    );
    compile(&root, root_inputs(&root));
    for literal in ["true", "false"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{declarations} machine keep(value: u64 in Indexed<{literal}>) -> u64 in Indexed<{literal}> {{ value }}"
            ),
        );
        let diagnostics =
            compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
                .expect_err("Boolean domain literals cannot satisfy an integer telescope");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("canonical type `bool`, expected `u64`")),
            "{diagnostics:?}"
        );
    }
}
