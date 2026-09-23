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
fn a_signed_destination_narrowing_executes_the_modular_image() {
    // One coordinate away from the unsigned-destination rows above: the same
    // narrowing carriers with a SIGNED destination. The image's upper half
    // must land below zero — `130 -> -126` is the row a truncation toward
    // zero gets wrong — and each exact cast's operand carries the bound of
    // the mask that produced it.
    for (input, expected) in [
        (130_i128, -126_i128),
        (-1, -1),
        (-200, 56),
        (255, -1),
        (256, 0),
        (-128, -128),
        (-129, 127),
        (127, 127),
    ] {
        assert_eq!(
            execute(
                &narrow("i16", "i8"),
                &[integer(
                    IntegerSign::Signed,
                    16,
                    IntegerValue::Signed(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Signed,
                8,
                IntegerValue::Signed(expected)
            )),
            "i16 -> i8 in Wrapping of {input}"
        );
    }
}

#[test]
fn an_unsigned_source_narrowing_to_a_signed_target_executes_the_modular_image() {
    // The adjacent route through the other sign coordinate: `u16 -> i8`
    // shares the signed destination with `i16 -> i8` but reads the residue
    // off an unsigned carrier.
    for (input, expected) in [
        (255_u128, -1_i128),
        (200, -56),
        (300, 44),
        (128, -128),
        (127, 127),
        (65535, -1),
    ] {
        assert_eq!(
            execute(
                &narrow("u16", "i8"),
                &[integer(
                    IntegerSign::Unsigned,
                    16,
                    IntegerValue::Unsigned(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Signed,
                8,
                IntegerValue::Signed(expected)
            )),
            "u16 -> i8 in Wrapping of {input}"
        );
    }
}

#[test]
fn a_wider_signed_destination_narrowing_executes_the_modular_image() {
    // The signed-destination shape is bit-width general, not a byte target.
    for (input, expected) in [
        (-1_i128, -1_i128),
        (40000, -25536),
        (-40000, 25536),
        (65536, 0),
        (32768, -32768),
        (-32768, -32768),
    ] {
        assert_eq!(
            execute(
                &narrow("i32", "i16"),
                &[integer(
                    IntegerSign::Signed,
                    32,
                    IntegerValue::Signed(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Signed,
                16,
                IntegerValue::Signed(expected)
            )),
            "i32 -> i16 in Wrapping of {input}"
        );
    }
}

#[test]
fn a_same_width_sign_change_executes_the_modular_image() {
    // No narrowing carrier exists at all on a same-width pair: the halves
    // assemble in the destination width itself.
    for (input, expected) in [
        (255_u128, -1_i128),
        (200, -56),
        (128, -128),
        (127, 127),
        (0, 0),
    ] {
        assert_eq!(
            execute(
                &narrow("u8", "i8"),
                &[integer(
                    IntegerSign::Unsigned,
                    8,
                    IntegerValue::Unsigned(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Signed,
                8,
                IntegerValue::Signed(expected)
            )),
            "u8 -> i8 in Wrapping of {input}"
        );
    }
    for (input, expected) in [(-1_i128, 255_u128), (-128, 128), (127, 127), (-2, 254)] {
        assert_eq!(
            execute(
                &narrow("i8", "u8"),
                &[integer(IntegerSign::Signed, 8, IntegerValue::Signed(input))],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Unsigned,
                8,
                IntegerValue::Unsigned(expected)
            )),
            "i8 -> u8 in Wrapping of {input}"
        );
    }
    for (input, expected) in [(65535_u128, -1_i128), (40000, -25536), (32767, 32767)] {
        assert_eq!(
            execute(
                &narrow("u16", "i16"),
                &[integer(
                    IntegerSign::Unsigned,
                    16,
                    IntegerValue::Unsigned(input)
                )],
            ),
            TerminalExecutionResult::Scalar(integer(
                IntegerSign::Signed,
                16,
                IntegerValue::Signed(expected)
            )),
            "u16 -> i16 in Wrapping of {input}"
        );
    }
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
