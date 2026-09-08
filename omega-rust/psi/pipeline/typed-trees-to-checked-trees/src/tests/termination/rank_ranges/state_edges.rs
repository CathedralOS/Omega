use super::{lower_typed_trees, typed};

mod computed_copies;

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
fn computed_arrivals_transport_the_unique_authored_rank_subject() {
    let source = ALTERNATING.replace(
        "transition remaining > 2 {\n        true -> first(remaining)\n        false -> second(remaining)\n    }",
        "transition { _ -> first(remaining) }",
    );
    prove(&source);
    for argument in ["remaining + 0", "(remaining - 0) * 1"] {
        prove(&source.replace("first(remaining)", &format!("first({argument})")));
    }
    // Repeated occurrences are one dependency in the rank proof. General
    // exact-arithmetic checking independently owes each intermediate bound;
    // this assertion does not claim complete checking of those expressions.
    for argument in [
        "(2 * remaining) - remaining",
        "(remaining + remaining) - remaining",
    ] {
        crate::checks::termination::check_machine_termination(&typed(
            &source.replace("first(remaining)", &format!("first({argument})")),
        ))
        .expect("one current parameter despite repeated occurrences");
    }
    prove(&source.replace(
        "transition { _ -> first(remaining) }",
        "transition remaining > 0 { true -> first(remaining - 1) false -> remaining }",
    ));
    reject(&source.replace("first(remaining)", "first(remaining - 1)"));
    reject(&source.replace("second(pending - 1)", "second(pending)"));
    reject(&source.replace("second(pending - 1)", "second(pending - 2)"));
    reject(&format!(
        "operator + u32::add(left: u32, right: u32) -> u32; {}",
        source.replace("first(remaining)", "first(remaining + 0)")
    ));
}

#[test]
fn computed_increasing_arrivals_preserve_reordered_bound_slots() {
    let source = r#"
        machine climb(limit: u64, index: u64)
        requires index <= limit;
        terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1);
        -> u64 {
            transition index < limit {
                true -> first(index + 1, limit)
                false -> index
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
    reject(&source.replace("first(index + 1, limit)", "first(index + 1, limit + 1)"));
    reject(&source.replace(
        "second(ceiling, cursor + 1)",
        "second(ceiling + 1, cursor + 1)",
    ));
    reject(&source.replace("first(position + 1, bound)", "first(position, bound)"));
    reject(&format!(
        "operator + u64::add(left: u64, right: u64) -> u64; {source}"
    ));
}

#[test]
fn auxiliary_dependencies_do_not_replace_the_authored_rank_subject() {
    let source = ALTERNATING
        .replace("remaining: u32 [0..=5]", "remaining: u32 [0..=5], other: u32 [0..=5]")
        .replace("transition remaining > 2 {\n        true -> first(remaining)\n        false -> second(remaining)\n    }",
            "transition { _ -> first(remaining + (other - other)) }");
    prove(&source);
    reject(&source.replace("remaining + (other - other)", "0"));
    reject(&source.replace("remaining + (other - other)", "other"));
    reject(&source.replace("remaining + (other - other)", "remaining - other"));

    // Copying a root subject into two current slots does not make arithmetic
    // over both slots a single-current-parameter computation.
    reject(
        r#"
        machine walk(remaining: u32 [0..=5], payload: u32)
        terminates by remaining in 0..=5;
        -> u32 {
            transition { _ -> prepare(remaining, payload, payload) }
            state prepare(pending: u32, left: u32, right: u32) {
                transition { _ -> finish(pending, left + (right - right)) }
            }
            state finish(result: u32, spare: u32) { result }
        }
    "#,
    );
}

#[test]
fn computed_arrivals_use_the_authored_rank_among_auxiliary_parameters() {
    let source = r#"
        machine walk(step: u32 [1..=2], remaining: u32 [0..=5])
        terminates by remaining in 0..=5;
        -> u32 {
            transition remaining >= step {
                true -> iterate(step, remaining - step)
                false -> remaining
            }
            state iterate(stride: u32 [1..=2], pending: u32 [0..=5]) {
                transition pending >= stride {
                    true -> iterate(stride, pending - stride)
                    false -> pending
                }
            }
        }
    "#;
    prove(source);
    reject(&source.replace("1..=2", "0..=2"));
    reject(&source.replace("remaining >= step", "remaining > 0"));
    reject(&source.replace("pending >= stride", "pending > 0"));
    reject(&source.replace("pending - stride", "pending"));
    reject(&format!(
        "operator - u32::custom(left: u32, right: u32) -> u32; {source}"
    ));
}

#[test]
fn nested_auxiliary_computations_keep_one_current_rank_representative() {
    let source = r#"
        machine walk(remaining: u32 [0..=5], step: u32 [1..=1], extra: u32 [1..=1])
        terminates by remaining in 0..=5;
        -> u32 {
            transition remaining >= step + extra {
                true -> iterate(remaining - (step + extra), step, extra)
                false -> remaining
            }
            state iterate(pending: u32 [0..=5], stride: u32 [1..=1], offset: u32 [1..=1]) {
                transition pending >= stride + offset {
                    true -> iterate(pending - (stride + offset), stride, offset)
                    false -> pending
                }
            }
        }
    "#;
    prove(source);
    reject(&source.replace("pending - (stride + offset)", "pending"));
}

#[test]
fn computed_arrivals_transport_inductively_equal_rank_copies() {
    prove(
        r#"
        machine walk(remaining: u32 [0..=5])
        terminates by remaining in 0..=5;
        -> u32 {
            transition { _ -> prepare(remaining, remaining) }
            state prepare(left: u32 [0..=5], right: u32 [0..=5]) {
                transition { _ -> finish(left + (right - right)) }
            }
            state finish(result: u32 [0..=5]) { result }
        }
    "#,
    );
}

#[test]
fn computed_increasing_arrivals_keep_auxiliary_steps_separate_from_pinned_bounds() {
    let source = r#"
        machine climb(limit: u32 [0..=5], step: u32 [1..=1], index: u32 [0..=5])
        requires index <= limit;
        terminates by index -> Nat::IncreasingTo(limit) in 0..=5;
        -> u32 {
            transition index < limit {
                true -> iterate(index + step, step, limit)
                false -> index
            }
            state iterate(cursor: u32 [0..=5], stride: u32 [1..=1], ceiling: u32 [0..=5]) {
                transition cursor < ceiling {
                    true -> iterate(cursor + stride, stride, ceiling)
                    false -> cursor
                }
            }
        }
    "#;
    prove(source);
    reject(&source.replace(
        "index + step, step, limit",
        "index + step, step, limit + step",
    ));
    reject(&source.replace(
        "cursor + stride, stride, ceiling",
        "cursor + stride, stride, ceiling + stride",
    ));
    reject(&source.replace("cursor + stride", "cursor"));
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
    let source = r#"
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
    "#;
    reject(source);
    let computed = source
        .replace("first(remaining, other)", "first(remaining + 0, other + 0)")
        .replace(
            "second(remaining, other)",
            "second(remaining + 0, other + 0)",
        )
        .replace("finish(pending, spare)", "finish(pending + 0, spare + 0)")
        .replace("finish(extra, left)", "finish(extra + 0, left + 0)");
    reject(&computed);
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
