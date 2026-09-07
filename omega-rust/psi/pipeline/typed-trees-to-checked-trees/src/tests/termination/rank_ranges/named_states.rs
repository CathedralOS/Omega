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

    let source = format!("{COUNTDOWN} machine other(remaining: u32) -> u32 {{ remaining }}");
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
        let foreign =
            program.state_parameters(&program.machine_states(&program.machines()[1])[0])[0].symbol;
        let ExpressionNode::Name(path) = program.expression_table.expression_mut(argument) else {
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
