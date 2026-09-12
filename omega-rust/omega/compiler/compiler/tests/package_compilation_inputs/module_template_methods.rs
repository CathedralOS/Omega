use super::*;

fn check(body: &str) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let tree = TempTree::new();
    let root = tree.package("root");
    for (module, source) in [
        (
            "first",
            include_str!(
                "../../../../../../tests/omega/pass/modules/closed_template_methods/first.omg"
            ),
        ),
        (
            "second",
            include_str!(
                "../../../../../../tests/omega/pass/modules/closed_template_methods/second.omg"
            ),
        ),
    ] {
        TempTree::write(root.join(format!("{module}.omg")), source);
    }
    TempTree::write(
        root.join("main.omg"),
        &format!("use first; use second; {body}"),
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .unwrap();
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
}

#[test]
fn closed_module_methods_rejoin_exact_carrier_instances() {
    let checked = check("machine read_first(value: &first::Envelope<u64>) -> u64 { value.read() } machine read_second(value: &second::Envelope<u64>) -> u64 { value.read() } machine boolean(value: &first::Envelope<bool>) -> bool { value.read() } machine again(value: &first::Envelope<u64>) -> u64 { value.read() }").expect("closed module methods");
    let mut owners = Vec::new();
    for (_, machine) in checked.machines.iter() {
        let Some((_, owner)) = checked.data_definitions.iter().find(|(_, data)| {
            data.symbol == machine.attached_data_symbol && data.generic_instance.is_some()
        }) else {
            continue;
        };
        if !checked.symbols.name(machine.symbol).ends_with("read") {
            continue;
        }
        let typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } = checked
            .tables
            .type_reference_table
            .type_reference(owner.generic_instance.unwrap())
        else {
            panic!("closed carrier origin");
        };
        for state in checked.machine_states(machine) {
            assert!(
                state.symbol.is_valid(),
                "closed method retains its actual state"
            );
            assert_eq!(checked.symbols.get(state.symbol).parent, machine.symbol);
        }
        owners.push(checked.symbols.display_path(*base_symbol, "::"));
    }
    owners.sort();
    assert_eq!(
        owners,
        ["first::Envelope", "first::Envelope", "second::Envelope"]
    );
    assert!(
        check("machine wrong(value: &first::Envelope<u64>) -> bool { value.read() }").is_err(),
        "selected method result retains its argument type",
    );
}

#[test]
fn borrowed_template_method_cannot_copy_an_affine_payload() {
    let tree = TempTree::new();
    let root = tree.package("root");
    TempTree::write(
        root.join("first.omg"),
        "module first; pub data Envelope<T> { value: T; } pub machine Envelope::read<T>(&self) -> T { self.value } pub data Resource { value: u64; }",
    );
    TempTree::write(
        root.join("main.omg"),
        "use first; machine read(value: &first::Envelope<first::Resource>) -> first::Resource { value.read() }",
    );
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .unwrap();
    let diagnostics = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
        .expect_err("borrowed affine payload cannot become owned");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("borrow")
                || diagnostic.message.contains("copy")),
        "{diagnostics:?}"
    );
}

#[test]
fn instantiated_methods_keep_each_package_use_authority() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(middle.join("main.omg"), "data Middle {}");
    TempTree::write(
        root.join("main.omg"),
        "use leaf::first::Envelope; data Envelope { root: bool; } machine Envelope::read(&self) -> bool { self.root } machine read(value: &leaf::first::Envelope<u64>) -> u64 { value.read() }",
    );
    for (direct, public) in [(true, true), (true, false), (false, true)] {
        TempTree::write(
            leaf.join("first.omg"),
            &format!(
                "module first; pub data Envelope<T [copy]> {{ value: T; }} {}machine Envelope::read<T [copy]>(&self) -> T {{ self.value }} machine local(value: &Envelope<u64>) -> u64 {{ value.read() }}",
                if public { "pub " } else { "" }
            ),
        );
        let mut dependencies = vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(2)),
            PackageDependencyBinding::new(identity(2), "leaf", identity(3)),
        ];
        if direct {
            dependencies.push(PackageDependencyBinding::new(
                identity(1),
                "leaf",
                identity(3),
            ));
        }
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "middle", middle.clone()),
                PackageSourceBinding::new(identity(3), "leaf", leaf.clone()),
            ],
            dependencies,
        )
        .unwrap();
        let result = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs);
        if direct && public {
            result.expect("public direct method");
        } else {
            let diagnostics =
                result.expect_err("authorized local instance cannot grant foreign call authority");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains(if direct {
                        "private"
                    } else {
                        "failed to resolve"
                    })),
                "{diagnostics:?}"
            );
        }
    }
}

#[test]
fn methods_authored_outside_the_carrier_module_keep_selected_attachments() {
    for (import, carrier) in [
        ("use first::Envelope;", "Envelope"),
        ("use first;", "first::Envelope"),
    ] {
        let tree = TempTree::new();
        let root = tree.package("root");
        TempTree::write(
            root.join("first.omg"),
            "module first; pub data Envelope<T [copy]> { value: T; }",
        );
        TempTree::write(
            root.join("second.omg"),
            &format!(
                "module second; {import} pub machine {carrier}::read<T [copy]>(&self) -> T {{ self.value }}"
            ),
        );
        TempTree::write(
            root.join("main.omg"),
            "use first; use second; machine read(value: &first::Envelope<u64>) -> u64 { value.read() }",
        );
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .unwrap();
        compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
            .unwrap_or_else(|diagnostics| panic!("{carrier}: {diagnostics:?}"));
    }
}
