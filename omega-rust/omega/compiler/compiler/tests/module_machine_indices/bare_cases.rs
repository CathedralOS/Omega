use super::*;
use compiler::CheckedCompileRequest;

#[test]
fn package_bare_cases_share_brace_constructor_identity_and_authority() {
    let tree = Sources::new();
    let root = tree.package("root");
    let leaf = tree.package("leaf");
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; pub data Value { case Empty; case Present(value: u32); }",
    );
    let source = "use leaf::settings;
        machine bare() -> leaf::settings::Value { leaf::settings::Value::Empty }
        machine braces() -> leaf::settings::Value { leaf::settings::Value::Empty {} }";
    Sources::write(root.join("main.omg"), source);
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        vec![
            PackageSourceBinding::new(identity(1), "root", root.clone()),
            PackageSourceBinding::new(identity(2), "leaf", leaf),
        ],
        vec![PackageDependencyBinding::new(
            identity(1),
            "leaf",
            identity(2),
        )],
    )
    .unwrap();
    let checked = compile(&root, inputs);
    let mut landed = Vec::new();
    for name in ["bare", "braces"] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let [state] = checked.typed.machine_states(machine) else {
            panic!("one state");
        };
        let [StatementNode::Expression(expression)] = checked
            .typed
            .statement_table
            .statements(state.statement_nodes)
        else {
            panic!("constructor return");
        };
        let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
            checked.typed.expression_table.expression(*expression)
        else {
            panic!("bare and braced values share resolved construction");
        };
        assert!(literal.fields.is_empty());
        landed.push((literal.type_symbol, literal.case_symbol));
    }
    assert_eq!(
        landed[0], landed[1],
        "both spellings retain the same exact nominal constructor"
    );
    let constructors = selections(&checked, "settings::Value::Empty", identity(2));
    assert_eq!(
        constructors.len(),
        2,
        "each authored constructor retains its actual case: {constructors:?}"
    );
    let original = root.join("main.omg").canonicalize().unwrap();
    for (start, _) in source.match_indices("leaf::settings::Value::Empty") {
        assert!(constructors.iter().any(|selection| {
            let span = selection.source_span();
            checked
                .symbols
                .source_file(span)
                .is_some_and(|source| source.path == original)
                && (start..start + "leaf::settings::Value::Empty".len()).contains(&span.span.start)
        }));
    }
}

#[test]
fn bare_case_values_preserve_payload_defaults_shadowing_and_nominal_type() {
    for (declaration, body, expected) in [
        (
            "pub data Value { case Empty; case Present(value: u32); }",
            "leaf::settings::Value::Present",
            "not a declared local",
        ),
        (
            "pub data Value { value: u32 [1..=9]; case Empty; }",
            "leaf::settings::Value::Empty",
            "omits gated field",
        ),
        (
            "pub boundary data Value;",
            "leaf::settings::Value::Empty",
            "not a declared local",
        ),
        (
            "pub data Value { case Empty; }",
            "leaf::settings::Value::Missing",
            "not a declared local",
        ),
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        let leaf = tree.package("leaf");
        Sources::write(
            leaf.join("settings.omg"),
            &format!("module settings; {declaration}"),
        );
        Sources::write(
            root.join("main.omg"),
            &format!("use leaf::settings; machine make() -> leaf::settings::Value {{ {body} }}"),
        );
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "leaf", leaf),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "leaf",
                identity(2),
            )],
        )
        .unwrap();
        let errors = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("a bare value must retain constructor obligations");
        assert!(
            errors.iter().any(|error| error.message.contains(expected)),
            "{errors:?}"
        );
    }
}

#[test]
fn bare_package_cases_cannot_escape_local_shadow_or_nominal_destination() {
    for source in [
        "use leaf::settings; machine make(leaf: u32) -> leaf::settings::Value { leaf::settings::Value::Empty }",
        "use leaf::settings; machine make() -> leaf::settings::Value { let leaf: u32 = 1; leaf::settings::Value::Empty }",
        "use leaf::settings; data Other { case Empty; } machine make() -> Other { leaf::settings::Value::Empty }",
    ] {
        let tree = Sources::new();
        let root = tree.package("root");
        let leaf = tree.package("leaf");
        Sources::write(
            leaf.join("settings.omg"),
            "module settings; pub data Value { case Empty; }",
        );
        Sources::write(root.join("main.omg"), source);
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root.clone()),
                PackageSourceBinding::new(identity(2), "leaf", leaf),
            ],
            vec![PackageDependencyBinding::new(
                identity(1),
                "leaf",
                identity(2),
            )],
        )
        .unwrap();
        let errors = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("case construction cannot bypass lexical or nominal identity");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains("StaticPathSegment")
                    || error.message.contains("terminal expression")),
            "{errors:?}"
        );
    }
}

#[test]
fn bare_package_case_authority_requires_direct_dependency_and_public_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    let middle = tree.package("middle");
    let leaf = tree.package("leaf");
    Sources::write(
        middle.join("bridge.omg"),
        "use leaf::settings; pub machine bridge() -> u32 { 1 }",
    );
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; pub data Value { case Empty; }",
    );
    Sources::write(
        root.join("main.omg"),
        "use middle::bridge; use leaf::settings; machine make() -> leaf::settings::Value { leaf::settings::Value::Empty }",
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
    compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            PackageCompilationInputs::new_package(
                identity(1),
                sources.clone(),
                dependencies.clone(),
            )
            .unwrap(),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("transitive loading grants no direct selection authority");
    dependencies.push(PackageDependencyBinding::new(
        identity(1),
        "leaf",
        identity(3),
    ));
    compile(
        &root,
        PackageCompilationInputs::new_package(identity(1), sources.clone(), dependencies.clone())
            .unwrap(),
    );
    Sources::write(
        leaf.join("settings.omg"),
        "module settings; data Value { case Empty; }",
    );
    let errors = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(
            PackageCompilationInputs::new_package(identity(1), sources, dependencies).unwrap(),
        ),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("direct dependency does not expose a private carrier");
    assert!(
        errors.iter().any(|error| error.message.contains("private")),
        "{errors:?}"
    );
}

#[test]
fn bare_case_constructor_ambiguity_cannot_fall_back_to_a_simpler_name() {
    assert_bare_case_ambiguity(false);
}

#[test]
fn ambiguous_constant_prefix_cannot_be_redirected_to_a_case() {
    assert_bare_case_ambiguity(true);
}

fn assert_bare_case_ambiguity(constants: bool) {
    {
        let tree = Sources::new();
        let root = tree.package("root");
        for module in ["left", "right"] {
            Sources::write(
                root.join(format!("{module}.omg")),
                &if constants {
                    format!("module {module}; pub const Choice: u32 = 1;")
                } else {
                    format!("module {module}::Choice; pub data Value {{}}")
                },
            );
        }
        Sources::write(
            root.join("main.omg"),
            "use left::Choice; use right::Choice; data Choice { case Value; }
             machine make() -> Choice { Choice::Value }",
        );
        let Err(errors) = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        }) else {
            panic!("ambiguous declarations cannot become the unique root case");
        };
        assert!(
            errors.iter().any(|error| error.source_span.is_some()
                && error.message.contains("ambiguous")
                && error.message.contains("left::Choice")
                && error.message.contains("right::Choice")
                && error.message.contains("source imports")),
            "{errors:?}"
        );
    }
}

#[test]
fn bare_case_namespace_cannot_capture_a_generic_binder() {
    for binder in ["settings", "const settings: u32"] {
        let tree = Sources::new();
        let root = tree.package("root");
        Sources::write(
            root.join("settings.omg"),
            "module settings; pub data Value { case Empty; }",
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; machine make<{binder}>() -> settings::Value {{ settings::Value::Empty }}"
            ),
        );
        assert!(
            compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(root_inputs(&root)),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .is_err(),
            "a binder cannot be reinterpreted as a module namespace"
        );
    }
}

#[test]
fn bare_case_namespace_cannot_capture_a_conformance_binder() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Choice [copy] { case Ready; case Waiting; }",
    );
    for binder in ["Selected", "settings"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; trait Ranked {{}}
                 machine make<Element, {binder}: Element satisfies Ranked>(value: &Element) -> bool {{
                     settings::Choice::Ready == settings::Choice::Ready
                 }}"
            ),
        );
        let result = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        if binder == "Selected" {
            result.expect("an unrelated conformance binder permits module case construction");
        } else {
            assert!(
                result.is_err(),
                "a proof-static conformance binder cannot become a module qualifier"
            );
        }
    }
}

#[test]
fn bare_case_namespace_cannot_capture_a_named_state() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; pub data Choice [copy] { case Ready; case Waiting; }",
    );
    for state in ["finish", "settings"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; data Main {{}}
                 machine Main::run(&mut self) {{
                     let selected: bool = settings::Choice::Ready == settings::Choice::Ready;
                     transition {{ _ -> {state}() }}
                     state {state}(&mut self) {{}}
                 }}"
            ),
        );
        let result = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        if state == "finish" {
            result.expect("an unrelated named state permits module case construction");
        } else {
            assert!(
                result.is_err(),
                "an exact machine state cannot become a module qualifier"
            );
        }
    }
}

#[test]
fn normalized_bare_case_retains_checked_tag_predicate() {
    assert_normalized_case_predicate("");
}

#[test]
fn normalized_copy_case_retains_checked_tag_predicate() {
    assert_normalized_case_predicate("[copy]");
}

fn assert_normalized_case_predicate(multiplicity: &str) {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("main.omg"),
        &format!(
            "data Outcome {multiplicity} {{ case Success; case Failure; }}
         data Root {{ result: Outcome; }}
         machine Root::matches(&mut self) {{
             let selected: bool = self.result == Outcome::Success;
             transition selected {{ true -> yes() false -> no() }}
             state yes(&mut self) {{}} state no(&mut self) {{}}
         }}"
        ),
    );
    let checked = compile(&root, root_inputs(&root));
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == "Root::matches")
        .unwrap();
    let state = &checked.typed.machine_states(machine)[0];
    let expression = checked.facts.values.scalar_expressions.expression_at(
        state.symbol,
        0,
        checked_trees::CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 0 },
    );
    assert!(
        matches!(expression,
            Some(checked_trees::CheckedScalarExpression::Boolean(predicate))
            if matches!(predicate.as_ref(), checked_trees::CheckedBooleanExpression::StructuralCaseMembership { case, .. } if case == "Success")
        ),
        "the exact scalar-sum tag predicate must survive value normalization: {expression:?}"
    );
}

#[test]
fn qualified_constant_cannot_share_a_case_carrier_namespace() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("values.omg"),
        "module values; pub const Choice: u32 = 1; pub data Choice { case Empty; }",
    );
    Sources::write(
        root.join("main.omg"),
        "use values; machine make() -> values::Choice { values::Choice::Empty }",
    );
    let Err(errors) = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    }) else {
        panic!("a constant and case carrier cannot share the same qualified namespace");
    };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("values::Choice")
                && error.message.contains("collides")),
        "{errors:?}"
    );
}

#[test]
fn bare_case_value_equality_does_not_replace_payload_sum_membership() {
    let tree = Sources::new();
    let root = tree.package("root");
    let source = "data Sequence { case Empty; case Cons(head: u64, tail: Sequence); }
        machine empty(value: &Sequence) -> bool { value == Sequence::Empty }";
    Sources::write(root.join("main.omg"), source);
    let Err(errors) = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    }) else {
        panic!("payload-bearing sum value equality requires its declared conformance");
    };
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("Equatable")),
        "{errors:?}"
    );
    Sources::write(
        root.join("main.omg"),
        &source.replace("value == Sequence::Empty", "value in Sequence::Empty"),
    );
    compile(&root, root_inputs(&root));
}
