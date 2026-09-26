use super::{constant, evaluate, integer_encoding};
use syntax_trees::expression::ExpressionNode;

#[test]
fn floating_calls_preserve_landings_control_and_concrete_admission() {
    for (carrier, literal, encoding) in [
        ("f32", "8388609.499999999999999", "float:f32:4b000001"),
        ("f32", "-0.0f32", "float:f32:80000000"),
        ("f64", "-0.0f64", "float:f64:8000000000000000"),
    ] {
        let text = format!(
            "machine retain(value: {carrier}) -> {carrier} {{ value }}
             machine guarded(value: {carrier}, ready: bool) -> {carrier}
             requires ready; {{ value }}
             const VALUE: {carrier} = match true {{
                 true -> guarded(retain({literal}), true),
                 false -> guarded(retain(2.0), false)
             }};"
        );
        let typed = super::evaluate_fully(&[("main.omg", &text)], &[]);
        assert_eq!(
            typed.const_declarations()[0]
                .canonical_value_encoding
                .as_deref(),
            Some(encoding)
        );
    }
}

#[test]
fn floating_calls_in_guarded_named_transitions_preserve_selection() {
    for (choice, expected) in [
        ("true", "float:f32:3fc00000"),
        ("false", "float:f32:40000000"),
    ] {
        let text = format!(
            "machine keep(value: f32) -> f32 {{ value }}
             machine choose(value: f32, flag: bool) -> f32 {{
                 transition {{ flag -> finish(keep(value)) _ -> finish(keep(2.0f32)) }}
                 state finish(value: f32) -> f32 {{ value }}
             }}
             const VALUE: f32 = choose(1.5f32, {choice});"
        );
        let typed = super::evaluate_fully(&[("main.omg", &text)], &[]);
        assert_eq!(
            typed.const_declarations()[0]
                .canonical_value_encoding
                .as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn floating_calls_retain_computed_constant_dependencies() {
    let typed = super::evaluate_fully(
        &[(
            "main.omg",
            "machine keep(value: f32) -> f32 { value }
         machine ready(value: f32) -> bool { true }
         const BASE: f32 = keep(1.5f32); const VALUE: f32 = keep(BASE);
         const READY: bool = ready(BASE);",
        )],
        &[],
    );
    assert!(
        typed
            .const_declarations()
            .iter()
            .filter(|declaration| typed.symbols.name(declaration.symbol) != "READY")
            .all(
                |declaration| declaration.canonical_value_encoding.as_deref()
                    == Some("float:f32:3fc00000")
            )
    );
    let base = typed
        .const_declarations()
        .iter()
        .find(|declaration| typed.symbols.name(declaration.symbol) == "BASE")
        .unwrap();
    let typed_trees::expression::ExpressionNode::Call(call) =
        typed.expression_table.expression(base.authored_initializer)
    else {
        panic!("base call");
    };
    let argument = typed.expression_table.expression_handles(call.arguments)[0];
    let mut changed = typed.clone();
    *changed.expression_table.expression_mut(argument) =
        typed_trees::expression::ExpressionNode::Float(
            numerics::literals::FloatLiteral::from_f64(2.0)
                .with_landing(numerics::literals::FloatFormat::F32),
        );
    assert!(super::super::validate_retained_invocations(&changed, None).is_err());
}

#[test]
fn floating_calls_reject_wrong_formats_even_in_unused_initializers_and_arms() {
    for source in [
        "machine keep(value: f32) -> f32 { value } const UNUSED: f32 = keep(1.0f64);",
        "machine keep(value: f64) -> f64 { value } const UNUSED: f32 = keep(1.0f64);",
        "machine keep(value: f32) -> f32 { value }
         const UNUSED: f32 = match true { true -> keep(1.0f32), false -> keep(2.0f64) };",
        "machine guarded(value: f32, ready: bool) -> f32 requires ready; { value }
         const UNUSED: f32 = guarded(1.0f32, false);",
        "machine keep(value: f32) -> f32 { value } const UNUSED: u64 = keep(1.0f32);",
    ] {
        let diagnostics =
            evaluate(source).expect_err("typed carrier and invocation checks remain required");
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains("interpreter")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn floating_call_replay_rejects_changed_argument_and_materialized_bits() {
    let typed = super::evaluate_fully(
        &[(
            "main.omg",
            "machine keep(value: f32) -> f32 { value } const VALUE: f32 = keep(1.5f32);",
        )],
        &[],
    );
    for change_argument in [true, false] {
        let mut changed = typed.clone();
        let declaration = &changed.const_declarations()[0];
        let expression = if change_argument {
            let typed_trees::expression::ExpressionNode::Call(call) = changed
                .expression_table
                .expression(declaration.authored_initializer)
            else {
                panic!("authored call");
            };
            changed.expression_table.expression_handles(call.arguments)[0]
        } else {
            declaration.materialized_initializer
        };
        *changed.expression_table.expression_mut(expression) =
            typed_trees::expression::ExpressionNode::Float(
                numerics::literals::FloatLiteral::from_f64(2.0)
                    .with_landing(numerics::literals::FloatFormat::F32),
            );
        let diagnostics = super::super::validate_retained_invocations(&changed, None)
            .expect_err("receiving replay must derive the original bits independently");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("drifted")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn concrete_invocation_admission_preserves_demand_and_scalar_snapshots() {
    let evaluated = evaluate(
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine forward(value: u64) -> u64 { divide(value) }
        machine identity(value: u64) -> u64 { value }
        machine gate(flag: bool) -> u64
        crashes Abort flag
        { transition { !flag -> 7 } crash Abort; }
        machine forward_gate(flag: bool) -> u64 { gate(flag) }
        machine full_width(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> value } crash Trap; }
        machine signed(value: i64) -> u64
        crashes Trap value < 0
        { transition { value >= 0 -> 9 } crash Trap; }
        const DIVIDED: u64 = forward(identity(1 + 1));
        const SELECTED: u64 = match false { true -> divide(0), false -> forward_gate(false) };
        const MAXIMUM: u64 = full_width(18446744073709551615);
        const SIGNED: u64 = signed(0);",
    )
    .expect("concrete arguments discharge guarded calls before demanded execution");
    integer_encoding(&evaluated, "DIVIDED", 5);
    integer_encoding(&evaluated, "SELECTED", 7);
    integer_encoding(&evaluated, "MAXIMUM", i128::from(u64::MAX));
    integer_encoding(&evaluated, "SIGNED", 9);
}

#[test]
fn concrete_invocation_rejects_undischarged_published_routes_before_interpretation() {
    for source in [
        "machine divide(value: u64) -> u64 crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        const UNUSED: u64 = divide(0);",
        "machine gate(flag: bool) -> u64 crashes Abort flag { 7 }
        const UNUSED: u64 = gate(true);",
        "machine gate(flag: bool) -> u64 crashes Trap flag { 7 }
        machine forward(flag: bool) -> u64 { gate(flag) }
        const UNUSED: u64 = forward(true);",
        "machine gate() -> u64 crashes Trap { 7 }
        const UNUSED: u64 = gate();",
        "machine signed(value: i64) -> u64 crashes Trap value < 0
        { transition { value >= 0 -> 9 } crash Trap; }
        const UNUSED: u64 = signed(-1);",
    ] {
        let diagnostics = evaluate(source).expect_err("published route must discharge");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
            "must reject through admission, not an interpreter failure: {diagnostics:?}"
        );
    }
    assert!(
        evaluate(
            "machine constrained(value: u64) -> u64 requires value > 0 { value }
        const UNUSED: u64 = constrained(0);"
        )
        .is_err(),
        "ordinary precondition floor remains required"
    );
}

#[test]
fn concrete_invocation_discharges_arithmetic_guards_from_checked_evidence() {
    // The forwarding body passes an arithmetic actual. The published guard
    // retains that expression over entry values, and the checked scalar
    // channel decides it once the outer concrete call supplies the binding;
    // interpretation success alone cannot establish this safety.
    let evaluated = evaluate(
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine forward(value: u64) -> u64 { divide(value - 1) }
        machine product(value: u64) -> u64 { divide(value * 2) }
        const DIVIDED: u64 = forward(3);
        const PRODUCT: u64 = product(1);",
    )
    .expect("concrete arguments discharge arithmetic guard evidence");
    integer_encoding(&evaluated, "DIVIDED", 5);
    integer_encoding(&evaluated, "PRODUCT", 5);
}

#[test]
fn concrete_invocation_rejects_arithmetic_guards_that_hold_or_trap() {
    for source in [
        // `1 - 1` lands on the guarded divisor: the Trap route is confirmed.
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine forward(value: u64) -> u64 { divide(value - 1) }
        const UNUSED: u64 = forward(1);",
        // The exact subtraction itself traps before the callee is reached;
        // the caller's own recorded cause keeps the invocation refused.
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine forward(value: u64) -> u64 { divide(value - 1) }
        const UNUSED: u64 = forward(0);",
        // An operand with no entry-value custody keeps no provable origin.
        // `hidden(0)` would interpret to a safe `divide(1)`, but a call
        // result is not checked evidence and the guarded Trap must survive.
        "machine divide(value: u64) -> u64
        crashes Trap value == 0
        { transition { value != 0 -> 10 / value } crash Trap; }
        machine hidden(value: u64) -> u64 { 1 }
        machine forward(value: u64) -> u64 { divide(hidden(value)) }
        const UNUSED: u64 = forward(0);",
    ] {
        let diagnostics = evaluate(source)
            .expect_err("arithmetic guard evidence must not be invented from interpretation");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
            "must reject through admission, not an interpreter failure: {diagnostics:?}"
        );
    }
}

#[test]
fn concrete_invocation_discharges_comparison_actuals_from_entry_provenance() {
    // A Boolean actual is entry provenance, not only scalar annotation:
    // `gate(value != 0)` retains the comparison over the caller's entry so the
    // concrete probe can decide it. The local `armed` spells the same origin
    // through an immutable initializer.
    let evaluated = evaluate(
        "machine gate(flag: bool) -> u64
        crashes Abort flag
        { transition { !flag -> 7 } crash Abort; }
        machine forward(value: u64) -> u64 { gate(value != 0) }
        machine inspect(value: u64) -> u64
        { let armed: bool = value != 0 && value < 9; gate(armed) }
        const DISCHARGED: u64 = forward(0);
        const INSPECTED: u64 = inspect(9);",
    )
    .expect("comparison actuals discharge guarded calls through entry provenance");
    integer_encoding(&evaluated, "DISCHARGED", 7);
    integer_encoding(&evaluated, "INSPECTED", 7);
    for source in [
        // `3 != 0` is true: the Abort route is confirmed, not merely unproven.
        "machine gate(flag: bool) -> u64
        crashes Abort flag
        { transition { !flag -> 7 } crash Abort; }
        machine forward(value: u64) -> u64 { gate(value != 0) }
        const UNUSED: u64 = forward(3);",
        // `10 != 0 && 10 > 9` is true through the local initializer: the
        // Abort route is confirmed rather than discharged.
        "machine gate(flag: bool) -> u64
        crashes Abort flag
        { transition { !flag -> 7 } crash Abort; }
        machine inspect(value: u64) -> u64
        { let armed: bool = value != 0 && value > 9; gate(armed) }
        const UNUSED: u64 = inspect(10);",
    ] {
        let diagnostics =
            evaluate(source).expect_err("a confirmed crash route must survive concrete discharge");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("retains unhandled")),
            "must reject through admission, not an interpreter failure: {diagnostics:?}"
        );
    }
}

#[test]
fn concrete_invocation_discharges_authored_requires_at_snapshot_arguments() {
    // The conservative closure fence rejects any authored `requires` because a
    // context-free evaluation cannot prove it. A concrete invocation is not
    // context-free: the snapshot probe re-runs ordinary contract checking on
    // the exact call, so `constrained(3)` proves `3 > 0` before interpretation.
    let evaluated = evaluate(
        "machine constrained(value: u64) -> u64 requires value > 0 { value }
        machine forward(value: u64) -> u64 requires value > 0 { constrained(value) }
        machine paired(left: u64, right: u64) -> u64
        requires left > 1
        requires right > 1
        { left }
        machine guarded(value: u64) -> u64
        requires value > 0
        crashes Trap value == 1
        { transition { value != 1 -> 10 / value } crash Trap; }
        const USED: u64 = constrained(3);
        const FORWARDED: u64 = forward(2);
        const PAIRED: u64 = paired(3, 4);
        const GUARDED: u64 = guarded(2);",
    )
    .expect("concrete arguments prove authored requires premises");
    integer_encoding(&evaluated, "USED", 3);
    integer_encoding(&evaluated, "FORWARDED", 2);
    integer_encoding(&evaluated, "PAIRED", 3);
    integer_encoding(&evaluated, "GUARDED", 5);
}

#[test]
fn concrete_invocation_rejects_requires_that_fail_at_snapshot_arguments() {
    for source in [
        // The premise itself is false at the concrete argument.
        "machine constrained(value: u64) -> u64 requires value > 0 { value }
        const UNUSED: u64 = constrained(0);",
        // The same premise is false through one forwarding hop: `forward`'s own
        // requires is what proves its internal `constrained(value)` call, and
        // that premise is equally false at the snapshot argument.
        "machine constrained(value: u64) -> u64 requires value > 0 { value }
        machine forward(value: u64) -> u64 requires value > 0 { constrained(value) }
        const UNUSED: u64 = forward(0);",
        // One of two premises fails.
        "machine paired(left: u64, right: u64) -> u64
        requires left > 1
        requires right > 1
        { left }
        const UNUSED: u64 = paired(3, 0);",
        // A requires premise must not be weakened just because a guarded route
        // also exists: the crash check would pass (`guarded(0)` cannot reach its
        // `value == 1` Trap) but the authored premise is still violated.
        "machine guarded(value: u64) -> u64
        requires value > 0
        crashes Trap value == 1
        { transition { value != 1 -> 10 / value } crash Trap; }
        const UNUSED: u64 = guarded(0);",
    ] {
        let diagnostics = evaluate(source)
            .expect_err("a requires premise false at the concrete arguments must reject");
        assert!(
            !diagnostics.is_empty(),
            "requires violation produced no diagnostic"
        );
    }
    assert!(
        evaluate(
            "machine guarded(value: u64) -> u64
            requires value > 0
            crashes Trap value == 1
            { transition { value != 1 -> 10 / value } crash Trap; }
            const UNUSED: u64 = guarded(1);"
        )
        .is_err(),
        "a satisfied requires premise never erases a retained crash route"
    );
}

#[test]
fn ordinary_machine_initializers_retain_calls_and_exact_scalar_composition() {
    let evaluated = evaluate(
        "machine size() -> u64 { 7 }
        machine identity(value: u64) -> u64 { value }
        machine enabled(value: bool) -> bool { value }
        const SIZE: u64 = identity(size()) * 2;
        const ENABLED: bool = enabled(SIZE == 14) && true;",
    )
    .expect("ordinary checked scalar calls");
    integer_encoding(&evaluated, "SIZE", 14);
    assert!(
        !constant(&evaluated, "SIZE")
            .normalization
            .as_ref()
            .unwrap()
            .call_selections
            .is_empty()
    );
    assert!(matches!(
        evaluated
            .expressions
            .expression(constant(&evaluated, "ENABLED").value),
        ExpressionNode::Boolean(true)
    ));
}

#[test]
fn machine_initializer_dependencies_are_ready_before_helper_execution() {
    let evaluated = evaluate(
        "machine size() -> u64 { BASE }
        const SIZE: u64 = size();
        const BASE: u64 = 7 / 2 * 2;",
    )
    .expect("helper dependency is evaluated before its invocation");
    integer_encoding(&evaluated, "SIZE", 7);
}

#[test]
fn machine_initializers_transport_full_width_integers_without_relanding() {
    let evaluated = evaluate(
        "machine identity(value: u64) -> u64 { value }
        const MAXIMUM: u64 = identity(18446744073709551615);",
    )
    .expect("full-width unsigned interpreter snapshot");
    integer_encoding(&evaluated, "MAXIMUM", i128::from(u64::MAX));
    for source in [
        "machine narrow() -> u8 { 7 } const VALUE: u64 = narrow();",
        "machine identity(value: u64) -> u64 { value } const VALUE: u64 = identity(7u8);",
        "machine ignore(value: u8) -> bool { true } const VALUE: bool = false && ignore(256);",
        "machine ignore(value: u8) -> bool { true } const VALUE: bool = false && ignore(7 / 2);",
    ] {
        assert!(
            evaluate(source).is_err(),
            "invalid source argument/carrier accepted: {source}"
        );
    }
}

#[test]
fn machine_initializer_calls_compose_in_nominal_and_array_leaves() {
    let evaluated = evaluate(
        "data Config [copy] { size: u64; enabled: bool; }
        machine size() -> u64 { 7 }
        machine enabled() -> bool { true }
        const CONFIG: Config = Config { size: size() * 2, enabled: enabled() };
        const SIZES: [u64; 2] = [size(), size() * 2];",
    )
    .expect("ordinary calls preserve aggregate leaf custody");
    assert!(constant(&evaluated, "CONFIG").normalization.is_some());
    assert!(constant(&evaluated, "SIZES").normalization.is_some());
}
