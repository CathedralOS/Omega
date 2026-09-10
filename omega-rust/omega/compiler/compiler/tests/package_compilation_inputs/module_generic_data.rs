use super::*;

fn check(body: &str) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let tree = TempTree::new();
    let root = tree.package("root");
    for module in ["first", "second"] {
        TempTree::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; pub data Envelope<T> {{ value: T; }} pub data Point {{ value: u64; }}"
            ),
        );
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
    .expect("root package");
    compile_to_checked_with_packages(&root.join("main.omg"), None, inputs)
}

#[test]
fn qualified_record_templates_keep_distinct_module_owners() {
    let checked = check("machine keep(value: first::Envelope<u64>) -> first::Envelope<u64> { value } machine other(value: second::Envelope<u64>) -> second::Envelope<u64> { value }")
        .expect("each qualified template retains its own declaration");
    let owners = checked
        .data_definitions
        .iter()
        .map(|(_, data)| data)
        .filter_map(|data| {
            let origin = data.generic_instance?;
            let typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. } =
                checked.tables.type_reference_table.type_reference(origin)
            else {
                panic!("closed application");
            };
            Some(checked.symbols.display_path(*base_symbol, "::"))
        })
        .collect::<Vec<_>>();
    assert_eq!(owners.len(), 2, "{owners:?}");
    assert!(owners.iter().any(|owner| owner == "first::Envelope"));
    assert!(owners.iter().any(|owner| owner == "second::Envelope"));
    check("machine wrong(value: first::Envelope<u64>) -> second::Envelope<u64> { value }")
        .expect_err("same layout does not identify distinct generic declarations");
}

#[test]
fn nominal_arguments_and_repeated_import_spellings_keep_exact_identity() {
    let checked = check("use first::Envelope; use first::Point; machine keep(value: Envelope<Point>) -> first::Envelope<first::Point> { value } machine other(value: first::Envelope<second::Point>) -> first::Envelope<second::Point> { value }")
        .expect("one application has equal qualified and imported spellings");
    let instances = checked
        .data_definitions
        .iter()
        .map(|(_, data)| data)
        .filter(|data| data.generic_instance.is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        instances.len(),
        2,
        "equal imported/qualified arguments deduplicate; distinct nominal arguments do not"
    );
    assert_ne!(instances[0].symbol, instances[1].symbol);
    let nested = check("machine keep(value: first::Envelope<[first::Point; 2]>) -> first::Envelope<[first::Point; 2]> { value } machine other(value: first::Envelope<[second::Point; 2]>) -> first::Envelope<[second::Point; 2]> { value }")
        .expect("nested arrays retain exact nominal element identity");
    assert_eq!(
        nested
            .data_definitions
            .iter()
            .filter(|(_, data)| data.generic_instance.is_some())
            .count(),
        2
    );
    check("machine wrong(value: first::Envelope<[first::Point; 2]>) -> first::Envelope<[second::Point; 2]> { value }")
        .expect_err("array geometry cannot erase nested nominal identity");
    check("machine wrong(value: first::Envelope<first::Point>) -> first::Envelope<second::Point> { value }")
        .expect_err("same-leaf nominal arguments remain distinct");
}

#[test]
fn package_aliases_preserve_direct_dependency_and_private_template_gates() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(middle.join("main.omg"), "data Middle {}");
    TempTree::write(
        root.join("main.omg"),
        "use leaf::first::Envelope; machine keep(value: Envelope<u64>) -> leaf::first::Envelope<u64> { value }",
    );
    for (direct, public) in [(true, true), (true, false), (false, true)] {
        TempTree::write(
            leaf.join("first.omg"),
            &format!(
                "module first; {}data Envelope<T> {{ value: T; }} machine local(value: Envelope<u64>) -> Envelope<u64> {{ value }}",
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
        .expect("acyclic package inputs");
        let result = compile_to_checked_with_packages(&root.join("main.omg"), None, inputs);
        if direct && public {
            let checked = result
                .expect("direct public template admits both qualified and narrow import uses");
            let instances = checked
                .data_definitions
                .iter()
                .map(|(_, data)| data)
                .filter(|data| data.generic_instance.is_some())
                .collect::<Vec<_>>();
            assert_eq!(
                instances.len(),
                1,
                "both spellings share the exact application"
            );
            assert_eq!(
                checked.symbols.symbol_package_identity(instances[0].symbol),
                Some(identity(3))
            );
        } else {
            let diagnostics =
                result.expect_err("qualification cannot create visibility or dependency authority");
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
fn root_attached_machine_does_not_belong_to_same_leaf_module_template() {
    let checked = check("data Envelope { root: u64; } machine Envelope::read(&self) -> u64 { self.root } machine keep(value: first::Envelope<u64>) -> first::Envelope<u64> { value }")
        .expect("root method retains its nongeneric root carrier");
    let methods = checked
        .machines
        .iter()
        .map(|(_, machine)| machine)
        .filter(|machine| checked.symbols.name(machine.symbol).ends_with("read"))
        .collect::<Vec<_>>();
    assert_eq!(
        methods.len(),
        1,
        "the root method must not be cloned onto module instances"
    );
    assert_eq!(
        checked
            .symbols
            .display_path(methods[0].attached_data_symbol, "::"),
        "Envelope"
    );
}
