use super::{Sources, compile, compile_to_checked, identity, root_inputs, selections};
use build_time_evaluation::{BuildTimeAdmissionPlan, BuildTimeInvocationCustody, BuildTimeValue};
use compiler::CheckedCompileRequest;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{TerminalExecutionResult, TerminalScalarValue};

#[test]
fn static_array_constant_projections_execute_checked_and_decoded_terminal_values() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (module, value) in [("first", 3), ("second", 5)] {
        Sources::write(
            root.join(format!("{module}.omg")),
            &format!(
                "module {module}; data Sizes {{}}
             const Sizes::VALUES: [u8; 2] = [{value}, 2];
             const Sizes::FLAGS: [bool; 2] = [true, false];
             const Sizes::ROWS: [[u8; 2]; 1] = [[{value}, 2]];
             machine local() -> u8 {{ Sizes::VALUES[0] }}"
            ),
        );
    }
    for imports in ["use first; use second;", "use second; use first;"] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "{imports} use first::Sizes::VALUES;
             machine imported() -> u8 {{ VALUES[0] }}
             machine qualified() -> u8 {{ second::Sizes::VALUES[0] }}
             machine nested() -> u8 {{ second::Sizes::ROWS[0][1] }}
             machine keep(value: u8) -> u8 {{ value }}
             machine called() -> u8 {{ keep(second::Sizes::VALUES[0]) }}
             machine boolean() -> bool {{ first::Sizes::FLAGS[0] }}"
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert!(!selections(&checked, "first::Sizes::VALUES", identity(1)).is_empty());
        let mut artifacts = Vec::new();
        for (path, expected) in [
            ("first::local", BuildTimeValue::Int(3)),
            ("second::local", BuildTimeValue::Int(5)),
            ("imported", BuildTimeValue::Int(3)),
            ("qualified", BuildTimeValue::Int(5)),
            ("nested", BuildTimeValue::Int(2)),
            ("called", BuildTimeValue::Int(5)),
            ("boolean", BuildTimeValue::Bool(true)),
        ] {
            let machine = checked
                .typed
                .machines()
                .iter()
                .find(|machine| checked.symbols.display_path(machine.symbol, "::") == path)
                .expect("projection consumer");
            let execution = BuildTimeAdmissionPlan::infer(&checked.typed)
                .evaluate_machine_symbol_for_invocation_measured(
                    &checked.typed,
                    machine.symbol,
                    vec![],
                    BuildTimeInvocationCustody::Symbol(machine.symbol),
                )
                .expect("checked projection executes");
            assert_eq!(execution.value(), &expected, "{path}");
            let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, path)
                .expect("static projection lowers without constant storage");
            artifacts.push((
                terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics"),
                terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proof"),
                expected,
            ));
        }
        drop(checked);
        for (semantics, proof, expected) in artifacts {
            let result = terminal_interpreter::interpret_terminal_artifact(
                &semantics,
                &proof,
                &proof_admission::AdmissionProfile::default(),
                &[],
            )
            .expect("independently decode, verify and execute projection");
            let scalar = match expected {
                BuildTimeValue::Int(value) => TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                    value: IntegerValue::Unsigned(value as u128),
                },
                BuildTimeValue::Bool(value) => TerminalScalarValue::Boolean(value),
                _ => panic!("scalar oracle"),
            };
            assert_eq!(result, TerminalExecutionResult::Scalar(scalar));
        }
    }
}

#[test]
fn static_array_constant_projections_keep_bounds_and_result_types() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; data Sizes {}
         const Sizes::VALUES: [u8; 2] = [3, 2];
         const Sizes::EMPTY: [u8; 0] = [];
         const Sizes::ROWS: [[u8; 2]; 1] = [[3, 2]];
         const Sizes::FLAGS: [bool; 2] = [true, false];",
    );
    for (result_type, expression) in [
        ("u8", "settings::Sizes::VALUES[2]"),
        ("u8", "settings::Sizes::VALUES[-1]"),
        ("u8", "settings::Sizes::EMPTY[0]"),
        ("u8", "settings::Sizes::ROWS[1][0]"),
        ("u8", "settings::Sizes::ROWS[0][2]"),
        ("bool", "settings::Sizes::VALUES[0]"),
        ("u64", "settings::Sizes::VALUES[0]"),
        ("u8", "settings::Sizes::VALUES[0] + 253"),
        ("u8", "settings::Sizes::VALUES[0] + 1u64"),
        ("u8", "settings::Sizes::FLAGS[0]"),
        ("&u8", "&settings::Sizes::VALUES[0]"),
        ("u8", "(&settings::Sizes::VALUES)[0]"),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("use settings; machine projected() -> {result_type} {{ {expression} }}"),
        );
        assert!(
            compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(root_inputs(&root)),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .is_err(),
            "invalid projection must reject: {expression} -> {result_type}"
        );
    }
}

#[test]
fn constant_projection_types_are_checked_at_storage_call_and_conversion_sites() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; data Sizes {} const Sizes::VALUES: [u8; 2] = [7, 9];",
    );
    for consumer in [
        "machine read() { let value: u64 = settings::Sizes::VALUES[0]; }",
        "machine take(value: u64) {} machine read() { take(settings::Sizes::VALUES[0]); }",
        "data Holder { value: u64; } machine read() { let value: Holder = Holder { value: settings::Sizes::VALUES[0] }; }",
        "machine read() { let mut value: u64 = 0; value = settings::Sizes::VALUES[0]; }",
        "machine read() { settings::Sizes::VALUES[0] = 9; }",
    ] {
        Sources::write(root.join("main.omg"), &format!("use settings; {consumer}"));
        assert!(
            compile_to_checked(CheckedCompileRequest {
                package_inputs: Some(root_inputs(&root)),
                ..CheckedCompileRequest::new(&root.join("main.omg"), None)
            })
            .is_err(),
            "{consumer}"
        );
    }
    Sources::write(
        root.join("main.omg"),
        "use settings; machine read() -> u64 { settings::Sizes::VALUES[0] as u64 }",
    );
    let checked = compile(&root, root_inputs(&root));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "read")
        .expect("explicit widening lowers");
    let result = terminal_interpreter::interpret_terminal_artifact(
        &terminal_codec::encode_module(&lowered.semantic_module).expect("encode semantics"),
        &terminal_codec::encode_proof_bundle(&lowered.proof_bundle).expect("encode proof"),
        &proof_admission::AdmissionProfile::default(),
        &[],
    )
    .expect("explicit widening executes");
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"),
            value: IntegerValue::Unsigned(7),
        })
    );
}

#[test]
fn array_constant_projection_preserves_authored_index_bounds_obligations() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(
        root.join("settings.omg"),
        "module settings; data Sizes {} const Sizes::VALUES: [u8; 2] = [7, 9];",
    );
    Sources::write(
        root.join("main.omg"),
        "use settings; data Indexing {}
         operator [] index(items: &[u8], position: u64) -> u8;
         machine projected() -> u8 { settings::Sizes::VALUES[0] }",
    );
    let diagnostics = compile_to_checked(CheckedCompileRequest {
        package_inputs: Some(root_inputs(&root)),
        ..CheckedCompileRequest::new(&root.join("main.omg"), None)
    })
    .expect_err("builtin array bounds cannot discharge a different selected contract");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
                .contains("selected `requires` does not state the complete collection-relative bounds obligation")),
        "{diagnostics:?}"
    );
}
