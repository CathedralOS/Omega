//! Published call routes bind to emitted argument values, not folded source guards.

use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn encoded(source: &str) -> (Vec<u8>, Vec<u8>) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked =
        lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("canonical semantics"),
        encode_proof_bundle(&lowered.proof_bundle).expect("canonical proof"),
    )
}

fn assert_return(artifact: &(Vec<u8>, Vec<u8>), arguments: &[TerminalScalarValue], expected: bool) {
    // Independent decoding and proof verification see only bytes and runtime inputs.
    assert_eq!(
        interpret_terminal_artifact(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            arguments,
        )
        .unwrap(),
        TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(expected)),
    );
}

#[test]
fn literal_calls_preserve_published_guards_for_immutable_and_mutable_formals() {
    for (mutability, body) in [("", "!flag"), ("mut ", "flag = !flag; flag")] {
        for (argument, expected) in [(true, false), (false, true)] {
            let source = format!(
                r#"
                machine choose({mutability}flag: bool) -> bool
                requires {expected} == {expected}
                ensures {expected} == {expected}
                crashes Trap flag
                {{ {body} }}
                machine value() -> bool
                requires {expected} == {expected}
                ensures {expected} == {expected}
                crashes Trap
                {{ choose({argument}) }}
                "#,
            );
            assert_return(&encoded(&source), &[], expected);
        }
    }
}

#[test]
fn runtime_and_staged_arguments_keep_the_same_parameter_relative_crash_routes() {
    for argument in ["input", "input && true"] {
        let source = format!(
            r#"
            machine choose(flag: bool) -> bool
            requires true == true
            ensures true == true
            crashes Trap flag
            {{ !flag }}
            machine value(input: bool) -> bool
            requires true == true
            ensures true == true
            crashes Trap
            {{ let selected: bool = choose({argument}); selected }}
            "#,
        );
        let artifact = encoded(&source);
        for input in [false, true] {
            assert_return(&artifact, &[TerminalScalarValue::Boolean(input)], !input);
        }
    }
}
