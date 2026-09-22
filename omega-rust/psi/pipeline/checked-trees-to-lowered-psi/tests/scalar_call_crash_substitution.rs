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

#[test]
fn scalar_calls_keep_exact_callee_substitution() {
    use terminal_psi::{CrashCause, OperationKind};
    use terminal_verifier::{ModuleError, validate_module};

    let artifact = encoded(
        r#"
        machine choose(left: bool, right: bool) -> bool
        crashes Trap right
        { transition { !right -> left } crash Trap; }
        machine value(left: bool, right: bool) -> bool
        crashes Trap
        { choose(left, right) }
    "#,
    );
    for left in [false, true] {
        assert_return(
            &artifact,
            &[
                TerminalScalarValue::Boolean(left),
                TerminalScalarValue::Boolean(false),
            ],
            left,
        );
        let failure = interpret_terminal_artifact(
            &artifact.0,
            &artifact.1,
            &AdmissionProfile::default(),
            &[
                TerminalScalarValue::Boolean(left),
                TerminalScalarValue::Boolean(true),
            ],
        )
        .unwrap_err();
        assert!(matches!(
            failure,
            terminal_interpreter::TerminalArtifactInterpretError::Execution(
                terminal_interpreter::TerminalInterpretError::Crash(crash)
            ) if crash.cause == CrashCause::Trap
        ));
    }
    let module = terminal_codec::decode_module(&artifact.0).unwrap();
    validate_module(&module).expect("unmodified callee substitution validates");
    for mutation in 0..3 {
        let mut changed = module.clone();
        let caller = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        let (arguments, crash_continuations) = caller
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                OperationKind::Call {
                    arguments,
                    crash_continuations,
                    ..
                } => Some((arguments, crash_continuations)),
                _ => None,
            })
            .expect("wrapper retains its scalar call");
        assert_eq!(arguments.len(), 2);
        assert_eq!(crash_continuations.len(), 1);
        match mutation {
            0 => arguments.swap(0, 1),
            1 => crash_continuations.clear(),
            2 => crash_continuations[0].cause = CrashCause::Abort,
            _ => unreachable!(),
        }
        assert!(
            matches!(
                validate_module(&changed),
                Err(ModuleError::CallCrashContinuationsMismatch { .. })
            ),
            "mutation {mutation} must not change exact call substitution"
        );
    }
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
