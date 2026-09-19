use super::{
    Sources, assert_same_machine_types, compile, compile_to_checked, identity, root_inputs,
    selections,
};
use compiler::CheckedCompileRequest;

const FLAG: &str = "pub data Flag<const Enabled: bool> { value: u8; }";

fn keep(name: &str, index: &str) -> String {
    format!(
        "machine {name}(value: Flag<{index}>) -> Flag<{index}> {{ let local: Flag<{index}> = value; local }}"
    )
}

#[test]
fn unary_boolean_indices_preserve_canonical_identity_and_lexical_selection() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (expression, expected) in [
        ("!false", "true"),
        ("(!false)", "true"),
        ("!!false", "false"),
        ("!(false || true)", "false"),
        ("!settings::OFF", "true"),
        ("!(true || (1u8 / 0 == 0))", "false"),
        ("!(false && !settings::OFF)", "true"),
    ] {
        Sources::write(
            root.join("settings.omg"),
            "module settings; pub const OFF: bool = false;",
        );
        Sources::write(
            root.join("main.omg"),
            &format!(
                "use settings; {FLAG} {} {}",
                keep("keep", expression),
                keep("oracle", expected)
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
        if expression.contains("settings") {
            assert_eq!(selections(&checked, "settings::OFF", identity(1)).len(), 3);
        }
    }
}

#[test]
fn unary_complement_indices_preserve_integer_width_and_signedness() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, expression, expected) in [
        ("u8", "~1u8", "254"),
        ("u8", "(~1u8)", "254"),
        ("u8", "~~1u8", "1"),
        ("u16", "~0u16", "65535"),
        ("u32", "~0u32", "4294967295"),
        ("u64", "~0u64", "18446744073709551615"),
        ("i8", "~0i8", "-1"),
        ("i16", "~-1i16", "0"),
        ("i32", "~2147483647i32", "-2147483648"),
        ("i64", "~-9223372036854775808i64", "9223372036854775807"),
        ("u8", "~MASK", "254"),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "pub data Flag<const Bits: {carrier}> {{ value: u8; }}
                 const MASK: u8 = 1; {} {}",
                keep("keep", expression),
                keep("oracle", expected),
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        assert_same_machine_types(&checked, "keep", "oracle");
        if expression.contains("MASK") {
            assert_eq!(selections(&checked, "MASK", identity(1)).len(), 3);
        }
    }
}

#[test]
fn unary_indices_reject_wrong_carriers_and_unselected_invalid_operands() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, expression) in [
        ("bool", "!0"),
        ("bool", "!1u8"),
        ("bool", "~false"),
        ("bool", "!(true || !0)"),
        ("bool", "!(false && ~true)"),
        ("bool", "!(false || (1u8 / 0 == 0))"),
        ("bool", "~1u8"),
        ("u8", "!false"),
        ("u8", "~1u16"),
        ("u8", "~1"),
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "pub data Flag<const Value: {carrier}> {{ value: u8; }} {}",
                keep("keep", expression),
            ),
        );
        compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err(&format!("{expression} must not initialize {carrier}"));
    }
}

#[test]
fn unary_indices_reject_mismatched_specializations_and_shadowed_names() {
    let tree = Sources::new();
    let root = tree.package("root");
    for body in [
        "machine mismatch(value: Flag<!false>) -> Flag<false> { value }",
        "machine shadowed(OFF: bool, value: Flag<!OFF>) {}",
        "machine shadowed() { let OFF: bool = false; let value: Flag<!(true || OFF)>; }",
    ] {
        Sources::write(
            root.join("main.omg"),
            &format!("{FLAG} const OFF: bool = false; {body}"),
        );
        compile_to_checked(CheckedCompileRequest {
            package_inputs: Some(root_inputs(&root)),
            ..CheckedCompileRequest::new(&root.join("main.omg"), None)
        })
        .expect_err("canonicalization preserves applications and original lexical selection");
    }
}

#[test]
fn unary_constant_arguments_feed_source_free_terminal_execution() {
    let tree = Sources::new();
    let root = tree.package("root");
    for (carrier, expression, literal) in [("bool", "!false", "true"), ("u8", "~1u8", "254")] {
        Sources::write(
            root.join("main.omg"),
            &format!(
                "pub data Flag<const Value: {carrier}> {{ value: u8; }}
         machine enabled(value: Flag<{expression}>) -> u8 {{ value.value }}
         machine read() -> u8 {{
             let value: Flag<{literal}> = Flag {{ value: 7 }};
             enabled(value)
         }}"
            ),
        );
        let checked = compile(&root, root_inputs(&root));
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "read")
            .produce_artifact()
            .expect("constant specialization reaches Terminal");
        drop(checked);
        assert_eq!(
            terminal_interpreter::interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &proof_admission::AdmissionProfile::default(),
                &[],
            )
            .expect("source-free specialization executes"),
            terminal_interpreter::TerminalExecutionResult::Scalar(
                super::array_construction::integer(7, 8)
            )
        );
    }
}
