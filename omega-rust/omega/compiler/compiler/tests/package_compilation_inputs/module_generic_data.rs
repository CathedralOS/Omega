use super::*;
use compiler::CheckedCompileRequest;

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
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
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
        let result = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
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

#[test]
fn qualified_sum_construction_keeps_exact_application_and_case_owners() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/omega/pass/modules/closed_sum_constructors");
    for file in ["main.omg", "first.omg", "second.omg"] {
        TempTree::write(
            root.join(file),
            &std::fs::read_to_string(fixture.join(file)).unwrap(),
        );
    }
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
        Vec::new(),
    )
    .unwrap();
    let checked = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect("qualified constructors and forwarding retain exact closed sums");
    let instances = checked
        .data_definitions
        .iter()
        .filter(|(_, data)| data.generic_instance.is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        instances.len(),
        5,
        "three first Choice tuples, second Choice and mixed Packet"
    );
    let source = std::fs::read_to_string(root.join("main.omg")).unwrap();
    let mut primitive_instances = Vec::new();
    for (_, data) in instances {
        let origin = data.generic_instance.unwrap();
        let typed_trees::types::TypeReferenceNode::Generic {
            base_symbol,
            arguments,
            ..
        } = checked.tables.type_reference_table.type_reference(origin)
        else {
            panic!("retained generic origin");
        };
        assert!(matches!(
            checked.symbols.display_path(*base_symbol, "::").as_str(),
            "first::Choice" | "second::Choice" | "first::Packet"
        ));
        let [argument] = checked
            .tables
            .type_reference_table
            .type_reference_handles(*arguments)
        else {
            panic!("one argument");
        };
        if let typed_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
            .tables
            .type_reference_table
            .type_reference(*argument)
            && checked.symbols.get(*symbol).kind == symbols::SymbolKind::BuiltinType
        {
            primitive_instances.push((
                checked.symbols.display_path(*base_symbol, "::"),
                checked.symbols.name(*symbol).to_owned(),
                data.symbol,
            ));
        }
    }
    for (owner, argument, expression, case, fields) in [
        (
            "first::Choice",
            "u64",
            "first::Choice::Full { value: 7 }",
            "Full",
            &["value"][..],
        ),
        (
            "first::Choice",
            "bool",
            "first::Choice::Full { value: true }",
            "Full",
            &["value"][..],
        ),
        (
            "second::Choice",
            "u64",
            "second::Choice::Empty",
            "Empty",
            &[][..],
        ),
        (
            "second::Choice",
            "u64",
            "second::Choice::Full { value: 9 }",
            "Full",
            &["value"][..],
        ),
        (
            "second::Choice",
            "u64",
            "second::Choice::Full { value: 13 }",
            "Full",
            &["value"][..],
        ),
        (
            "first::Packet",
            "u64",
            "first::Packet::Full { value: 11, stamp: 2 }",
            "Full",
            &["value", "stamp"][..],
        ),
    ] {
        let (_, _, carrier) = primitive_instances
            .iter()
            .find(|(base, value, _)| base == owner && value == argument)
            .unwrap();
        let selected_case = checked
            .symbols
            .find_child_by_name(*carrier, case)
            .expect("exact materialized case");
        let mut targets = vec![selected_case];
        for field in fields {
            targets.push(
                checked
                    .symbols
                    .find_child_by_name(selected_case, field)
                    .or_else(|| checked.symbols.find_child_by_name(*carrier, field))
                    .expect("exact materialized payload/common field"),
            );
        }
        let start = source.find(expression).unwrap();
        let end = start + expression.len();
        for target in targets {
            assert!(checked.authored_declaration_selections().iter().any(|selection| {
                let language_semantics::declaration_selection::AuthoredDeclarationSelectionTarget::Resolved(resolved) = selection.target() else { return false; };
                let span = selection.source_span();
                resolved.selected_symbol() == target && (start..end).contains(&span.span.start)
                    && checked.symbols.source_file(span).is_some_and(|file| file.path == root.join("main.omg").canonicalize().unwrap())
            }), "{expression} lost exact selection {}", checked.symbols.display_path(target, "::"));
        }
    }
}

fn check_sum(body: &str) -> Result<compiler::CheckedCompilation, Vec<diagnostics::Diagnostic>> {
    let tree = TempTree::new();
    let root = tree.package("root");
    for module in ["first", "second"] {
        TempTree::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; pub data Choice<T> {{ case Empty; case Full(value: T); }} pub data Leaf {{ value: u64; }}"
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
    .unwrap();
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(inputs),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
}

#[test]
fn match_constructor_results_keep_exact_expected_applications() {
    check_sum(
        r#"
        machine keep(value: first::Choice<u64>) -> first::Choice<u64> { value }
        machine make(selected: bool) -> first::Choice<u64> {
            keep(match selected {
                true -> first::Choice::Full { value: 7 },
                false -> first::Choice::Empty
            })
        }
        machine nested(selected: bool) -> first::Choice<second::Choice<bool>> {
            let result: first::Choice<second::Choice<bool>> = match selected {
                true -> first::Choice::Full {
                    value: match selected {
                        true -> second::Choice::Full { value: true },
                        false -> second::Choice::Empty
                    }
                },
                false -> first::Choice::Empty
            };
            result
        }
    "#,
    )
    .expect("return, call and nested field destinations retain distinct tuples");
}

#[test]
fn qualified_sums_reject_wrong_carriers_cases_and_payloads() {
    for body in [
        "machine wrong() -> first::Choice<u64> { second::Choice::Full { value: 7 } }",
        "machine wrong() -> first::Choice<u64> { second::Choice::Empty }",
        "machine wrong() -> first::Choice<u64> { first::Choice::Missing }",
        "machine wrong() -> first::Choice<u64> { first::Choice::Full { value: true } }",
        "machine wrong(value: first::Choice<u64>) -> second::Choice<u64> { value }",
        "machine wrong(value: first::Choice<bool>) -> first::Choice<u64> { value }",
        "machine wrong() -> first::Choice<first::Leaf> { first::Choice::Full { value: second::Leaf { value: 3 } } }",
        "use first::Choice; use second::Choice; machine wrong() -> first::Choice<u64> { Choice::Empty }",
        "machine wrong() -> first::Choice<u64> { match true { true -> first::Choice::Empty, false -> second::Choice::Empty } }",
        "machine wrong() -> first::Choice<u64> { match true { true -> first::Choice::Empty, false -> first::Choice::Full { value: true } } }",
        "machine wrong(Choice: u64) -> first::Choice<u64> { match true { true -> first::Choice::Empty, false -> Choice::Empty } }",
    ] {
        assert!(check_sum(body).is_err(), "accepted: {body}");
    }
}

#[test]
fn sum_payload_context_rejoins_nested_applications_and_imported_owners() {
    let checked = check_sum("use first::Choice; use first::Leaf; machine make() -> Choice<Leaf> { Choice::Full { value: first::Leaf { value: 3 } } } machine keep(value: Choice<Leaf>) -> first::Choice<first::Leaf> { value } machine other() -> first::Choice<second::Leaf> { first::Choice::Full { value: second::Leaf { value: 5 } } }")
        .expect("exact nominal arguments retain distinct sum payload owners");
    assert_eq!(
        checked
            .data_definitions
            .iter()
            .filter(|(_, data)| data.generic_instance.is_some())
            .count(),
        2
    );
}

#[test]
fn shared_sum_instances_preserve_package_authority_at_each_constructor() {
    let tree = TempTree::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    TempTree::write(middle.join("main.omg"), "data Middle {}");
    TempTree::write(
        root.join("main.omg"),
        "use leaf::first::Choice; machine make() -> Choice<u64> { leaf::first::Choice::Full { value: 7 } }",
    );
    for (direct, public) in [(true, true), (true, false), (false, true)] {
        TempTree::write(
            leaf.join("first.omg"),
            &format!(
                "module first; {}data Choice<T> {{ case Empty; case Full(value: T); }} machine local() -> Choice<u64> {{ Choice::Empty }}",
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
        let result = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        if direct && public {
            result.expect("one exact public sum shared by authorized constructors");
        } else {
            let diagnostics = result.expect_err("shared instance cannot grant package authority");
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
fn sum_case_names_preserve_each_lexical_value_prefix() {
    let accepted = [
        "machine wrong(first: u32) -> first::Choice<u64> { first::Choice::Empty }",
        "use first::Choice; machine wrong(Choice: u32) -> first::Choice<u64> { Choice::Empty }",
        "data Value { Empty: u32; } use first::Choice; machine wrong(Choice: Value) -> first::Choice<u64> { Choice::Empty }",
        "data Value { case Empty; } use first::Choice; machine wrong(Choice: Value) -> first::Choice<u64> { Choice::Empty }",
        "machine wrong() -> first::Choice<u64> { let first: u32 = 0; first::Choice::Empty }",
        "use first::Choice; machine wrong() -> first::Choice<u64> { let Choice: u32 = 0; Choice::Empty }",
        "data Wrap { value: first::Choice<u64>; } machine wrong(first: u32) -> Wrap { Wrap { value: first::Choice::Empty } }",
        "machine keep(value: first::Choice<u64>) -> first::Choice<u64> { value } machine wrong(first: u32) -> first::Choice<u64> { keep(first::Choice::Empty) }",
        "data Holder { value: first::Choice<u64>; } machine wrong(first: u32) { let value = first::Choice::Empty; }",
    ].into_iter().filter(|body| check_sum(body).is_ok()).collect::<Vec<_>>();
    assert!(
        accepted.is_empty(),
        "lexically captured cases accepted: {accepted:#?}"
    );
}

#[test]
fn sum_case_name_initializers_precede_their_own_and_later_local_bindings() {
    for body in [
        "use first::Choice; machine keep() -> first::Choice<u64> { let Choice: first::Choice<u64> = Choice::Empty; Choice }",
        "machine keep() -> first::Choice<u64> { let first: first::Choice<u64> = first::Choice::Empty; first }",
        "machine keep() -> first::Choice<u64> { let value: first::Choice<u64> = first::Choice::Empty; let first: u32 = 0; value }",
        "use first::Choice; machine keep() -> first::Choice<u64> { let value: first::Choice<u64> = Choice::Empty; let Choice: u32 = 0; value }",
    ] {
        check_sum(body).unwrap_or_else(|diagnostics| panic!("{body}: {diagnostics:?}"));
    }
}

#[test]
fn sum_case_names_preserve_unique_and_ambiguous_constant_prefixes() {
    let tree = TempTree::new();
    let root = tree.package("root");
    for module in ["first", "second"] {
        TempTree::write(
            root.join(format!("{module}.omg")),
            &format!("module {module}; pub const Choice: u32 = 1;"),
        );
    }
    let mut accepted = Vec::new();
    for imports in [
        "use first::Choice;",
        "use first::Choice; use second::Choice;",
    ] {
        TempTree::write(
            root.join("main.omg"),
            &format!(
                "{imports} data Choice<T> {{ case Empty; }} machine wrong() -> Choice<u64> {{ Choice::Empty }}"
            ),
        );
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", root.clone())],
            Vec::new(),
        )
        .unwrap();
        if compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .is_ok()
        {
            accepted.push(imports);
        }
    }
    assert!(
        accepted.is_empty(),
        "constant prefixes reinterpreted as generic constructors: {accepted:?}"
    );
}
