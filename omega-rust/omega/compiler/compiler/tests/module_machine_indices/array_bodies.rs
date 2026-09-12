use super::{
    PackageCompilationInputs, PackageDependencyBinding, PackageSourceBinding, Sources, compile,
    compile_to_checked, identity, root_inputs, selections,
};
use build_time_evaluation::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};
use compiler::CheckedCompileRequest;

#[test]
fn array_constant_bodies_keep_exact_module_values_and_independent_copies() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, first) in [("first", 3), ("second", 5)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
            "module {module}; pub data Sizes {{}} pub const Sizes::VALUES: [u8; 2] = [{first}, 2];
             machine read() -> [u8; 2] {{ let value: [u8; 2] = Sizes::VALUES; value }}"
        ),
        );
    }
    for imports in ["use first; use second;", "use second; use first;"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} use first::Sizes::VALUES;
             data Sizes {{}} const Sizes::VALUES: [u8; 2] = [1, 2];
             machine read() -> [u8; 2] {{ let value: [u8; 2] = Sizes::VALUES; value }}
             machine imported() -> [u8; 2] {{ let value: [u8; 2] = VALUES; value }}
             machine selected() -> [u8; 2] {{ let value: [u8; 2] = second::Sizes::VALUES; value }}
             machine independent() -> [u8; 2] {{
                 let mut value: [u8; 2] = first::Sizes::VALUES;
                 value[0] = 9;
                 let untouched: [u8; 2] = first::Sizes::VALUES;
                 untouched
             }}
             machine shadow() -> [u8; 2] {{ let VALUES: [u8; 2] = [7, 2]; VALUES }}"
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        for (path, first) in [
            ("read", 1),
            ("first::read", 3),
            ("second::read", 5),
            ("imported", 3),
            ("selected", 5),
            ("independent", 3),
            ("shadow", 7),
        ] {
            let machine = checked
                .typed
                .machines()
                .iter()
                .find(|machine| checked.symbols.display_path(machine.symbol, "::") == path)
                .expect("array consumer");
            let result = BuildTimeAdmissionPlan::infer(&checked.typed)
                .evaluate_machine_symbol_for_invocation_measured(
                    &checked.typed,
                    machine.symbol,
                    vec![],
                    BuildTimeInvocationCustody::Symbol(machine.symbol),
                )
                .expect("array constant value evaluates");
            assert_eq!(
                result.value(),
                &BuildTimeValue::Array(vec![BuildTimeValue::Int(first), BuildTimeValue::Int(2)]),
                "{path}"
            );
        }
        assert_eq!(selections(&checked, "first::Sizes", identity(1)).len(), 1);
        assert_eq!(selections(&checked, "second::Sizes", identity(1)).len(), 1);
    }
}

#[test]
fn dynamic_array_constant_projection_retains_the_value_indexing_boundary() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; data Sizes {} const Sizes::VALUES: [u8; 2] = [3, 2];",
    );
    for expression in [
        "settings::Sizes::VALUES[position]",
        "settings::Sizes::VALUES[0..1]",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("use settings; machine projected(position: u64) -> u8 {{ {expression} }}"),
        );
        let diagnostics = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("temporary array projection must not acquire place semantics");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("value-based array projection")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn array_constant_declared_shape_survives_empty_and_nested_empty_values() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (declared, value, wrong) in [
        ("[u8; 0]", "[]", "[bool; 0]"),
        ("[u8; 0]", "[]", "[u64; 0]"),
        ("[[u8; 0]; 1]", "[[]]", "[[bool; 0]; 1]"),
        ("[[u8; 2]; 0]", "[]", "[[u8; 3]; 0]"),
        ("[u8; 2]", "[1, 2]", "[u64; 2]"),
        ("[bool; 2]", "[true, false]", "[u8; 2]"),
        ("[[bool; 2]; 1]", "[[true, false]]", "[[u8; 2]; 1]"),
    ] {
        Sources::write(
            root.join("settings.omg"),
            &format!("module settings; data Sizes {{}} const Sizes::VALUES: {declared} = {value};"),
        );
        for destination in [declared, wrong] {
            for consumer in [
                format!("machine read() {{ let value: {destination} = settings::Sizes::VALUES; }}"),
                format!("machine read() -> {destination} {{ settings::Sizes::VALUES }}"),
                format!(
                    "machine take(value: {destination}) {{}} machine read() {{ take(settings::Sizes::VALUES); }}"
                ),
                format!(
                    "data Holder {{ value: {destination}; }} machine read() {{ let value: Holder = Holder {{ value: settings::Sizes::VALUES }}; }}"
                ),
            ] {
                Sources::write(root.join("main.omg"), &format!("use settings; {consumer}"));
                let result = compile_to_checked(CheckedCompileRequest {
                    package_inputs: Some(root_inputs(&root)),
                    ..CheckedCompileRequest::new(&root.join("main.omg"), None)
                });
                if destination == declared {
                    result.expect("matching declared array carrier checks");
                } else {
                    let diagnostics =
                        result.expect_err("substitution cannot anonymize a declared array carrier");
                    assert!(
                        diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.message.contains("constant array")),
                        "{declared} -> {destination}: {consumer}: {diagnostics:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn array_constant_body_substitution_preserves_dependency_and_visibility_checks() {
    let tree = Sources::new();
    let root = tree.package("root");
    let dependency = tree.package("dependency");
    let mut sources = vec![
        PackageSourceBinding::new(identity(1), "root", root.clone()),
        PackageSourceBinding::new(identity(2), "dependency", dependency.clone()),
    ];
    Sources::write(
        root.join("main.omg"),
        "use dependency::settings; machine read() -> [u8; 2] { let value: [u8; 2] = dependency::settings::Sizes::VALUES; value }",
    );
    for public in [true, false] {
        Sources::write(
            dependency.join("settings.omg"),
            &format!(
                "module settings; pub data Sizes {{}} {} const Sizes::VALUES: [u8; 2] = [1, 2];",
                if public { "pub" } else { "" }
            ),
        );
        let inputs = PackageCompilationInputs::new_package(
            identity(1),
            sources.clone(),
            vec![PackageDependencyBinding::new(
                identity(1),
                "dependency",
                identity(2),
            )],
        )
        .expect("direct dependency graph");
        let result = compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        });
        if public {
            let checked = result.expect("public array body checks through direct dependency");
            assert_eq!(
                selections(&checked, "settings::Sizes::VALUES", identity(2)).len(),
                1
            );
        } else {
            let diagnostics = result.expect_err("copying cannot erase private constant selection");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("private const")),
                "{diagnostics:?}"
            );
        }
    }
    let middle = tree.package("middle");
    Sources::write(
        middle.join("bridge.omg"),
        "use dependency::settings; pub machine bridge() -> u64 { 1 }",
    );
    Sources::write(
        dependency.join("settings.omg"),
        "module settings; pub data Sizes {} pub const Sizes::VALUES: [u8; 2] = [1, 2];",
    );
    Sources::write(
        root.join("main.omg"),
        "use middle::bridge; use dependency::settings; machine read() -> [u8; 2] { let value: [u8; 2] = dependency::settings::Sizes::VALUES; value }",
    );
    sources.push(PackageSourceBinding::new(identity(3), "middle", middle));
    let inputs = PackageCompilationInputs::new_package(
        identity(1),
        sources,
        vec![
            PackageDependencyBinding::new(identity(1), "middle", identity(3)),
            PackageDependencyBinding::new(identity(3), "dependency", identity(2)),
        ],
    )
    .expect("reachable transitive package without a direct dependency");
    assert!(
        compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(inputs),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .is_err()
    );
}
