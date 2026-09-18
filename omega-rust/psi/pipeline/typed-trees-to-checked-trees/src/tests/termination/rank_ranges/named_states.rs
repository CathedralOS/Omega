use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = r#"
machine walk(remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition { _ -> iterate(remaining) }
    state iterate(pending: u32 [0..=5]) {
        transition pending > 0 {
            true -> iterate(pending - 1)
            false -> pending
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
fn named_state_range_follows_the_exact_renamed_arrival_parameter() {
    prove(COUNTDOWN);
}

#[test]
fn named_state_ranges_reject_non_decreasing_or_out_of_range_backedges() {
    for argument in ["pending", "pending + 1", "pending - 2"] {
        reject(&COUNTDOWN.replace("iterate(pending - 1)", &format!("iterate({argument})")));
    }
}

#[test]
fn named_state_range_does_not_invent_an_entry_correspondence() {
    reject(&COUNTDOWN.replace("iterate(remaining)", "iterate(6)"));
    reject(&COUNTDOWN.replace(
        "_ -> iterate(remaining)",
        "remaining == 0 -> iterate(6) _ -> iterate(remaining)",
    ));
}

#[test]
fn named_state_range_rejects_intervening_parameter_writes() {
    reject(&COUNTDOWN.replace(
        "transition pending > 0",
        "pending = 4; transition pending > 0",
    ));
    reject(&COUNTDOWN.replace(
        "transition { _ -> iterate",
        "remaining = 4; transition { _ -> iterate",
    ));
}

#[test]
fn named_state_rank_and_endpoint_follow_reordered_arguments() {
    let source = r#"
    machine climb(limit: u64, index: u64)
    requires index <= limit;
    terminates by index -> Nat::IncreasingTo(limit) in 0..=limit;
    -> u64 {
        transition { _ -> iterate(index, limit) }
        state iterate(cursor: u64, ceiling: u64) {
            transition cursor < ceiling {
                true -> iterate(cursor + 1, ceiling)
                false -> cursor
            }
        }
    }
    "#;
    prove(source);
    reject(&source.replace(
        "iterate(cursor + 1, ceiling)",
        "iterate(cursor + 1, ceiling + 1)",
    ));
    reject(&source.replace("iterate(cursor + 1, ceiling)", "iterate(cursor, ceiling)"));
    // Swapping the initial actuals makes the looping guard impossible under
    // the transported invariant. This still terminates, regardless of names.
    prove(&source.replace("iterate(index, limit)", "iterate(limit, index)"));
    // Distinct incoming permutations cannot share one assumed correspondence.
    reject(&source.replace(
        "_ -> iterate(index, limit)",
        "index == 0 -> iterate(limit, index) _ -> iterate(index, limit)",
    ));
}

const DISTANCE: &str = r#"
machine walk(lower: u32 [0..=5], upper: u32 [5..=10])
requires lower <= upper;
terminates by (lower, upper) -> Nat::BoundedDistance in 0..=10;
-> u32 {
    transition lower < upper {
        true -> step(lower + 1, upper)
        false -> lower
    }
    state step(left: u32 [0..=10], right: u32 [0..=10]) {
        transition left < right {
            true -> step(left + 1, right)
            false -> left
        }
    }
}
"#;

#[test]
fn distance_named_state_arrivals_claim_both_ranked_roles() {
    prove(DISTANCE);
    // A computed actual carrying copies of BOTH distance subjects claims the
    // entry its earliest dependency names; the edge judgment then reproves
    // membership and descent for that exact reading.
    prove(&DISTANCE.replace(
        "step(left + 1, right)",
        "step(left + (right - right) + 1, right)",
    ));
    prove(&DISTANCE.replace(
        "step(left + 1, right)",
        "step(left, right + (left - left) - 1)",
    ));
    // A mix that does not cancel keeps its named role's range reading, which
    // no longer fits the destination's distance.
    reject(&DISTANCE.replace("step(left + 1, right)", "step(left + right, right)"));
    // Diverging copies of a required distance subject keep their equality
    // obligation: the upper role cannot silently become a second lower copy.
    reject(&DISTANCE.replace("step(left + 1, right)", "step(left, left + 1)"));
}

const NESTED_RECORD: &str = r#"
data Inner { remaining: u64 [0..=5]; }
data Countdown { label: u64; inner: Inner; }
data Pair { left: Countdown; tag: u64; }
measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.inner.remaining }

machine walk(countdown: Countdown, ceiling: u64 [0..=5])
requires countdown.inner.remaining <= ceiling;
terminates by countdown -> Countdown::Remaining in 0..=ceiling;
-> u64 {
    transition { _ -> iterate(ceiling, Pair { left: countdown, tag: 0 }) }
    state iterate(limit: u64 [0..=5], pending: Pair) {
        transition pending.left.inner.remaining > 0 {
            true -> iterate(limit, Pair { left: Countdown { label: pending.left.label, inner: Inner { remaining: pending.left.inner.remaining - 1 } }, tag: pending.tag })
            false -> pending.left.inner.remaining
        }
    }
}
"#;

#[test]
fn named_state_record_role_arrives_nested_in_a_unique_carrier_field() {
    // `pending.left` is the only `Countdown` path inside `Pair`, so `pending`
    // carries `countdown`'s role and the coordinate reads the nested chain.
    prove(NESTED_RECORD);
    // The same carriage behind the destination formal's own `&` unwraps one
    // borrow and reads the same nested chain. The leaf keeps no declared
    // field range there: construction-range proof does not read a declared
    // field bound through a `&` member chain, so an unconstrained `u64`
    // leaves the range judgment to the transported invariant alone.
    prove(
        &NESTED_RECORD
            .replace("remaining: u64 [0..=5];", "remaining: u64;")
            .replace("pending: Pair", "pending: &Pair")
            .replace("Pair { left: countdown", "&Pair { left: countdown")
            .replace("Pair { left: Countdown {", "&Pair { left: Countdown {"),
    );
    // A store into the carried record's path before the transition
    // invalidates the arrival premise.
    reject(&NESTED_RECORD.replace(
        "transition pending.left.inner.remaining > 0",
        "pending.left.inner.remaining = 2; transition pending.left.inner.remaining > 0",
    ));
    // The loop must actually descend the nested coordinate.
    reject(&NESTED_RECORD.replace(
        "pending.left.inner.remaining - 1",
        "pending.left.inner.remaining",
    ));
}

#[test]
fn named_state_record_role_rejects_ambiguous_nested_carriers() {
    // Two `Countdown` fields leave the carriage a guess: `pending.left` and
    // `pending.right` are equally valid readings, so the slot keeps no role
    // even though this arrival happens to use `left`.
    let source = r#"
    data Inner { remaining: u64 [0..=5]; }
    data Countdown { label: u64; inner: Inner; }
    data Pair { left: Countdown; right: Countdown; }
    measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.inner.remaining }

    machine walk(countdown: Countdown, ceiling: u64 [0..=5])
    requires countdown.inner.remaining <= ceiling;
    terminates by countdown -> Countdown::Remaining in 0..=ceiling;
    -> u64 {
        transition { _ -> iterate(ceiling, Pair { left: countdown, right: Countdown { label: 0, inner: Inner { remaining: 0 } } }) }
        state iterate(limit: u64 [0..=5], pending: Pair) {
            transition pending.left.inner.remaining > 0 {
                true -> iterate(limit, Pair { left: Countdown { label: pending.left.label, inner: Inner { remaining: pending.left.inner.remaining - 1 } }, right: pending.right })
                false -> pending.left.inner.remaining
            }
        }
    }
    "#;
    reject(source);
}

#[test]
fn named_state_record_role_arrives_as_the_chain_mid_record() {
    // `pending` IS the `inner` record the authored chain descends partway:
    // the coordinate reads `pending.remaining`, the residual leaf.
    let source = r#"
    data Inner { remaining: u64 [0..=5]; }
    data Countdown { label: u64; inner: Inner; }
    measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.inner.remaining }

    machine walk(countdown: Countdown, ceiling: u64 [0..=5])
    requires countdown.inner.remaining <= ceiling;
    terminates by countdown -> Countdown::Remaining in 0..=ceiling;
    -> u64 {
        transition { _ -> iterate(ceiling, countdown.inner) }
        state iterate(limit: u64 [0..=5], pending: Inner) {
            transition pending.remaining > 0 {
                true -> iterate(limit, Inner { remaining: pending.remaining - 1 })
                false -> pending.remaining
            }
        }
    }
    "#;
    prove(source);
    reject(&source.replace("pending.remaining - 1", "pending.remaining"));
}

#[test]
fn named_state_cannot_reuse_a_non_inductive_machine_requirement() {
    let source = r#"
    machine walk(remaining: u32 [0..=5])
    requires remaining > 0;
    terminates by remaining in 0..=5;
    -> u32 {
        transition { _ -> iterate(remaining) }
        state iterate(remaining: u32 [0..=5]) {
            transition { _ -> iterate(remaining - 1) }
        }
    }
    "#;
    reject(source);
}

#[test]
fn every_root_selected_named_loop_has_its_own_rank_proof() {
    let source = r#"
    machine walk(remaining: u32 [0..=5])
    terminates by remaining in 0..=5;
    -> u32 {
        transition remaining > 2 {
            true -> first(remaining)
            false -> second(remaining)
        }
        state first(pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> first(pending - 1)
                false -> pending
            }
        }
        state second(left: u32 [0..=5]) {
            transition left > 0 {
                true -> second(left - 1)
                false -> left
            }
        }
    }
    "#;
    prove(source);
    reject(&source.replace("second(left - 1)", "second(left)"));
}

#[test]
fn foreign_same_spelled_arrival_symbols_do_not_transport_rank_facts() {
    use typed_trees::expression::ExpressionNode;
    use typed_trees::statement::{StatementNode, TransitionTargetNode};

    for actual in ["remaining", "remaining + 0"] {
        let source = format!(
            "{} machine other(remaining: u32) -> u32 {{ remaining }}",
            COUNTDOWN.replace("iterate(remaining)", &format!("iterate({actual})"))
        );
        for replace_head in [false, true] {
            let mut program = typed(&source);
            let root = &program.machine_states(&program.machines()[0])[0];
            let StatementNode::Transition(arrival) =
                &program.statement_table.statements(root.statement_nodes)[0]
            else {
                panic!("root arrival");
            };
            let TransitionTargetNode::Named { arguments, .. } =
                program.statement_table.transition_target(arrival.target)
            else {
                panic!("named arrival");
            };
            let argument = program.statement_table.expression_handles(*arguments)[0];
            let argument = match program.expression_table.expression(argument) {
                ExpressionNode::Binary(binary) => binary.left,
                _ => argument,
            };
            let foreign = program
                .state_parameters(&program.machine_states(&program.machines()[1])[0])[0]
                .symbol;
            let ExpressionNode::Name(path) = program.expression_table.expression_mut(argument)
            else {
                panic!("identity forwarding");
            };
            path.symbol = foreign;
            if replace_head {
                path.head_symbol = foreign;
            }
            let diagnostics = crate::checks::termination::check_machine_termination(&program)
                .expect_err("spelling does not establish arrival custody");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("cannot prove rank range"))
            );
        }
    }
}
