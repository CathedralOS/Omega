//! Published call routes bind to emitted argument values, not folded source guards.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

fn encoded(source: &str) -> (Vec<u8>, Vec<u8>) {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("canonical semantics"),
        encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("canonical proof"),
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
fn inferred_callee_ceiling_reaches_scalar_contracts_and_continuations() {
    // `middle` authors no crash clause: its InternalInferred ceiling is the
    // Trap route surviving its call to `inner`. Scalar machine contracts and
    // call continuations must publish that ceiling or the verifier sees an
    // uncovered Trap route.
    for argument in ["input", "input && true"] {
        let source = format!(
            r#"
            machine inner(flag: bool) -> bool
            requires true == true
            ensures true == true
            crashes Trap flag
            {{ !flag }}
            machine middle(input: bool) -> bool
            requires true == true
            ensures true == true
            {{ inner(input) }}
            machine value(input: bool) -> bool
            requires true == true
            ensures true == true
            crashes Trap
            {{ let selected: bool = middle({argument}); selected }}
            "#,
        );
        let artifact = encoded(&source);
        for input in [false, true] {
            assert_return(&artifact, &[TerminalScalarValue::Boolean(input)], !input);
        }
    }
}
