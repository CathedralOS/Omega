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
    checked: &typed_trees_to_checked_trees::checked_trees::CheckedTrees,
    predicate: impl Fn(&typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression) -> bool,
) -> bool {
    fn contains(
        expression: &typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression,
        predicate: &dyn Fn(
            &typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression,
        ) -> bool,
    ) -> bool {
        use typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression as Expression;
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
                    expression: &typed_trees_to_checked_trees::checked_trees::CheckedBooleanExpression,
                    predicate: &dyn Fn(
                        &typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression,
                    ) -> bool,
                ) -> bool {
                    use typed_trees_to_checked_trees::checked_trees::CheckedBooleanExpression as Boolean;
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
                typed_trees_to_checked_trees::checked_trees::CheckedScalarComputationKind::Apply { expression, .. }
                | typed_trees_to_checked_trees::checked_trees::CheckedScalarComputationKind::Value(expression) => {
                    contains(expression, &predicate)
                }
                _ => false,
            })
}

/// The Trapping primitives of the lowered program, in module order, and
/// whether every machine owning one publishes the unconditional `Trap`
/// route that covers its operation-level site.
fn trapping_primitives(source: &str) -> Vec<terminal_psi::TrappingIntegerPrimitive> {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    let mut primitives = Vec::new();
    for machine in &lowered.semantic_module.machines {
        let owned = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation.kind {
                terminal_psi::OperationKind::TrappingInteger { operation } => {
                    Some(operation.primitive())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if !owned.is_empty() {
            assert!(
                machine.contract.crash_routes.iter().any(|bucket| {
                    bucket.cause == terminal_psi::CrashCause::Trap
                        && bucket.alternatives == [terminal_psi::CrashRouteGuard::Truth]
                }),
                "a machine owning a Trapping operation publishes Trap: {source}"
            );
        }
        primitives.extend(owned);
    }
    primitives
}

/// `narrow_u16_to_u8_trapping` and every other `*_trapping` conversion end in
/// `(value as u8 in Trapping) as u8`. The checked `IntegerTrappingCast`
/// lowers to one Trapping `Convert` operation — its own crash site — and the
/// private caller inherits the callee's `Trap` route.
#[test]
fn a_trapping_conversion_lowers_to_its_own_trap_operation() {
    assert_eq!(
        trapping_primitives(
            r#"
        data Main {}
        machine narrow(value: u16) -> u8 { (value as u8 in Trapping) as u8 }
        machine Main::main(value: u16) { let narrowed: u8 = narrow(value); }
    "#,
        ),
        [terminal_psi::TrappingIntegerPrimitive::Convert]
    );
}

/// `numeric_conversion_trap_if` opens with `invalid as u32 in Wrapping`.
/// Checking admits a Boolean source for a fixed-integer target and confines it
/// to `0..=1`; the dedicated `BooleanToInteger` computation carries the
/// authored cast occurrence and lands through an ordinary conditional, so the
/// initializer reaches a plan without a new Terminal operation or a
/// cast-derived `Select` borrowing authored `&&`/`||` provenance.
#[test]
fn a_boolean_integer_conversion_lands_through_its_own_computation() {
    let checked = crate::front_end::checked_program(
        r#"
        data Main {}
        machine trap_if(invalid: bool) {
            let invalid_value: u32 in Wrapping = invalid as u32 in Wrapping;
        }
        machine Main::main(invalid: bool) { trap_if(invalid); }
    "#,
    );
    let plans = &checked.facts.values.scalar_computations;
    let root = plans
        .roots
        .iter()
        .map(|(_, root)| root)
        .find(|root| {
            matches!(
                plans.nodes.get(root.root).kind,
                typed_trees_to_checked_trees::checked_trees::CheckedScalarComputationKind::BooleanToInteger { .. }
            )
        })
        .expect("the Boolean-to-integer initializer is a computation root");
    let node = plans.nodes.get(root.root);
    assert_eq!(
        node.primitive_type,
        symbol_resolved_trees_to_typed_trees::typed_trees::types::PrimitiveType::U32
    );
    let typed_trees_to_checked_trees::checked_trees::CheckedScalarComputationKind::BooleanToInteger { operand, .. } = node.kind
    else {
        unreachable!()
    };
    assert_eq!(
        plans.nodes.get(operand).primitive_type,
        symbol_resolved_trees_to_typed_trees::typed_trees::types::PrimitiveType::Bool
    );
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("the conversion lowers through an ordinary conditional landing");
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
                    typed_trees_to_checked_trees::checked_trees::CheckedScalarComputationKind::BooleanToInteger { .. }
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

/// `numeric_conversion_trap_if` ends with `(1 as u8 in Trapping) << (count as
/// u32 in Trapping)`, whose out-of-range count is the library's trap. The
/// checked `TrappingShiftLeft` lowers to one Trapping `ShiftLeft` operation;
/// the same-width count cast folds away because it can never trap.
#[test]
fn a_trapping_shift_lowers_to_its_own_trap_operation() {
    assert_eq!(
        trapping_primitives(
            r#"
        data Main {}
        machine trap_if(count: u32) {
            let probe: u8 in Trapping = (1 as u8 in Trapping) << (count as u32 in Trapping);
        }
        machine Main::main(count: u32) { trap_if(count); }
    "#,
        ),
        [terminal_psi::TrappingIntegerPrimitive::ShiftLeft]
    );
}

/// The checked stage retains the narrowing Trapping cast occurrence as
/// `IntegerTrappingCast` inside its computation plan: checking carries the
/// policy, and lowering realizes it as its own operation.
#[test]
fn a_trapping_conversion_keeps_its_checked_cast_occurrence() {
    let checked = crate::front_end::checked_program(
        r#"
        data Main {}
        machine narrow(value: u16) -> u8 { (value as u8 in Trapping) as u8 }
        machine Main::main(value: u16) { let narrowed: u8 = narrow(value); }
    "#,
    );
    assert!(
        scalar_expressions_contain(&checked, |expression| {
            matches!(
            expression,
            typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression::IntegerTrappingCast { .. }
        )
        }),
        "the narrowing Trapping cast survives checking as IntegerTrappingCast"
    );
}

/// A Trapping cast whose target already contains every source value can never
/// fire: `construct_integer_cast` folds same-width spellings to the operand
/// and total widenings to `IntegerWiden` before the Trapping arm, so these
/// reach a plan without any trap machinery. Only the trap-reachable narrowing
/// above owns a Trapping operation.
#[test]
fn a_never_trapping_conversion_composes_without_a_trap_operation() {
    lowers(
        r#"
        data Main {}
        machine widen(value: u8) -> u16 { (value as u16 in Trapping) as u16 }
        machine Main::main(value: u8) -> u16 { widen(value) }
    "#,
    );
    lowers(
        r#"
        data Main {}
        machine widen(value: i8) -> i16 { (value as i16 in Trapping) as i16 }
        machine Main::main(value: i8) -> i16 { widen(value) }
    "#,
    );
    lowers(
        r#"
        data Main {}
        machine identity(value: u8) -> u8 { (value as u8 in Trapping) as u8 }
        machine Main::main(value: u8) -> u8 { identity(value) }
    "#,
    );
    let checked = crate::front_end::checked_program(
        r#"
        data Main {}
        machine widen(value: u8) -> u16 { (value as u16 in Trapping) as u16 }
        machine Main::main(value: u8) -> u16 { widen(value) }
    "#,
    );
    assert!(
        scalar_expressions_contain(&checked, |expression| matches!(
            expression,
            typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression::IntegerWiden { .. }
        )),
        "the total Trapping conversion folds to IntegerWiden at checked stage"
    );
    assert!(
        !scalar_expressions_contain(&checked, |expression| {
            matches!(
            expression,
            typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression::IntegerTrappingCast { .. }
        )
        }),
        "no Trapping conversion node remains once the trap is unreachable"
    );
}

/// The signed modular shapes all compose now, each through a different
/// residue spelling: a signed source narrowing to a signed destination lands
/// the residue on `u_B` and reassembles (`i16 -> i8`), a same-width
/// unsigned-to-signed pair extracts the halves off the source carrier
/// (`u16 -> i16`), and a same-width signed-to-unsigned pair borrows the next
/// signed carrier up for its mask (`i16 -> u16`).
#[test]
fn signed_wrapping_conversions_compose_on_any_carrier() {
    for (source, target) in [("i16", "i8"), ("u16", "i16"), ("i16", "u16")] {
        lowers(&format!(
            r#"
            data Main {{}}
            machine narrow(value: {source}) -> {target} {{
                (value as {target} in Wrapping) as {target}
            }}
            machine Main::main(value: {source}) {{ let narrowed: {target} = narrow(value); }}
        "#
        ));
    }
}

/// `checked_integer_binary_kind` selects a Trapping primitive for every
/// Trapping operator, so a Trapping `+` plans its value and lowers to one
/// Trapping `Add` operation (the same-carrier operand casts fold away). The
/// same add under Wrapping still lowers through `WrappingIntegerAdd`.
#[test]
fn a_trapping_arithmetic_operation_lowers_to_its_own_trap_operation() {
    assert_eq!(
        trapping_primitives(
            r#"
        data Main {}
        machine trap_add(left: u8, right: u8) {
            let sum: u8 in Trapping = (left as u8 in Trapping) + (right as u8 in Trapping);
        }
        machine Main::main(left: u8, right: u8) { trap_add(left, right); }
    "#,
        ),
        [terminal_psi::TrappingIntegerPrimitive::Add]
    );
    lowers(
        r#"
        data Main {}
        machine wrap_add(left: u8, right: u8) {
            let sum: u8 in Wrapping = (left as u8 in Wrapping) + (right as u8 in Wrapping);
        }
        machine Main::main(left: u8, right: u8) { wrap_add(left, right); }
    "#,
    );
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
        scalar_expressions_contain(&checked, |expression| {
            matches!(
            expression,
            typed_trees_to_checked_trees::checked_trees::CheckedScalarExpression::IntegerSaturatingCast { .. }
        )
        }),
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
    terminal_interpreter::interpret_terminal_artifact(
        &semantics,
        &proof,
        &proof_admission::AdmissionProfile::default(),
        arguments,
    )
    .unwrap_or_else(|error| panic!("{source}: {error:#?}"))
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

/// A signed narrowing saturates on BOTH ends: positive overflow lands on the
/// destination maximum, negative underflow on the minimum, and an in-range
/// operand — including the boundary values — crosses unchanged.
#[test]
fn a_signed_narrowing_saturating_conversion_executes_the_clamp() {
    for (input, expected) in [
        (-32768_i128, -128_i128),
        (-300, -128),
        (-129, -128),
        (-128, -128),
        (-1, -1),
        (0, 0),
        (127, 127),
        (128, 127),
        (300, 127),
        (32767, 127),
    ] {
        assert_eq!(
            execute(
                &saturating_narrow("i16", "i8"),
                &[signed_argument(16, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(signed_result(8, expected)),
            "i16 -> i8 in Saturating of {input}"
        );
    }
}

/// The same clamp is bit-width general: `i32 -> i16` saturates at the
/// sixteen-bit bounds, not the source's.
#[test]
fn a_wider_signed_narrowing_saturating_conversion_executes_the_clamp() {
    for (input, expected) in [
        (i128::from(i32::MIN), -32768_i128),
        (-40000, -32768),
        (-32769, -32768),
        (-32768, -32768),
        (-1, -1),
        (32767, 32767),
        (32768, 32767),
        (40000, 32767),
        (i128::from(i32::MAX), 32767),
    ] {
        assert_eq!(
            execute(
                &saturating_narrow("i32", "i16"),
                &[signed_argument(32, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(signed_result(16, expected)),
            "i32 -> i16 in Saturating of {input}"
        );
    }
}

/// A signed source reaching an unsigned destination floors at zero — the
/// coordinate where a truncation toward zero or a modular image would both
/// be wrong — and ceilings at `u_B`'s maximum.
#[test]
fn a_signed_to_unsigned_saturating_conversion_executes_the_clamp() {
    for (input, expected) in [
        (-32768_i128, 0_u128),
        (-300, 0),
        (-1, 0),
        (0, 0),
        (200, 200),
        (255, 255),
        (256, 255),
        (32767, 255),
    ] {
        assert_eq!(
            execute(
                &saturating_narrow("i16", "u8"),
                &[signed_argument(16, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(unsigned_result(8, expected)),
            "i16 -> u8 in Saturating of {input}"
        );
    }
}

/// An unsigned source reaching a signed destination only binds at the
/// ceiling: `u16 -> i8` clamps above `i8`'s maximum and keeps every value
/// below it.
#[test]
fn an_unsigned_to_signed_saturating_conversion_executes_the_clamp() {
    for (input, expected) in [
        (0_u128, 0_i128),
        (127, 127),
        (128, 127),
        (255, 127),
        (300, 127),
        (65535, 127),
    ] {
        assert_eq!(
            execute(
                &saturating_narrow("u16", "i8"),
                &[unsigned_argument(16, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(signed_result(8, expected)),
            "u16 -> i8 in Saturating of {input}"
        );
    }
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

/// A signed source reaching a wider unsigned destination floors at zero but
/// keeps every nonnegative value — only the lower bound of the clamp binds.
#[test]
fn a_signed_to_wider_unsigned_saturating_conversion_executes_the_floor() {
    for (input, expected) in [(-32768_i128, 0_u128), (-1, 0), (0, 0), (32767, 32767)] {
        assert_eq!(
            execute(
                &saturating_narrow("i16", "u64"),
                &[signed_argument(16, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(unsigned_result(64, expected)),
            "i16 -> u64 in Saturating of {input}"
        );
    }
}

/// The widest sign crossings: `i64 -> u8` exercises the two-sided signed
/// clamp, and `u64 -> i64` ceilings at the signed maximum.
#[test]
fn the_widest_saturating_crossings_execute_the_clamp() {
    for (input, expected) in [
        (i128::from(i64::MIN), 0_u128),
        (-1, 0),
        (300, 255),
        (i128::from(i64::MAX), 255),
    ] {
        assert_eq!(
            execute(
                &saturating_narrow("i64", "u8"),
                &[signed_argument(64, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(unsigned_result(8, expected)),
            "i64 -> u8 in Saturating of {input}"
        );
    }
    for (input, expected) in [
        (0_u128, 0_i128),
        (u128::from(i64::MAX as u64), i128::from(i64::MAX)),
        (u128::from(i64::MAX as u64) + 1, i128::from(i64::MAX)),
        (u128::from(u64::MAX), i128::from(i64::MAX)),
    ] {
        assert_eq!(
            execute(
                &saturating_narrow("u64", "i64"),
                &[unsigned_argument(64, input)]
            ),
            terminal_interpreter::TerminalExecutionResult::Scalar(signed_result(64, expected)),
            "u64 -> i64 in Saturating of {input}"
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
