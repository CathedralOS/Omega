use super::{lower_typed_trees, typed};

const ALTERNATING: &str = r#"
machine walk(remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition remaining > 2 {
        true -> first(remaining)
        false -> second(remaining)
    }
    state first(pending: u32 [0..=5]) {
        transition pending > 0 {
            true -> second(pending - 1)
            false -> pending
        }
    }
    state second(left: u32 [0..=5]) {
        transition left > 0 {
            true -> first(left - 1)
            false -> left
        }
    }
}
"#;

fn prove(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {source}\n{diagnostics:#?}"));
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("complete checking: {source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics =
        crate::checks::termination::check_machine_termination(&typed(source)).expect_err(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn alternating_named_states_reprove_rank_range_on_each_cyclic_edge() {
    prove(ALTERNATING);
}

#[test]
fn one_descending_edge_cannot_excuse_another_preserving_state_edge() {
    reject(&ALTERNATING.replace("second(pending - 1)", "second(pending)"));
    reject(&ALTERNATING.replace("first(left - 1)", "first(left)"));
    reject(&ALTERNATING.replace("first(left - 1)", "first(left - 2)"));
}

#[test]
fn acyclic_arrivals_preserve_the_range_without_requiring_descent() {
    prove(
        r#"
    machine walk(remaining: u32 [0..=5])
    terminates by remaining in 0..=5;
    -> u32 {
        transition { _ -> prepare(remaining) }
        state prepare(pending: u32 [0..=5]) {
            transition { _ -> finish(pending) }
        }
        state finish(result: u32 [0..=5]) { result }
    }
    "#,
    );
}

#[test]
fn an_identity_forwarding_chain_establishes_the_named_loop_mapping() {
    prove(
        r#"
    machine walk(remaining: u32 [0..=5])
    terminates by remaining in 0..=5;
    -> u32 {
        transition { _ -> prepare(remaining) }
        state prepare(pending: u32 [0..=5]) {
            transition { _ -> iterate(pending) }
        }
        state iterate(left: u32 [0..=5]) {
            transition left > 0 {
                true -> iterate(left - 1)
                false -> left
            }
        }
    }
    "#,
    );
}

#[test]
fn acyclic_computed_arrivals_need_membership_but_not_strict_decrease() {
    let source = r#"
    machine walk(remaining: u32 [0..=5])
    terminates by remaining in 0..=5;
    -> u32 {
        transition remaining > 2 {
            true -> prepare(remaining)
            false -> finish(remaining)
        }
        state prepare(pending: u32 [0..=5]) {
            transition pending < 5 {
                true -> finish(pending + 1)
                false -> pending
            }
        }
        state finish(result: u32 [0..=5]) { result }
    }
    "#;
    prove(source);
    reject(&source.replace("pending < 5", "pending <= 5"));
}

#[test]
fn cross_state_argument_ordinals_preserve_the_exact_view_bound() {
    let source = r#"
    machine climb(limit: u64, index: u64)
    requires index <= limit;
    terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
    -> u64 {
        transition index == 0 {
            true -> first(index, limit)
            false -> second(limit, index)
        }
        state first(cursor: u64, ceiling: u64) {
            transition cursor < ceiling {
                true -> second(ceiling, cursor + 1)
                false -> cursor
            }
        }
        state second(bound: u64, position: u64) {
            transition position < bound {
                true -> first(position + 1, bound)
                false -> position
            }
        }
    }
    "#;
    prove(source);
    reject(&source.replace(
        "second(ceiling, cursor + 1)",
        "second(ceiling + 1, cursor + 1)",
    ));
    reject(&source.replace("second(ceiling, cursor + 1)", "second(cursor + 1, ceiling)"));
    reject(&source.replace("first(position + 1, bound)", "first(position, bound)"));
}

#[test]
fn computed_arguments_do_not_invent_an_unestablished_state_mapping() {
    reject(&ALTERNATING.replace(
        "transition remaining > 2 {\n        true -> first(remaining)\n        false -> second(remaining)\n    }",
        "transition { _ -> first(remaining) }",
    ));
}

#[test]
fn cross_state_rank_proofs_do_not_reuse_written_parameter_values() {
    reject(&ALTERNATING.replace(
        "transition pending > 0",
        "pending = 5; transition pending > 0",
    ));
    reject(&ALTERNATING.replace("transition left > 0", "left = 5; transition left > 0"));
}

#[test]
fn a_late_conflicting_identity_arrival_cannot_reuse_a_processed_mapping() {
    reject(
        r#"
    machine walk(remaining: u32 [0..=5], other: u32 [0..=5])
    terminates by remaining in 0..=5;
    -> u32 {
        transition remaining > 2 {
            true -> first(remaining, other)
            false -> second(remaining, other)
        }
        state first(pending: u32 [0..=5], spare: u32 [0..=5]) {
            transition { _ -> finish(pending, spare) }
        }
        state second(left: u32 [0..=5], extra: u32 [0..=5]) {
            transition { _ -> finish(extra, left) }
        }
        state finish(result: u32 [0..=5], unused: u32 [0..=5]) { result }
    }
    "#,
    );
}

#[test]
fn every_parallel_edge_occurrence_still_requires_its_own_proof() {
    reject(
        &ALTERNATING
            .replace("true -> second(pending - 1)", "true -> second(pending)")
            .replace(
                "transition pending > 0 {",
                "transition pending > 1 { true -> second(pending - 1) } transition pending > 0 {",
            ),
    );
}
