//! Which arithmetic-policy conversions reach a Unit plan and which stop.
//!
//! `source/library/core/numeric_conversion.omg` publishes one named machine per
//! conversion policy, and the `tests/omega/pass/core/numeric_*` canaries call
//! them. Three of those shapes still stop before native execution, and the stop
//! is not a Unit control-builder restriction: two are rejected by this crate's
//! own explicit policy-realization limits, and the Trapping shift has no
//! checked scalar expression at all, so the ordinary statement sequence finds
//! nothing to plan. Boolean-to-integer conversion now composes: a dedicated
//! `BooleanToInteger` checked computation owns the authored cast occurrence
//! and lands through the ordinary conditional selection of 0 and 1.
//!
//! Each rejection is paired with the admitted neighbour that differs in one
//! coordinate, so a repair has to move the actual boundary rather than widen a
//! recognizer. The Trapping controls stay until Terminal Psi carries
//! executable Trapping operations — [structural
//! predicates](../../../../../wiki/spec/terminal-psi/structural_predicates.md)
//! requires them to carry "their primitive denotation and path-conditioned
//! crash site", so a producer may not expand one into a guard and a `Crash`
//! terminator — and the signed-modular control stays until signed narrowing
//! or same-width sign reinterpretation has a runtime realization; widening
//! conversions are already value-preserving and need no such operator.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

fn lowering_error(source: &str) -> checked_trees_to_lowered_psi::LoweringError {
    let checked = checked(source);
    checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect_err("this policy conversion has no native realization yet")
}

fn lowers(source: &str) {
    let checked = checked(source);
    checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
        .expect("this conversion composes from admitted operations");
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

/// `narrow_u16_to_u8_trapping` and every other `*_trapping` conversion end in
/// `(value as u8 in Trapping) as u8`. The checked stage retains the cast as
/// `IntegerTrappingCast`; this crate has no Terminal operation to carry its
/// crash site.
#[test]
fn a_trapping_conversion_has_no_runtime_policy_realization() {
    let error = lowering_error(
        r#"
        data Main {}
        machine narrow(value: u16) -> u8 { (value as u8 in Trapping) as u8 }
        machine Main::main(value: u16) { let narrowed: u8 = narrow(value); }
    "#,
    );
    assert_eq!(
        error,
        checked_trees_to_lowered_psi::LoweringError::Unsupported(
            "checked trapping conversion requires runtime policy realization"
        )
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
/// change sign at or below the source width. Unsigned remainder has the
/// destination's modular image; a signed carrier on either side does not, so
/// this crate rejects it rather than substituting a different value.
#[test]
fn a_signed_wrapping_conversion_has_no_runtime_policy_realization() {
    let error = lowering_error(
        r#"
        data Main {}
        machine narrow(value: u8) -> i8 { (value as i8 in Wrapping) as i8 }
        machine Main::main(value: u8) { let narrowed: i8 = narrow(value); }
    "#,
    );
    assert_eq!(
        error,
        checked_trees_to_lowered_psi::LoweringError::Unsupported(
            "signed wrapping conversion requires runtime policy realization"
        )
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
    let checked = checked(
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
    checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
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
        let checked = checked(&format!(
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
        checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main")
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
/// u32 in Trapping)`, whose out-of-range count is the library's trap. No
/// checked integer binary kind pairs a shift with Trapping, so that
/// initializer has no value fact either.
#[test]
fn a_trapping_shift_leaves_its_initializer_without_a_value_fact() {
    let (machine, omission) = unit_plan_omission(
        r#"
        data Main {}
        machine trap_if(count: u32) {
            let probe: u8 in Trapping = (1 as u8 in Trapping) << (count as u32 in Trapping);
        }
        machine Main::main(count: u32) { trap_if(count); }
    "#,
    );
    assert_eq!(machine, "Main::main");
    assert_eq!(
        omission,
        "`Main::main` calls `trap_if`, which has no plan; `trap_if` has no admitted body \
         (local construction stopped at statement sequence: local data: scalar local: \
         pure initializer, statement 0)"
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
