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
//! - Saturating conversion: unsigned-to-unsigned narrowings now carry
//!   `IntegerSaturatingCast` and lower through the same modular-bound shape as
//!   the wrapping neighbour (`value - (value sat_sub target_max)`, then a
//!   remainder that supplies the exact-cast bound). A saturating cast with a
//!   signed carrier still has no checked cast kind — a two-sided clamp needs a
//!   comparison that unsigned saturating subtraction cannot spell — so those
//!   spellings keep the no-value-fact boundary.
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

fn lowering_error(source: &str) -> checked_trees_to_lowered_psi::LoweringError {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect_err("this policy conversion has no native realization yet")
}

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

/// The omission chain naming the machine whose own body stopped, with the
/// ordinary builder's last phase and the statement it was planning.
fn unit_plan_omission(source: &str) -> (String, String) {
    let checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan {
        machine,
        omission,
        ..
    } = lowering_error(source)
    else {
        panic!("a missing checked value fact leaves the Unit closure without a plan");
    };
    (
        machine,
        omission.expect("the omission roster names the machine"),
    )
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

/// The same narrowing under Wrapping composes from exact remainder and an
/// exact cast, so only the policy separates the admitted case from the stop.
#[test]
fn an_unsigned_wrapping_conversion_composes_from_exact_operations() {
    lowers(
        r#"
        data Main {}
        machine narrow(value: u16) -> u8 { (value as u8 in Wrapping) as u8 }
        machine Main::main(value: u16) { let narrowed: u8 = narrow(value); }
    "#,
    );
}

/// `narrow_u8_to_i8_wrapping`, `narrow_i16_to_u8_wrapping` and their siblings
/// change sign at or below the source width. A signed destination receives
/// the modular image assembled from its halves — the residue's low `B - 1`
/// bits and its sign-position bit are masked separately, landed by exact
/// casts whose operands carry the masks' bounds, and reassembled — and a
/// same-width pair extracts those halves straight off the source carrier.
/// `signed_wrapping_conversion_values.rs` executes the answers.
#[test]
fn a_signed_wrapping_conversion_composes_from_its_halves() {
    lowers(
        r#"
        data Main {}
        machine narrow(value: u8) -> i8 { (value as i8 in Wrapping) as i8 }
        machine Main::main(value: u8) { let narrowed: i8 = narrow(value); }
    "#,
    );
}

/// A Wrapping conversion whose target already contains every source value is
/// value-preserving, so the modular image is the widened operand itself.
/// Signed sources and signed targets therefore need no signed modular
/// operator: `i8 -> i16` and `u8 -> i16` compose as `IntegerWiden`.
#[test]
fn a_signed_widening_wrapping_conversion_composes_as_widening() {
    lowers(
        r#"
        data Main {}
        machine widen(value: i8) -> i16 { (value as i16 in Wrapping) as i16 }
        machine Main::main(value: i8) { let widened: i16 = widen(value); }
    "#,
    );
    lowers(
        r#"
        data Main {}
        machine widen(value: u8) -> i16 { (value as i16 in Wrapping) as i16 }
        machine Main::main(value: u8) { let widened: i16 = widen(value); }
    "#,
    );
}

/// Crossing the same sign boundary under Exact needs only a range and its
/// representability proof, so the signed carrier is not itself the limit.
#[test]
fn an_exact_conversion_crosses_the_sign_boundary_with_a_declared_range() {
    lowers(
        r#"
        data Main {}
        machine narrow(value: u8 [0..=127]) -> i8
        requires
            value <= 127;
        {
            value as i8
        }
        machine Main::main(value: u8 [0..=127])
        requires
            value <= 127;
        {
            let narrowed: i8 = narrow(value);
        }
    "#,
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
                checked_trees::CheckedScalarComputationKind::BooleanToInteger { .. }
            )
        })
        .expect("the Boolean-to-integer initializer is a computation root");
    let node = plans.nodes.get(root.root);
    assert_eq!(node.primitive_type, typed_trees::types::PrimitiveType::U32);
    let checked_trees::CheckedScalarComputationKind::BooleanToInteger { operand, .. } = node.kind
    else {
        unreachable!()
    };
    assert_eq!(
        plans.nodes.get(operand).primitive_type,
        typed_trees::types::PrimitiveType::Bool
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

/// The identical initializer over an integer source composes through
/// `IntegerWrappingCast`, so the two carriers now share one working surface.
#[test]
fn an_integer_wrapping_conversion_initializer_reaches_a_plan() {
    lowers(
        r#"
        data Main {}
        machine trap_if(invalid: u8) {
            let invalid_value: u32 in Wrapping = invalid as u32 in Wrapping;
        }
        machine Main::main(invalid: u8) { trap_if(invalid); }
    "#,
    );
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

/// The same shift under Wrapping reaches a plan through
/// `WrappingIntegerShiftLeft`, including the anonymous `1` landing at the
/// shifted carrier, so only the Trapping count law is missing.
#[test]
fn a_wrapping_shift_initializer_reaches_a_plan() {
    lowers(
        r#"
        data Main {}
        machine trap_if(count: u32) {
            let probe: u8 in Wrapping = (1 as u8 in Wrapping) << count;
        }
        machine Main::main(count: u32) { trap_if(count); }
    "#,
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
        scalar_expressions_contain(&checked, |expression| matches!(
            expression,
            checked_trees::CheckedScalarExpression::IntegerTrappingCast { .. }
        )),
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
            checked_trees::CheckedScalarExpression::IntegerWiden { .. }
        )),
        "the total Trapping conversion folds to IntegerWiden at checked stage"
    );
    assert!(
        !scalar_expressions_contain(&checked, |expression| matches!(
            expression,
            checked_trees::CheckedScalarExpression::IntegerTrappingCast { .. }
        )),
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

/// `narrow_*_saturating` in the library clamps through an explicit
/// `transition`; an unsigned narrowing `in Saturating` cast now carries
/// `IntegerSaturatingCast`, whose lowering is the wrapping neighbour's
/// remainder-bound shape around `min(value, target_max)` — spelled
/// `value - (value sat_sub target_max)` since unsigned saturating
/// subtraction floors at zero. Saturating `+` on the same carriers already
/// lowers through `SaturatingIntegerAdd`. A signed carrier still has no
/// checked cast kind: a two-sided clamp needs a comparison the unsigned
/// saturating-subtraction spelling cannot express, so that boundary stays.
#[test]
fn a_saturating_conversion_composes_on_unsigned_narrowing() {
    lowers(
        r#"
        data Main {}
        machine narrow(value: u16) {
            let narrowed: u8 in Saturating = (value as u8 in Saturating) as u8;
        }
        machine Main::main(value: u16) { narrow(value); }
    "#,
    );
    lowers(
        r#"
        data Main {}
        machine narrow(value: u16) -> u8 { (value as u8 in Saturating) as u8 }
        machine Main::main(value: u16) { let narrowed: u8 = narrow(value); }
    "#,
    );
    let (machine, omission) = unit_plan_omission(
        r#"
        data Main {}
        machine narrow(value: i16) {
            let narrowed: i8 in Saturating = (value as i8 in Saturating) as i8;
        }
        machine Main::main(value: i16) { narrow(value); }
    "#,
    );
    assert_eq!(machine, "Main::main");
    assert_eq!(
        omission,
        "`Main::main` calls `narrow`, which has no plan; `narrow` has no admitted body \
         (local construction stopped at statement sequence: local data: scalar local: \
         pure initializer, statement 0)"
    );
    lowers(
        r#"
        data Main {}
        machine sat_add(left: u8, right: u8) {
            let sum: u8 in Saturating = (left as u8 in Saturating) + (right as u8 in Saturating);
        }
        machine Main::main(left: u8, right: u8) { sat_add(left, right); }
    "#,
    );
}
