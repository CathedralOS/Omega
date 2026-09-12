use super::*;
use build_time_evaluation::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};

#[test]
fn package_qualified_case_values_and_membership_select_the_declaring_owner() {
    for (expression, is_empty) in [
        ("shapes::settings::Choice::Empty", true),
        ("shapes::settings::Choice::Empty {}", true),
        ("shapes::settings::Choice::Some { value: 37 }", false),
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        let shapes = tree.package("shapes");
        Sources::write(
            shapes.join("settings.omg"),
            "module settings; pub data Choice { case Empty; case Some(value: u32); }",
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use shapes::settings;
                 data Choice {{ case Some(value: bool); case Empty; }}
                 data Holder {{ choice: shapes::settings::Choice; }}
                 machine projected(holder: &Holder) -> bool {{
                     (&holder.choice) in shapes::settings::Choice::Empty
                 }}
                 machine borrowed_projection_is_empty() -> bool {{
                     let holder: Holder = Holder {{ choice: {expression} }};
                     projected(&holder)
                 }}
                 machine make() -> shapes::settings::Choice {{ {expression} }}
                 machine matches(value: &shapes::settings::Choice) -> bool {{
                     value in shapes::settings::Choice::Some
                 }}
                 machine is_empty() -> bool {{
                     {expression} in shapes::settings::Choice::Empty
                 }}
                 machine is_some() -> bool {{
                     {expression} in shapes::settings::Choice::Some
                 }}
                 machine local_is_empty() -> bool {{
                     let value: shapes::settings::Choice = {expression};
                     value in shapes::settings::Choice::Empty
                 }}
                 machine borrowed_is_empty() -> bool {{
                     let value: shapes::settings::Choice = {expression};
                     (&value) in shapes::settings::Choice::Empty
                 }}"
            ),
        );
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "shapes", shapes),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "shapes",
                identity(2),
            )],
        )
        .unwrap();
        let checked = compile(&root, inputs);
        let artifact = terminal_production::produce_terminal_artifact(&checked, "matches")
            .expect("package-qualified borrowed membership reaches canonical Terminal");
        let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        assert!(
            module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    terminal_psi::OperationKind::StructuralCaseMembership { .. }
                ))
        );
        assert!(!selections(&checked, "settings::Choice::Empty", identity(2)).is_empty());
        assert!(!selections(&checked, "settings::Choice::Some", identity(2)).is_empty());
        let membership_owners = selections(&checked, "settings::Choice", identity(2));
        assert!(membership_owners.iter().any(|selection| {
            selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::CaseReference
                && selection.source_span().span.end - selection.source_span().span.start
                    == "shapes::settings::Choice".len()
        }), "membership retains the complete authored carrier occurrence");
        for (entry, expected) in [
            ("is_empty", is_empty),
            ("is_some", !is_empty),
            ("local_is_empty", is_empty),
            ("borrowed_is_empty", is_empty),
            ("borrowed_projection_is_empty", is_empty),
        ] {
            let machine = checked
                .typed
                .machines()
                .iter()
                .find(|machine| checked.symbols.display_path(machine.symbol, "::") == entry)
                .expect("membership consumer");
            let result = BuildTimeAdmissionPlan::infer(&checked.typed)
                .evaluate_machine_symbol_for_invocation_measured(
                    &checked.typed,
                    machine.symbol,
                    vec![],
                    BuildTimeInvocationCustody::Symbol(machine.symbol),
                )
                .expect("checked membership evaluates with its selected nominal owner");
            assert_eq!(result.value(), &BuildTimeValue::Bool(expected));
        }
    }
}

#[test]
fn case_selection_requires_the_requesting_packages_direct_dependency() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let shapes = tree.package("shapes");
    Sources::write(
        shapes.join("settings.omg"),
        "module settings; pub data Choice { case Empty; }",
    );
    Sources::write(
        middle.join("bridge.omg"),
        "use shapes::settings; pub machine bridge() -> u32 { 1 }",
    );
    Sources::write(root.join("main.omg"), "use middle::bridge; use shapes::settings;
        machine make() -> shapes::settings::Choice { shapes::settings::Choice::Empty }
        machine member(value: &shapes::settings::Choice) -> bool { value in shapes::settings::Choice::Empty }");
    let sources = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "middle", middle),
        PackageSourceBinding::new(identity(3), "shapes", shapes),
    ];
    let mut dependencies = vec![
        PackageDependencyBinding::new(identity(1), "middle", identity(2)),
        PackageDependencyBinding::new(identity(2), "shapes", identity(3)),
    ];
    let inputs =
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap();
    assert!(
        compile_to_checked_with_packages(&root.join("main.omg"), None, inputs).is_err(),
        "loading a transitive source cannot authorize case selection"
    );
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "shapes",
        identity(3),
    ));
    compile(
        &root,
        PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap(),
    );
}

#[test]
fn membership_selection_filters_actual_cases_before_reporting_competing_owners() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("left.omg"),
        "module left; pub data Choice { case Empty; }",
    );
    Sources::write(
        root.join("right.omg"),
        "module right; pub data Choice { case Other; }",
    );
    Sources::write(
        root.join("main.omg"),
        "use left::Choice; use right::Choice;
         machine member(value: &left::Choice) -> bool { value in Choice::Empty }",
    );
    compile(&root, root_inputs(&root));
    Sources::write(
        root.join("right.omg"),
        "module right; pub data Choice { case Empty; }",
    );
    let errors =
        match compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root)) {
            Ok(_) => panic!("two eligible imported case owners must be ambiguous"),
            Err(errors) => errors,
        };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("ambiguous")
                && error.message.contains("left::Choice::Empty")
                && error.message.contains("right::Choice::Empty")),
        "{errors:?}"
    );
}

#[test]
fn qualified_cases_reject_wrong_owners_private_carriers_and_invalid_values() {
    for (declaration, source, expected) in [
        (
            "pub data Choice { case Empty; case Some(value: u32); }",
            "machine wrong(value: &Choice) -> bool { value in shapes::settings::Choice::Empty }",
            "membership",
        ),
        (
            "pub data Choice { case Empty; }",
            "machine wrong(value: Choice) -> bool { (&value) in shapes::settings::Choice::Empty }",
            "membership",
        ),
        (
            "data Choice { case Empty; }",
            "machine wrong(value: &shapes::settings::Choice) -> bool { value in shapes::settings::Choice::Empty }",
            "private",
        ),
        (
            "pub data Choice { case Empty; case Some(value: u32); }",
            "machine wrong() -> shapes::settings::Choice { shapes::settings::Choice::Some }",
            "not a declared local, parameter, field, or type",
        ),
        (
            "pub data Choice { value: u32 [1..=9]; case Empty; }",
            "machine wrong() -> shapes::settings::Choice { shapes::settings::Choice::Empty }",
            "omits gated field",
        ),
        (
            "pub data Choice { case Empty; }",
            "machine wrong(shapes: u32) -> shapes::settings::Choice { shapes::settings::Choice::Empty }",
            "static path through runtime binding `shapes`",
        ),
        (
            "pub data Choice { case Empty; }",
            "machine wrong(value: &shapes::settings::Choice) -> bool { value in shapes::settings::Choice::Missing }",
            "unknown domain",
        ),
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        let shapes = tree.package("shapes");
        Sources::write(
            shapes.join("settings.omg"),
            &format!("module settings; {declaration}"),
        );
        Sources::write(
            root.join("main.omg"),
            &format!("use shapes::settings; data Choice {{ case Empty; }} {source}"),
        );
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "shapes", shapes),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "shapes",
                identity(2),
            )],
        )
        .unwrap();
        let errors = match compile_to_checked_with_packages(&root.join("main.omg"), None, inputs) {
            Ok(_) => {
                panic!("qualification bypassed case or source authority obligations: {source}")
            }
            Err(errors) => errors,
        };
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{source}: {errors:?}"
        );
    }
}
