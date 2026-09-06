//! Scalar call destinations land complete anonymous values before serialization.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffect, TerminalEffectHandler, TerminalEffectRejection,
    TerminalExecutionResult, TerminalInterpretError, TerminalScalarValue,
    interpret_terminal_artifact, interpret_terminal_artifact_with_effect_handler_measured,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::lower_typed_trees;

fn encoded(source: &str) -> (Vec<u8>, Vec<u8>) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let checked =
        lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("canonical semantic bytes"),
        encode_proof_bundle(&lowered.proof_bundle).expect("canonical proof bytes"),
    )
}

fn scalar(destination: &str, value: u32) -> TerminalScalarValue {
    let (sign, value) = match destination {
        "i32" => (IntegerSign::Signed, IntegerValue::Signed(i128::from(value))),
        "u32" => (
            IntegerSign::Unsigned,
            IntegerValue::Unsigned(u128::from(value)),
        ),
        _ => panic!("fixture uses a fixed 32-bit integer"),
    };
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(sign, 32).unwrap(),
        value,
    }
}

fn assert_returned(source: &str, destination: &str, expected: u32) {
    // The producer is gone before interpretation. Only serialized semantics
    // and proof cross the independent decoder/verifier execution boundary.
    let (semantics, proof) = encoded(source);
    let result = interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), &[])
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    assert_eq!(
        result,
        TerminalExecutionResult::Scalar(scalar(destination, expected)),
        "{source}",
    );
}

fn identity_source(destination: &str, policy: &str, body: &str) -> String {
    format!(
        r#"
        machine identity(input: {destination}{policy}) -> {destination}{policy}
        requires 0{destination} == 0{destination}
        ensures 0{destination} == 0{destination}
        {{ input }}
        machine value() -> {destination}{policy}
        requires 0{destination} == 0{destination}
        ensures 0{destination} == 0{destination}
        {{ {body} }}
        "#,
    )
}

#[test]
fn returned_calls_land_fractional_cancellation_as_seven_and_4097() {
    for (expression, destination, expected) in [
        ("7 / 2 * 2", "i32", 7),
        ("(4097 / 4096) * 4096", "u32", 4097),
    ] {
        let source = identity_source(destination, "", &format!("identity({expression})"));
        assert_returned(&source, destination, expected);
    }
}

#[test]
fn local_call_bindings_preserve_complete_anonymous_argument_values() {
    for (expression, destination, expected) in [
        ("7 / 2 * 2", "i32", 7),
        ("(4097 / 4096) * 4096", "u32", 4097),
    ] {
        for body in [
            format!("let landed: {destination} = identity({expression}); landed"),
            format!("let landed: {destination} = identity({expression}); identity(landed)"),
        ] {
            assert_returned(
                &identity_source(destination, "", &body),
                destination,
                expected,
            );
        }
    }
}

#[test]
fn explicitly_typed_wrapping_call_arguments_retain_truncating_division() {
    // ExactMultiply currently has a separate Terminal proof frontier. These
    // execution controls explicitly select Wrapping; they do not substitute
    // a wrapping result for the typed checker's Exact arithmetic contract.
    for (expression, destination, expected) in [
        ("(7i32 as i32 in Wrapping) / 2 * 2", "i32", 6),
        ("(4097u32 as u32 in Wrapping) / 4096 * 4096", "u32", 4096),
    ] {
        for body in [
            format!("identity({expression})"),
            format!("let landed: {destination} in Wrapping = identity({expression}); landed"),
        ] {
            assert_returned(
                &identity_source(destination, " in Wrapping", &body),
                destination,
                expected,
            );
        }
    }
}

#[test]
fn named_state_argument_lands_exact_seven_before_transition() {
    let source = r#"
        machine value() -> i32
        requires 7i32 == 7i32
        ensures 7i32 == 7i32
        {
            transition { _ -> finish(7 / 2 * 2) }
            state finish(input: i32) -> i32 { input }
        }
    "#;
    assert_returned(source, "i32", 7);
}

#[test]
fn anonymous_call_argument_can_exceed_machine_integer_range_before_landing() {
    let expression = "(18446744073709551615 + 7) - 18446744073709551615";
    for destination in ["i32", "u32"] {
        assert_returned(
            &identity_source(destination, "", &format!("identity({expression})")),
            destination,
            7,
        );
    }
}

#[derive(Default)]
struct ObserveArguments(Vec<Vec<TerminalScalarValue>>);

impl TerminalEffectHandler for ObserveArguments {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            panic!("fixture emits only the checked boundary call");
        };
        assert!(structural_arguments.is_empty());
        self.0.push(arguments.clone());
        Ok(())
    }
}

#[test]
fn statement_calls_deliver_exact_arguments_to_the_callee_after_serialization() {
    for indirect in [false, true] {
        let callee = if indirect { "forward" } else { "Sink::observe" };
        let source = format!(
            r#"
            boundary trait Sink {{ machine observe(first: i32, second: u32) reaches Sink; }}
            machine forward(first: i32, second: u32)
            reaches Sink
            {{ Sink::observe(first, second); }}
            machine value()
            reaches Sink
            {{ {callee}(7 / 2 * 2, (4097 / 4096) * 4096); }}
            "#,
        );
        let (semantics, proof) = encoded(&source);
        let mut observer = ObserveArguments::default();
        let result = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[],
            &mut observer,
        )
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
        assert_eq!(result.value(), TerminalExecutionResult::Unit);
        assert_eq!(
            observer.0,
            vec![vec![scalar("i32", 7), scalar("u32", 4097)]]
        );
    }
}

#[test]
fn anonymous_middle_argument_preserves_first_crash_order_before_the_outer_call() {
    for (first, second, expected) in [
        ("Abort", "Trap", terminal_psi::CrashCause::Abort),
        ("Trap", "Abort", terminal_psi::CrashCause::Trap),
    ] {
        let source = format!(
            r#"
            boundary trait Sink {{
                machine observe(first: bool, middle: i32, last: bool) reaches Sink;
            }}
            machine first() -> bool crashes {first} {{ crash {first}; }}
            machine second() -> bool crashes {second} {{ crash {second}; }}
            machine value()
            reaches Sink
            crashes Abort
            crashes Trap
            {{
                Sink::observe(first(), 7 / 2 * 2, second());
            }}
            "#,
        );
        let (semantics, proof) = encoded(&source);
        let mut observer = ObserveArguments::default();
        let result = interpret_terminal_artifact_with_effect_handler_measured(
            &semantics,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[],
            &mut observer,
        );
        assert!(
            matches!(result,
                Err(TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::Crash(crash)
                )) if crash.cause == expected),
            "the first sibling's crash wins around the anonymous middle argument: {source}",
        );
        assert!(
            observer.0.is_empty(),
            "the outer call cannot run after the crash"
        );
    }
}
