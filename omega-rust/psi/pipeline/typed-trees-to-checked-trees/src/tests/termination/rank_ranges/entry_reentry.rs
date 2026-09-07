use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = r#"
machine walk(remaining: u32 [0..=5])
terminates by remaining in 0..=5;
-> u32 {
    transition remaining > 0 {
        true -> step(remaining - 1)
        false -> remaining
    }
    state step(pending: u32 [0..=5]) {
        transition pending > 0 {
            true -> walk(pending - 1)
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
fn named_state_can_reenter_entry_with_a_proven_rank() {
    prove(COUNTDOWN);
}

#[test]
fn every_edge_of_an_entry_cycle_still_owes_strict_descent() {
    reject(&COUNTDOWN.replace("step(remaining - 1)", "step(remaining)"));
    reject(&COUNTDOWN.replace("walk(pending - 1)", "walk(pending)"));
    reject(&COUNTDOWN.replace("walk(pending - 1)", "walk(pending - 2)"));
}

#[test]
fn entry_assumptions_cannot_restart_on_replacement_parameters() {
    let source = COUNTDOWN.replace("remaining: u32 [0..=5]", "remaining: u32")
        .replace("terminates by", "requires 1 <= remaining && remaining <= 5; terminates by")
        .replace("transition remaining > 0 {\n        true -> step(remaining - 1)\n        false -> remaining\n    }",
            "transition { _ -> step(remaining - 1) }");
    reject(&source);
    reject(
        "machine walk(n: u32) requires 1 <= n && n <= 5; terminates by n in 0..=5; -> u32 { transition { _ -> walk(n - 1) } }",
    );
}

#[test]
fn entry_parameter_refinements_still_require_valid_arrivals() {
    // Ranking may assume a state's parameter refinement, but complete checking
    // must reject the back-edge that leaves that refinement.
    let source = "machine walk(n: u32 [1..=5]) requires 1 <= n && n <= 5; terminates by n in 0..=5; -> u32 { transition { _ -> walk(n - 1) } }";
    let diagnostics = lower_typed_trees(typed(source)).expect_err(source);
    assert!(!diagnostics.is_empty(), "{source}");
}

const VARIABLE_STEP: &str = r#"
machine walk(n: u64, step: u64, cap: u64)
requires step > 0 && n <= cap;
terminates by n in 0..=cap;
-> u64 {
    transition n >= step {
        true -> walk(n - step, step, cap)
        false -> n
    }
}
"#;

#[test]
fn preserved_auxiliary_entry_facts_remain_available_as_invariants() {
    prove(VARIABLE_STEP);
}

#[test]
fn different_edges_cannot_mix_incompatible_induction_hypotheses() {
    reject(&VARIABLE_STEP.replace("false -> n", "").replace(
        "    }\n}",
        "    }\n    transition n > 0 { true -> walk(n - 1, 0, cap) false -> n }\n}",
    ));
}

#[test]
fn nonreentered_entry_retains_its_initial_precision() {
    prove(
        r#"
        machine walk(n: u32)
        requires n == 2;
        terminates by n in 0..=5;
        -> u32 {
            transition { _ -> finish(n + 1) }
            state finish(result: u32) { result }
        }
    "#,
    );
}

#[test]
fn entry_reentry_preserves_reordered_increasing_bounds() {
    let source = r#"
        machine climb(limit: u64, index: u64)
        requires index <= limit;
        terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1);
        -> u64 {
            transition index < limit {
                true -> step(index + 1, limit)
                false -> index
            }
            state step(cursor: u64, ceiling: u64) {
                transition cursor < ceiling {
                    true -> climb(ceiling, cursor + 1)
                    false -> cursor
                }
            }
        }
    "#;
    prove(source);
    reject(&source.replace(
        "climb(ceiling, cursor + 1)",
        "climb(ceiling + 1, cursor + 1)",
    ));
    reject(&source.replace("climb(ceiling, cursor + 1)", "climb(cursor + 1, ceiling)"));
    reject(&source.replace(
        "transition cursor < ceiling",
        "cursor = 0; transition cursor < ceiling",
    ));
}
