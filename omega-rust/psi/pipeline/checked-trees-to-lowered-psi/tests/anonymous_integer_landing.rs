//! Source-to-serialized-Terminal-Psi execution, without a native program entry.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn execute(source: &str, arguments: &[TerminalScalarValue]) -> TerminalExecutionResult {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked =
        lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics = encode_module(&lowered.semantic_module).expect("canonical semantic bytes");
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("canonical proof bytes");
    // The interpreter decodes and verifies the serialized sections independently;
    // no checked scalar tree or producer-owned module is an execution input.
    interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), arguments)
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
}

fn signed(value: i64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).expect("i32"),
        value: IntegerValue::Signed(i128::from(value)),
    }
}

fn unsigned(value: u64) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).expect("u32"),
        value: IntegerValue::Unsigned(u128::from(value)),
    }
}

#[test]
fn anonymous_fractional_cancellation_executes_the_exact_integer() {
    for (body, target, literal, expected) in [
        ("7 / 2 * 2", "i32", 7, signed(7)),
        ("(4097 / 4096) * 4096", "u32", 4097, unsigned(4097)),
    ] {
        let source = format!(
            "machine value() -> {target}\nrequires {literal}{target} == {literal}{target}\nensures {literal}{target} == {literal}{target}\n{{ {body} }}"
        );
        assert_eq!(
            execute(&source, &[]),
            TerminalExecutionResult::Scalar(expected)
        );
    }
}

#[test]
fn explicitly_typed_wrapping_division_executes_truncating_integer_arithmetic() {
    for (body, target, literal, expected) in [
        ("(7i32 as i32 in Wrapping) / 2 * 2", "i32", 6, signed(6)),
        (
            "(4097u32 as u32 in Wrapping) / 4096 * 4096",
            "u32",
            4096,
            unsigned(4096),
        ),
    ] {
        let source = format!(
            "machine value() -> {target} in Wrapping\nrequires {literal}{target} == {literal}{target}\nensures {literal}{target} == {literal}{target}\n{{ {body} }}"
        );
        assert_eq!(
            execute(&source, &[]),
            TerminalExecutionResult::Scalar(expected)
        );
    }
}

#[test]
fn anonymous_local_and_storage_landings_execute_after_serialization() {
    for body in [
        "let landed: i32 = 7 / 2 * 2; landed",
        "let mut landed: i32 = 0; landed = 7 / 2 * 2; landed",
    ] {
        let source = format!(
            "machine value() -> i32\nrequires 7i32 == 7i32\nensures 7i32 == 7i32\n{{ {body} }}"
        );
        assert_eq!(
            execute(&source, &[]),
            TerminalExecutionResult::Scalar(signed(7))
        );
    }
}

#[test]
fn mixed_wrapping_operands_execute_exact_and_typed_constant_subtrees() {
    for (body, expected) in [
        ("input * (4097 / 2 * 2)", 4097),
        ("(4097 / 2 * 2) * input", 4097),
        ("input * ((4097i32 as i32 in Wrapping) / 2 * 2)", 4096),
    ] {
        let source = format!(
            "machine value(input: i32 in Wrapping) -> i32 in Wrapping\nrequires {expected}i32 == {expected}i32\nensures {expected}i32 == {expected}i32\n{{ {body} }}"
        );
        assert_eq!(
            execute(&source, &[signed(1)]),
            TerminalExecutionResult::Scalar(signed(expected))
        );
    }
}

#[test]
fn explicit_integer_cast_lands_the_complete_anonymous_value() {
    let source = "machine value() -> i32\nrequires 7i32 == 7i32\nensures 7i32 == 7i32\n{ (7 / 2 * 2) as i32 }";
    assert_eq!(
        execute(source, &[]),
        TerminalExecutionResult::Scalar(signed(7))
    );
}
