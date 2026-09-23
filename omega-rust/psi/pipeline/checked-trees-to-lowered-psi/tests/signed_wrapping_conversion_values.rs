//! ARITHMETIC-POLICY-REALIZATION: a signed NARROWING `in Wrapping` conversion
//! must produce the two's-complement modular image, not a truncation toward
//! zero. Lowering composing is not evidence of that; these execute the
//! serialized module, so the interpreter's own decode and verification stand
//! between the composed spelling and the answer.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_codec::{encode_module, encode_proof_section};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};

fn integer(sign: IntegerSign, bits: u16, value: IntegerValue) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(sign, bits).expect("a fixed integer carrier"),
        value,
    }
}

fn execute(source: &str, arguments: &[TerminalScalarValue]) -> TerminalExecutionResult {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics = encode_module(&lowered.semantic_module).expect("canonical semantic bytes");
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("canonical proof bytes");
    interpret_terminal_artifact(&semantics, &proof, &AdmissionProfile::default(), arguments)
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
}

fn narrow(source_type: &str, target: &str) -> String {
    format!(
        "machine value(input: {source_type}) -> {target} \
         {{ (input as {target} in Wrapping) as {target} }}"
    )
}

#[test]
fn a_signed_source_narrowing_to_an_unsigned_target_executes_the_modular_image() {
    // Each row is one a truncation toward zero would get WRONG: a negative
    // operand's modular image is positive, and an out-of-range positive one
    // is not its own low bits.
    for (input, expected) in [
        (-1_i128, 255_u128),
        (300, 44),
        (-200, 56),
        (-256, 0),
        (0, 0),
        (255, 255),
    ] {
        assert_eq!(
            execute(
                &narrow("i16", "u8"),
                &[integer(
                    IntegerSign::Signed,
                    16,
                    IntegerValue::Signed(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Unsigned,
                8,
                IntegerValue::Unsigned(expected)
            )),
            "i16 -> u8 in Wrapping of {input}"
        );
    }
}

#[test]
fn a_wider_signed_source_narrowing_executes_the_modular_image() {
    // Not just the byte target: the spelling is bit-width general.
    for (input, expected) in [(-1_i128, 65535_u128), (-40000, 25536), (65536, 0)] {
        assert_eq!(
            execute(
                &narrow("i32", "u16"),
                &[integer(
                    IntegerSign::Signed,
                    32,
                    IntegerValue::Signed(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Unsigned,
                16,
                IntegerValue::Unsigned(expected)
            )),
            "i32 -> u16 in Wrapping of {input}"
        );
    }
}

#[test]
fn the_unsigned_neighbour_still_executes_its_own_remainder_route() {
    // The control that did not change: unsigned-to-unsigned narrowing keeps
    // the remainder spelling it already had, and still answers the same.
    for (input, expected) in [(300_u128, 44_u128), (255, 255), (256, 0)] {
        assert_eq!(
            execute(
                &narrow("u16", "u8"),
                &[integer(
                    IntegerSign::Unsigned,
                    16,
                    IntegerValue::Unsigned(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Unsigned,
                8,
                IntegerValue::Unsigned(expected)
            )),
            "u16 -> u8 in Wrapping of {input}"
        );
    }
}

#[test]
fn a_signed_destination_still_has_no_realization() {
    // The boundary, one coordinate away from every executing row above: the
    // same narrowing carriers with a SIGNED destination. The fold that would
    // place the modular image in the negative half has no bound the exact
    // cast can consume, so this is refused rather than answered.
    for (source_type, target) in [("i16", "i8"), ("u16", "i8"), ("i32", "i16")] {
        let checked = crate::front_end::checked_program(&narrow(source_type, target));
        let error = checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("value"),
        )
        .expect_err("a signed wrapping destination has no realization yet");
        assert_eq!(
            error,
            checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "signed wrapping conversion target requires a folded modular bound"
            ),
            "{source_type} -> {target} in Wrapping"
        );
    }
}
