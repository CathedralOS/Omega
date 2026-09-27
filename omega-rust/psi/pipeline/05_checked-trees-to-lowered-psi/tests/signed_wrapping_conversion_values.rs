//! ARITHMETIC-POLICY-REALIZATION: a signed or mixed-sign `in Wrapping`
//! conversion must produce the two's-complement modular image, not a
//! truncation toward zero — narrowing, same-width, and sign-widening pairs
//! alike. Lowering composing is not evidence of that; these execute the
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

/// `u16`/`i32`/… spell `<sign><bits>`; the tests name carriers so the row's
/// own width is never retyped by hand.
fn carrier_bits(name: &str) -> u16 {
    name[1..].parse().expect("a fixed integer width")
}

#[test]
fn a_sign_widening_executes_the_modular_image() {
    // A signed source reaching a WIDER unsigned destination borrows the
    // smallest signed carrier above the target for its mask.
    for (source_type, target, input, expected) in [
        ("i16", "u16", -1_i128, 65535_u128),
        ("i16", "u16", -32768, 32768),
        ("i16", "u16", 32767, 32767),
        ("i8", "u16", -1, 65535),
        ("i16", "u32", -1, 4294967295),
        ("i16", "u32", -200, 4294967096),
        ("i32", "u32", -1, 4294967295),
    ] {
        assert_eq!(
            execute(
                &narrow(source_type, target),
                &[integer(
                    IntegerSign::Signed,
                    carrier_bits(source_type),
                    IntegerValue::Signed(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Unsigned,
                carrier_bits(target),
                IntegerValue::Unsigned(expected)
            )),
            "{source_type} -> {target} in Wrapping of {input}"
        );
    }
}

#[test]
fn the_widest_sign_crossings_execute_the_modular_image() {
    // `u64` has no wider signed carrier to mask in, so the residue's halves
    // cross separately: `i8 -> u64` widens to `i64` first, `i64 -> u64` and
    // `u64 -> i64` split at the sign bit itself.
    assert_eq!(
        execute(
            &narrow("i8", "u64"),
            &[integer(IntegerSign::Signed, 8, IntegerValue::Signed(-1))],
        ),
        TerminalExecutionResult::Scalar(integer(
            IntegerSign::Unsigned,
            64,
            IntegerValue::Unsigned(18446744073709551615)
        )),
        "i8 -> u64 in Wrapping of -1"
    );
    for (input, expected) in [
        (-1_i128, 18446744073709551615_u128),
        (i128::from(i64::MIN), 1_u128 << 63),
        (i128::from(i64::MAX), u128::from(i64::MAX as u64)),
        (0, 0),
    ] {
        assert_eq!(
            execute(
                &narrow("i64", "u64"),
                &[integer(
                    IntegerSign::Signed,
                    64,
                    IntegerValue::Signed(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Unsigned,
                64,
                IntegerValue::Unsigned(expected)
            )),
            "i64 -> u64 in Wrapping of {input}"
        );
    }
    for (input, expected) in [
        (18446744073709551615_u128, -1_i128),
        (1_u128 << 63, i128::from(i64::MIN)),
        (u128::from(i64::MAX as u64), i128::from(i64::MAX)),
        (0, 0),
    ] {
        assert_eq!(
            execute(
                &narrow("u64", "i64"),
                &[integer(
                    IntegerSign::Unsigned,
                    64,
                    IntegerValue::Unsigned(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Signed,
                64,
                IntegerValue::Signed(expected)
            )),
            "u64 -> i64 in Wrapping of {input}"
        );
    }
}
