use psi_core::{IntegerSign, IntegerType, IntegerValue};
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn anonymous_integer_returns_preserve_the_landed_value_through_terminal_execution() {
    for (expression, destination, sign, width, value) in [
        (
            "3 + 4",
            "u8",
            IntegerSign::Unsigned,
            8,
            IntegerValue::Unsigned(7),
        ),
        (
            "(255 + 1) - 1",
            "u8",
            IntegerSign::Unsigned,
            8,
            IntegerValue::Unsigned(255),
        ),
        (
            "(0 - 1) + 2",
            "u8",
            IntegerSign::Unsigned,
            8,
            IntegerValue::Unsigned(1),
        ),
        (
            "(127 + 1) - 1",
            "i8",
            IntegerSign::Signed,
            8,
            IntegerValue::Signed(127),
        ),
        (
            "(0 - 129) + 1",
            "i8",
            IntegerSign::Signed,
            8,
            IntegerValue::Signed(-128),
        ),
        (
            "(18446744073709551615 + 1) - 1",
            "u64",
            IntegerSign::Unsigned,
            64,
            IntegerValue::Unsigned(u128::from(u64::MAX)),
        ),
    ] {
        let expected = match value {
            IntegerValue::Signed(value) => value.to_string(),
            IntegerValue::Unsigned(value) => value.to_string(),
        };
        let source = format!(
            "machine value() -> {destination}\nrequires {expected}{destination} == {expected}{destination}\nensures {expected}{destination} == {expected}{destination}\n{{ {expression} }}"
        );
        let tokens = Lexer::new(&source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed)
            .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "value")
            .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
        let semantics = encode_module(&lowered.semantic_module).expect("encode semantics");
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("encode proof");
        let result =
            interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), &[])
                .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
        assert_eq!(
            result,
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(sign, width).expect("fixed integer type"),
                value,
            }),
            "{source}",
        );
    }
}
