//! Which arithmetic-policy conversions reach a Unit plan and which stop.
//!
//! `source/library/core/numeric_conversion.omg` publishes one named machine per
//! conversion policy, and the `tests/omega/pass/core/numeric_*` canaries call
//! them. Boolean-to-integer conversion composes: a dedicated
//! `BooleanToInteger` checked computation owns the authored cast occurrence
//! and lands through the ordinary conditional selection of 0 and 1.
//!
//! The realization frontier, pinned below at the stage where each case lands
//! or stops:
//!
//! - Trapping conversion, shifts, and arithmetic: each lowers to one
//!   `TrappingInteger` Terminal operation that is its own `Trap` crash site
//!   (`trapping_operation_sites.rs` executes the answers and the rejection
//!   controls). A Trapping cast whose target contains every source value
//!   still folds to `IntegerWiden` or the operand itself, so no trap
//!   operation exists where no trap can fire. The inferred body that owns a
//!   Trapping operation publishes the unconditional `Trap` route, and a
//!   private caller inherits it.
//! - Signed modular conversion: `IntegerWrappingCast` is retained for any
//!   fixed-integer pair, and lowering composes every one. A narrowing pair
//!   whose destination is unsigned masks inside the source carrier — `operand
//!   & (2^B - 1)` is the modular image already inside the destination, and
//!   the bound machinery carries an interval through the mask, so the exact
//!   cast that receives it discharges. A SIGNED destination receives the same
//!   image assembled from its halves — `low | (bit <<% (B-1))` — where each
//!   half's exact cast discharges on the bound of the mask that produced it;
//!   a signed source lands its residue on the `u_B` carrier first, since an
//!   `i_C -> i_B` cast has no native carrier. A same-width pair extracts the
//!   halves straight off the source carrier, and a signed source reaching
//!   `u64` — the one unsigned destination with no wider signed carrier —
//!   splits at the `i64` sign bit before its halves cross.
//!   `signed_wrapping_conversion_values.rs` executes those answers rather
//!   than asserting the composition.
//! - Saturating conversion: every fixed-integer pair now carries
//!   `IntegerSaturatingCast` and lowers by clamping on the source carrier,
//!   then letting the wrapping conversion carry the clamped value across —
//!   an operand already inside the destination range is its own modular
//!   image. Unsigned carriers keep `value -% (value sat_sub target_max)`;
//!   signed carriers read `max(0, x)` off the arithmetic sign mask
//!   `x & !(x >>% (C - 1))`, so the two-sided clamp needs no comparison —
//!   and a signed saturating subtraction is deliberately NOT the unsigned
//!   clamp identity: it floors at the carrier minimum, so the nonnegative
//!   part is taken off the distance instead. Executed answers live below in
//!   the saturating conversion tests.
//!
//! Each rejection is paired with the admitted neighbour that differs in one
//! coordinate, so a repair has to move the actual boundary rather than widen a
//! recognizer. [Structural
//! predicates](../../../../../wiki/spec/terminal-psi/structural_predicates.md)
//! require Trapping operations to carry "their primitive denotation and
//! path-conditioned crash site", so the Trapping controls check that the
//! operation survives as itself — never expanded into a guard and a `Crash`
//! terminator, and never weakened into its Wrapping or Saturating sibling.
//! The signed-modular pairs compose in every sign and width combination, so
//! their controls live with the executed answers in
//! `signed_wrapping_conversion_values.rs`.

use checked_trees_to_lowered_psi::TerminalMachineSelection;

fn lowers(source: &str) {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("this conversion composes from admitted operations");
}

/// Whether any retained scalar expression or computation node contains a
/// `CheckedScalarExpression` satisfying `predicate`, descending through
/// operand positions so nested casts still count.
fn scalar_expressions_contain(
    checked: &checked_trees::CheckedTrees,
    predicate: impl Fn(&checked_trees::CheckedScalarExpression) -> bool,
) -> bool {
    fn contains(
        expression: &checked_trees::CheckedScalarExpression,
        predicate: &dyn Fn(&checked_trees::CheckedScalarExpression) -> bool,
    ) -> bool {
        use checked_trees::CheckedScalarExpression as Expression;
        if predicate(expression) {
            return true;
        }
        match expression {
            Expression::IntegerBinary { left, right, .. } => {
                contains(left, predicate) || contains(right, predicate)
            }
            Expression::IntegerBitwiseNot { operand, .. }
            | Expression::IntegerWiden { operand, .. }
            | Expression::IntegerExactCast { operand, .. }
            | Expression::IntegerWrappingCast { operand, .. }
            | Expression::IntegerSaturatingCast { operand, .. }
            | Expression::IntegerTrappingCast { operand, .. }
            | Expression::StructuralParameterIndexedRead { index: operand, .. } => {
                contains(operand, predicate)
            }
            Expression::Boolean(boolean) => {
                fn boolean_contains(
                    expression: &checked_trees::CheckedBooleanExpression,
                    predicate: &dyn Fn(&checked_trees::CheckedScalarExpression) -> bool,
                ) -> bool {
                    use checked_trees::CheckedBooleanExpression as Boolean;
                    match expression {
                        Boolean::Not(operand) => boolean_contains(operand, predicate),
                        Boolean::Equal { left, right }
                        | Boolean::And { left, right }
                        | Boolean::Or { left, right } => {
                            boolean_contains(left, predicate) || boolean_contains(right, predicate)
                        }
                        Boolean::IntegerComparison { left, right, .. } => {
                            contains(left, predicate) || contains(right, predicate)
                        }
                        _ => false,
                    }
                }
                boolean_contains(boolean, predicate)
            }
            _ => false,
        }
    }
    checked
        .facts
        .values
        .scalar_expressions
        .expressions
        .iter()
        .any(|entry| contains(&entry.expression, &predicate))
        || checked
            .facts
            .values
            .scalar_computations
            .nodes
            .iter()
            .any(|(_, node)| match &node.kind {
                checked_trees::CheckedScalarComputationKind::Apply { expression, .. }
                | checked_trees::CheckedScalarComputationKind::Value(expression) => {
                    contains(expression, &predicate)
                }
                _ => false,
            })
}

/// The operand keeps its authored Boolean computation: a selected comparison
/// or a call result converts through the same node, and a signed destination
/// lands the same 0/1 payload in its own carrier.
#[test]
fn a_boolean_integer_conversion_keeps_its_operands_evaluation() {
    for (operand, target) in [
        ("invalid == false", "u32"),
        ("invalid", "i8"),
        ("probe()", "u16"),
    ] {
        let checked = crate::front_end::checked_program(&format!(
            r#"
            data Main {{}}
            machine probe() -> bool {{ true }}
            machine trap_if(invalid: bool) {{
                let invalid_value: {target} = ({operand}) as {target};
            }}
            machine Main::main(invalid: bool) {{ trap_if(invalid); }}
        "#
        ));
        let plans = &checked.facts.values.scalar_computations;
        assert!(
            plans
                .roots
                .iter()
                .map(|(_, root)| root)
                .any(|root| matches!(
                    plans.nodes.get(root.root).kind,
                    checked_trees::CheckedScalarComputationKind::BooleanToInteger { .. }
                )),
            "{operand} as {target}: the conversion is a computation root"
        );
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Main::main"),
        )
        .unwrap_or_else(|error| panic!("{operand} as {target}: {error:?}"));
    }
}

/// The one-coordinate neighbour of the unsigned narrowing above: a signed
/// pair keeps the checked cast occurrence and lowers — the clamp is spelled
/// `value -% max(0, value sat_sub high)` then `value +% max(0, low -%
/// value)` on the source carrier, and the wrapping crossing lands it.
#[test]
fn a_saturating_conversion_composes_on_signed_narrowing() {
    let checked = crate::front_end::checked_program(
        r#"
        data Main {}
        machine narrow(value: i16) {
            let narrowed: i8 in Saturating = value as i8 in Saturating;
        }
        machine Main::main(value: i16) { narrow(value); }
    "#,
    );
    assert!(
        scalar_expressions_contain(&checked, |expression| matches!(
            expression,
            checked_trees::CheckedScalarExpression::IntegerSaturatingCast { .. }
        )),
        "the signed narrowing keeps its checked IntegerSaturatingCast occurrence"
    );
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("the signed saturating conversion composes from admitted operations");
}

/// Every remaining sign combination composes the same way — the clamp on the
/// source carrier, then the wrapping crossing: `i16 -> u8` floors at zero,
/// `u16 -> i8` ceilings at `i8`'s maximum, and same-width sign changes clamp
/// inside the shared width.
#[test]
fn a_saturating_conversion_composes_on_mixed_sign_pairs() {
    for (source, target) in [
        ("i16", "u8"),
        ("u16", "i8"),
        ("i8", "u8"),
        ("u8", "i8"),
        ("i8", "u16"),
        ("i16", "u64"),
        ("u64", "i64"),
        ("i64", "i8"),
    ] {
        lowers(&format!(
            r#"
            data Main {{}}
            machine narrow(value: {source}) -> {target} {{
                (value as {target} in Saturating) as {target}
            }}
            machine Main::main(value: {source}) {{ let narrowed: {target} = narrow(value); }}
        "#
        ));
    }
}

// --- Executed answers -------------------------------------------------------
//
// Lowering composing is not evidence of clamp values: these rows execute the
// serialized module through the terminal interpreter, so decode and proof
// verification stand between the composed spelling and the answer. The
// harness mirrors `signed_wrapping_conversion_values.rs`.

fn integer(
    sign: semantic_vocabulary::IntegerSign,
    bits: u16,
    value: semantic_vocabulary::IntegerValue,
) -> terminal_interpreter::TerminalScalarValue {
    terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: semantic_vocabulary::IntegerType::new(sign, bits)
            .expect("a fixed integer carrier"),
        value,
    }
}

fn execute(
    source: &str,
    arguments: &[terminal_interpreter::TerminalScalarValue],
) -> terminal_interpreter::TerminalExecutionResult {
    let (semantics, proof) = encoded_program(source);
    terminal_interpreter::interpret_terminal_artifact(
        &semantics,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        arguments,
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
}

fn encoded_program(source: &str) -> (Vec<u8>, Vec<u8>) {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let semantics =
        terminal_codec::encode_module(&lowered.semantic_module).expect("canonical semantic bytes");
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("canonical proof bytes");
    (semantics, proof)
}

#[test]
fn saturating_shifts_execute_signed_unsigned_and_mixed_count_boundaries() {
    use terminal_interpreter::TerminalExecutionResult;
    for bits in [8_u16, 16, 32, 64] {
        for signed in [false, true] {
            let prefix = if signed { "i" } else { "u" };
            let carrier = format!("{prefix}{bits}");
            // Counts deliberately retain a different carrier at narrow widths.
            for operator in ["<<", ">>"] {
                let source = format!(
                    "machine value(input: {carrier} in Saturating, count: u64) -> {carrier} \
                     requires count < {bits} {{ (input {operator} count) as {carrier} }}"
                );
                let (semantics, proof) = encoded_program(&source);
                let minimum = if signed { -(1_i128 << (bits - 1)) } else { 0 };
                let maximum = (1_i128 << (bits - u16::from(signed))) - 1;
                let inputs = if signed {
                    vec![minimum, minimum + 1, -1, 0, 1, maximum]
                } else {
                    vec![0, 1, 17, maximum - 1, maximum]
                };
                for input in inputs {
                    for count in [0_u16, 1, bits - 2, bits - 1] {
                        let expected = if operator == "<<" {
                            (input << count).clamp(minimum, maximum)
                        } else {
                            input >> count
                        };
                        let input = if signed {
                            signed_argument(bits, input)
                        } else {
                            unsigned_argument(bits, input as u128)
                        };
                        let expected = if signed {
                            signed_result(bits, expected)
                        } else {
                            unsigned_result(bits, expected as u128)
                        };
                        let result = terminal_interpreter::interpret_terminal_artifact(
                            &semantics,
                            &proof,
                            &proof_admission::AdmissionProfile::default(),
                            &[input, unsigned_argument(64, u128::from(count))],
                        )
                        .unwrap_or_else(|error| panic!("{source}, count {count}: {error:#?}"));
                        assert_eq!(
                            result,
                            TerminalExecutionResult::Scalar(expected),
                            "{source}, count {count}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn saturating_shift_evaluates_a_trapping_count_once_and_keeps_its_range_obligation() {
    use terminal_interpreter::{TerminalArtifactInterpretError, TerminalInterpretError};
    use terminal_psi::{OperationKind, TrappingIntegerPrimitive};
    let source = "machine value(input: i8 in Saturating, count: u64 in Trapping, extra: u64 in Trapping) -> i8 \
                  { (input << (((count + extra) as u64) & 7)) as i8 }";
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("value"),
    )
    .expect("saturating shift composes with a trapping count expression");
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let traps = operations
        .iter()
        .filter_map(|operation| match operation.kind {
            OperationKind::TrappingInteger { operation } => Some(operation.primitive()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        traps,
        [TrappingIntegerPrimitive::Add],
        "one authored count, one crash site"
    );
    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(
                operation.kind,
                OperationKind::ExactIntegerShiftRight { .. }
            ))
            .count(),
        1,
        "masked scaling must not erase the count-range obligation"
    );
    let semantics = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let run = |count, extra| {
        terminal_interpreter::interpret_terminal_artifact(
            &semantics,
            &proof,
            &proof_admission::AdmissionProfile::default(),
            &[
                signed_argument(8, 1),
                unsigned_argument(64, count),
                unsigned_argument(64, extra),
            ],
        )
    };
    assert_eq!(
        run(3, 4).unwrap(),
        terminal_interpreter::TerminalExecutionResult::Scalar(signed_result(8, 127))
    );
    let error =
        run(u128::from(u64::MAX), 1).expect_err("the count addition still traps before scaling");
    let TerminalArtifactInterpretError::Execution(TerminalInterpretError::Crash(crash)) = error
    else {
        panic!("expected the count operation's Trap, got {error:#?}");
    };
    assert_eq!(crash.cause, terminal_psi::CrashCause::Trap);
}

#[test]
fn saturating_shift_invalid_counts_still_reject() {
    for operator in ["<<", ">>"] {
        for count in ["-1", "8"] {
            let source = format!(
                "machine value(input: i8 in Saturating) -> i8 \
                 {{ (input {operator} ({count})) as i8 }}"
            );
            assert!(
                crate::front_end::checked_program_result(&source).is_err(),
                "{source}"
            );
        }
        let source = format!(
            "machine value(input: u8 in Saturating, count: u64) -> u8 \
             {{ (input {operator} count) as u8 }}"
        );
        assert!(
            crate::front_end::checked_program_result(&source).is_err(),
            "{source}"
        );
    }
}

fn saturating_narrow(source_type: &str, target: &str) -> String {
    format!(
        "machine value(input: {source_type}) -> {target} \
         {{ (input as {target} in Saturating) as {target} }}"
    )
}

fn signed_argument(bits: u16, input: i128) -> terminal_interpreter::TerminalScalarValue {
    integer(
        semantic_vocabulary::IntegerSign::Signed,
        bits,
        semantic_vocabulary::IntegerValue::Signed(input),
    )
}

fn signed_result(bits: u16, expected: i128) -> terminal_interpreter::TerminalScalarValue {
    integer(
        semantic_vocabulary::IntegerSign::Signed,
        bits,
        semantic_vocabulary::IntegerValue::Signed(expected),
    )
}

fn unsigned_argument(bits: u16, input: u128) -> terminal_interpreter::TerminalScalarValue {
    integer(
        semantic_vocabulary::IntegerSign::Unsigned,
        bits,
        semantic_vocabulary::IntegerValue::Unsigned(input),
    )
}

fn unsigned_result(bits: u16, expected: u128) -> terminal_interpreter::TerminalScalarValue {
    integer(
        semantic_vocabulary::IntegerSign::Unsigned,
        bits,
        semantic_vocabulary::IntegerValue::Unsigned(expected),
    )
}

/// Same-width sign changes clamp inside the shared width — `i8 -> u8` floors
/// at zero and `u8 -> i8` ceilings at 127 — with no wider carrier anywhere
/// in the composition.
#[test]
fn a_same_width_saturating_sign_change_executes_the_clamp() {
    for (input, expected) in [(-128_i128, 0_u128), (-1, 0), (0, 0), (127, 127)] {
        assert_eq!(
            execute(&saturating_narrow("i8", "u8"), &[signed_argument(8, input)]),
            terminal_interpreter::TerminalExecutionResult::Scalar(unsigned_result(8, expected)),
            "i8 -> u8 in Saturating of {input}"
        );
    }
    for (input, expected) in [(0_u128, 0_i128), (127, 127), (128, 127), (255, 127)] {
        assert_eq!(
            execute(
                &saturating_narrow("u8", "i8"),
                &[unsigned_argument(8, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(signed_result(8, expected)),
            "u8 -> i8 in Saturating of {input}"
        );
    }
}

/// The unsigned neighbour is unchanged: `u16 -> u8` still clamps through the
/// same saturating-subtraction spelling it already had.
#[test]
fn the_unsigned_saturating_neighbour_still_executes_the_clamp() {
    for (input, expected) in [(0_u128, 0_u128), (255, 255), (256, 255), (65535, 255)] {
        assert_eq!(
            execute(
                &saturating_narrow("u16", "u8"),
                &[unsigned_argument(16, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(unsigned_result(8, expected)),
            "u16 -> u8 in Saturating of {input}"
        );
    }
}
