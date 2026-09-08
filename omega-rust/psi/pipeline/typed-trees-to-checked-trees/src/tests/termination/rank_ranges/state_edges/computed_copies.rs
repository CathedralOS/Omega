use super::{prove, reject};

const ACYCLIC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/computed_rank_copies/main.omg"
));

const CYCLIC: &str = r#"
machine walk(remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition { _ -> prepare(remaining, remaining) }
    state prepare(left: u32 [0..=5], right: u32 [0..=5]) {
        transition left > 0 {
            true -> finish(left + (right - right) - 1)
            false -> left
        }
    }
    state finish(result: u32 [0..=5]) {
        transition result > 0 {
            true -> prepare(result - 1, result - 1)
            false -> result
        }
    }
}
"#;

#[test]
fn computed_rank_copies_do_not_depend_on_operand_order() {
    for expression in ["left + (right - right)", "right + (left - left)"] {
        prove(&ACYCLIC.replace("left + (right - right)", expression));
    }
    prove(
        &ACYCLIC
            .replace(
                "prepare(remaining, remaining)",
                "prepare(remaining, remaining, remaining)",
            )
            .replace(
                "right: u32 [0..=5]",
                "right: u32 [0..=5], extra: u32 [0..=5]",
            )
            .replace(
                "left + (right - right)",
                "left + (right - right) + (extra - extra)",
            ),
    );
}

#[test]
fn computed_rank_copies_reject_unequal_initial_arrivals_even_with_valid_ranges() {
    // Both actuals fit 0..=5. Only their required equality is missing.
    let source = ACYCLIC.replace("remaining: u32 [0..=5]", "remaining: u32 [1..=4]");
    for actuals in [
        "remaining, remaining + 1",
        "remaining + 1, remaining",
        "remaining, remaining - 1",
        "remaining - 1, remaining",
    ] {
        reject(&source.replace(
            "prepare(remaining, remaining)",
            &format!("prepare({actuals})"),
        ));
    }
}

#[test]
fn computed_rank_copies_reestablish_equality_and_descent_on_every_cycle_edge() {
    prove(CYCLIC);
    prove(
        &CYCLIC
            .replace("left + (right - right)", "right + (left - left)")
            .replace("transition left > 0", "transition right > 0"),
    );
    for actuals in ["result - 1, result", "result, result - 1"] {
        reject(&CYCLIC.replace(
            "prepare(result - 1, result - 1)",
            &format!("prepare({actuals})"),
        ));
    }
    // Equality and range membership cannot replace strict cyclic decrease.
    reject(&CYCLIC.replace(
        "finish(left + (right - right) - 1)",
        "finish(left + (right - right))",
    ));
    reject(&CYCLIC.replace("prepare(result - 1, result - 1)", "prepare(result, result)"));
}

#[test]
fn computed_rank_copies_require_immutable_parameters_and_checked_operator_meaning() {
    reject(&ACYCLIC.replace("right: u32", "mut right: u32"));
    reject(&format!(
        "operator + u32::custom(left: u32, right: u32) -> u32; {ACYCLIC}"
    ));
}

#[test]
fn computed_rank_arrival_uses_established_copy_equality() {
    let source = ACYCLIC.replace(
        "transition { _ -> finish(left + (right - right)) }",
        "transition right >= left { true -> finish(left + (right - left)) false -> left }",
    );
    // This proof requires equality, unlike the cancellation controls above.
    // The independent store-bound consumer does not yet transport this private
    // rank invariant; complete source-check success is covered separately.
    crate::checks::termination::check_machine_termination(&super::super::typed(&source))
        .expect("established copy equality proves the computed rank arrival");
}

#[test]
fn a_valid_parallel_arrival_cannot_hide_changed_rank_copies() {
    let source = ACYCLIC.replace(
        "remaining: u32 [0..=5]",
        "remaining: u32 [1..=4], enabled: bool",
    );
    for (first, second) in [
        ("remaining, remaining", "remaining, remaining + 1"),
        ("remaining, remaining + 1", "remaining, remaining"),
    ] {
        reject(&source.replace(
            "transition { _ -> prepare(remaining, remaining) }",
            &format!(
                "transition enabled {{ true -> prepare({first}) false -> prepare({second}) }}"
            ),
        ));
    }
}

#[test]
fn explicit_self_edges_cannot_publish_termination_guarantees() {
    let source = "machine spin(remaining: u32 [0..=5])
        terminates by remaining in 0..=5;
        -> u32 { transition { _ -> self } }";
    assert!(
        crate::checks::termination::check_machine_termination(&super::super::typed(source))
            .is_err(),
        "an unchanged self edge has no strict descent"
    );
    let unannotated = "machine spin() { transition { _ -> self } }";
    let program = super::super::typed(unannotated);
    assert_eq!(
        crate::infer_machine_termination_summary(&program, program.machines()[0].symbol),
        Some(language_semantics::TerminationGuarantee::NoGuarantee),
        "compile-time consumers must not infer acyclic termination for self loops"
    );
    super::super::lower_typed_trees(program)
        .expect("a productive loop without a termination promise remains valid");
    let caller = format!("{unannotated} machine caller() terminates; {{ spin(); }}");
    assert!(
        super::super::lower_typed_trees(super::super::typed(&caller)).is_err(),
        "a terminating caller cannot inherit a false guarantee from a self loop"
    );
}

#[test]
fn explicit_self_occurrences_cannot_hide_beside_descending_edges() {
    for source in [
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 { transition remaining > 0 {
             true -> walk(remaining - 1) false -> self
         } }",
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 { transition remaining == 0 {
             true -> self false -> walk(remaining - 1)
         } }",
        "machine walk(remaining: u32 [0..=5])
         terminates by remaining in 0..=5;
         -> u32 {
             transition { _ -> finish(remaining) }
             state finish(result: u32 [0..=5]) { transition { _ -> self } }
         }",
    ] {
        let diagnostics =
            crate::checks::termination::check_machine_termination(&super::super::typed(source))
                .expect_err("every self occurrence requires its own strict descent");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("cannot prove rank range")
                    || diagnostic
                        .message
                        .contains("cannot prove the `terminates by` ranking")
            }),
            "{source}\n{diagnostics:#?}"
        );
        assert!(super::super::lower_typed_trees(super::super::typed(source)).is_err());
    }
}
