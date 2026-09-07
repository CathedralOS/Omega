use super::{lower_typed_trees, typed};

const CLIMB: &str = r#"
data Payload { value: u64; }
machine climb(flag: bool, limit: u64, payload: Payload, index: u64)
requires index <= limit;
terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1);
-> u64 {
    transition { _ -> iterate(payload, index, flag, limit) }
    state iterate(carried: Payload, cursor: u64, enabled: bool, ceiling: u64) {
        transition cursor < ceiling {
            true -> iterate(carried, cursor + 1, enabled, ceiling)
            false -> cursor
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
fn named_state_ranges_ignore_unrelated_payloads_without_shifting_ordinals() {
    prove(CLIMB);
    reject(&CLIMB.replace(
        "cursor + 1, enabled, ceiling)",
        "cursor + 1, enabled, ceiling + 1)",
    ));
    reject(&CLIMB.replace("cursor + 1, enabled, ceiling)", "cursor, enabled, ceiling)"));
}

#[test]
fn unrelated_payloads_can_cross_acyclic_and_cyclic_state_edges() {
    prove(
        r#"
    data Payload { value: u64; }
    machine walk(remaining: u32 [0..=5], payload: Payload)
    terminates by remaining in 0..=5;
    -> u32 {
        transition remaining > 2 {
            true -> first(payload, remaining)
            false -> second(remaining, payload)
        }
        state first(carried: Payload, pending: u32 [0..=5]) {
            transition pending > 0 {
                true -> second(pending - 1, carried)
                false -> pending
            }
        }
        state second(left: u32 [0..=5], saved: Payload) {
            transition left > 0 {
                true -> first(saved, left - 1)
                false -> left
            }
        }
    }
    "#,
    );
}

#[test]
fn numeric_mapping_mismatches_cannot_disappear_as_unrelated_payloads() {
    for destination_type in ["u32", "bool", "Payload", "u64 in Wrapping"] {
        reject(&CLIMB.replace("cursor: u64", &format!("cursor: {destination_type}")));
    }
}

#[test]
fn payload_type_compatibility_remains_an_ordinary_checking_obligation() {
    let source = CLIMB
        .replace(
            "data Payload { value: u64; }",
            "data Payload { value: u64; } data Other { value: u64; }",
        )
        .replace("carried: Payload", "carried: Other");
    crate::checks::termination::check_machine_termination(&typed(&source))
        .expect("the numeric rank does not prove payload compatibility");
    let diagnostics = lower_typed_trees(typed(&source)).expect_err("incompatible payload arrival");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("Payload")
                && diagnostic.message.contains("Other")),
        "{diagnostics:#?}"
    );
}

#[test]
fn unrelated_carriers_do_not_need_a_second_ranking_type_catalog() {
    for carrier in ["&Payload", "[u64; 2]", "f64", "u64 in Wrapping"] {
        prove(
            &CLIMB
                .replace("payload: Payload", &format!("payload: {carrier}"))
                .replace("carried: Payload", &format!("carried: {carrier}")),
        );
    }
}

#[test]
fn uninterpreted_integer_payloads_cannot_supply_cancelled_rank_terms() {
    let source = CLIMB
        .replace("payload: Payload", "payload: u64 in Wrapping")
        .replace("carried: Payload", "carried: u64 in Wrapping");
    for cancelled in ["payload - payload", "0 * payload"] {
        reject(&source.replace("limit + 1", &format!("limit + ({cancelled}) + 1")));
    }
}

#[test]
fn named_states_can_drop_unranked_entry_parameters() {
    let source = CLIMB
        .replace(
            "iterate(payload, index, flag, limit)",
            "iterate(index, limit)",
        )
        .replace(
            "carried: Payload, cursor: u64, enabled: bool, ceiling: u64",
            "cursor: u64, ceiling: u64",
        )
        .replace(
            "iterate(carried, cursor + 1, enabled, ceiling)",
            "iterate(cursor + 1, ceiling)",
        );
    prove(&source);
    prove(&source.replace("payload: Payload", "payload: u64"));
    reject(&source.replace("iterate(cursor + 1, ceiling)", "iterate(cursor, ceiling)"));
    reject(&source.replace(
        "iterate(cursor + 1, ceiling)",
        "iterate(cursor + 1, ceiling + 1)",
    ));
}

#[test]
fn named_states_can_duplicate_unranked_entry_parameters() {
    let source = CLIMB
        .replace(
            "iterate(payload, index, flag, limit)",
            "iterate(payload, index, payload, limit)",
        )
        .replace("enabled: bool", "enabled: Payload");
    prove(&source.replace(": Payload", ": &Payload"));
    let numeric = source
        .replace("payload: Payload", "payload: u64")
        .replace("carried: Payload", "carried: u64")
        .replace("enabled: Payload", "enabled: u64");
    prove(&numeric);
    // Shared ancestry does not imply the copies still contain equal values.
    prove(&numeric.replace(
        "iterate(carried, cursor + 1, enabled, ceiling)",
        "iterate(carried + 0, cursor + 1, cursor + 0, ceiling)",
    ));
    reject(
        &numeric
            .replace("cursor < ceiling", "carried < enabled")
            .replace("cursor + 1", "cursor"),
    );

    crate::checks::termination::check_machine_termination(&typed(&source))
        .expect("rank evidence does not authorize copying an affine payload");
    let diagnostics = lower_typed_trees(typed(&source)).expect_err("affine payload copied");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("already transferred or consumed")),
        "{diagnostics:#?}"
    );
}

#[test]
fn missing_rank_or_endpoint_slots_reject_even_on_impossible_arrivals() {
    for (actual, formal) in [("index", "cursor"), ("limit", "ceiling")] {
        for guard in ["index <= limit", "index > limit"] {
            reject(&format!(
                r#"
                machine climb(limit: u64, index: u64)
                requires index <= limit;
                terminates by index -> Nat::IncreasingTo(limit) in 0..=(limit + 1);
                -> u64 {{
                    transition {guard} {{
                        true -> finish({actual})
                        false -> index
                    }}
                    state finish({formal}: u64) {{ {formal} }}
                }}
            "#
            ));
        }
    }
}

#[test]
fn duplicated_rank_inputs_are_equal_at_every_arrival() {
    let source = CLIMB
        .replace(
            "iterate(payload, index, flag, limit)",
            "iterate(index, index, flag, limit)",
        )
        .replace("carried: Payload", "carried: u64")
        .replace(
            "carried, cursor + 1, enabled, ceiling",
            "cursor + 1, cursor + 1, enabled, ceiling",
        );
    prove(&source);
    reject(&source.replace(
        "iterate(index, index, flag, limit)",
        "iterate(index + 1, index, flag, limit)",
    ));
    reject(&source.replace(
        "cursor + 1, cursor + 1, enabled, ceiling",
        "cursor + 1, cursor, enabled, ceiling",
    ));
    reject(&source.replace(
        "cursor + 1, cursor + 1, enabled, ceiling",
        "cursor, cursor + 1, enabled, ceiling",
    ));
}

#[test]
fn duplicated_endpoints_are_pinned_as_equal_copies() {
    let source = CLIMB
        .replace(
            "iterate(payload, index, flag, limit)",
            "iterate(limit, index, flag, limit)",
        )
        .replace("carried: Payload", "carried: u64");
    prove(&source);
    reject(&source.replace(
        "carried, cursor + 1, enabled, ceiling",
        "carried + 1, cursor + 1, enabled, ceiling + 1",
    ));
}

#[test]
fn duplicate_equality_checks_parallel_and_acyclic_arrivals() {
    let source = r#"
        machine walk(remaining: u32 [0..=5])
        terminates by remaining in 0..=5;
        -> u32 {
            transition remaining > 2 {
                true -> prepare(remaining, remaining)
                false -> prepare(remaining, remaining)
            }
            state prepare(first: u32 [0..=5], second: u32 [0..=5]) {
                transition { _ -> iterate(second, first) }
            }
            state iterate(left: u32 [0..=5], right: u32 [0..=5]) {
                transition left > 0 && right > 0 {
                    true -> iterate(left - 1, right - 1)
                    false -> left
                }
            }
        }
    "#;
    prove(source);
    reject(&source.replace(
        "false -> prepare(remaining, remaining)",
        "false -> prepare(remaining + 1, remaining)",
    ));
    reject(&source.replace("iterate(second, first)", "iterate(second + 1, first)"));
}

#[test]
fn auxiliary_step_copies_are_an_inductively_checked_premise() {
    let source = r#"
        machine walk(remaining: u32 [0..=5], step: u32 [1..=1])
        terminates by remaining in 0..=5;
        -> u32 {
            transition { _ -> iterate(remaining, step, step) }
            state iterate(pending: u32, first: u32 [1..=1], second: u32 [1..=1]) {
                transition pending > 0 {
                    true -> iterate(pending - first, first, second)
                    false -> pending
                }
            }
        }
    "#;
    prove(source);
    reject(&source.replace(
        "pending - first, first, second",
        "pending - first, first, second + 1",
    ));
}

#[test]
fn duplicated_rank_or_endpoint_slots_cannot_choose_a_convenient_copy() {
    for (actual, arrivals) in [
        (
            "index",
            [
                "cursor + 1, cursor, enabled, ceiling",
                "cursor, cursor + 1, enabled, ceiling",
            ],
        ),
        (
            "limit",
            [
                "ceiling + 1, cursor + 1, enabled, ceiling",
                "ceiling, cursor + 1, enabled, ceiling + 1",
            ],
        ),
    ] {
        for arrival in arrivals {
            let source = CLIMB
                .replace(
                    "iterate(payload, index, flag, limit)",
                    &format!("iterate({actual}, index, flag, limit)"),
                )
                .replace("carried: Payload", "carried: u64")
                .replace("carried, cursor + 1, enabled, ceiling", arrival);
            reject(&source);
        }
    }
}

#[test]
fn different_arity_states_can_compose_exact_rank_mappings() {
    prove(
        r#"
        machine walk(unused: bool, remaining: u32 [0..=5], payload: u64)
        terminates by remaining in 0..=5;
        -> u32 {
            transition { _ -> prepare(payload, remaining, payload) }
            state prepare(first_copy: u64, pending: u32, second_copy: u64) {
                transition { _ -> iterate(pending, second_copy) }
            }
            state iterate(left: u32, carried: u64) {
                transition left > 0 {
                    true -> iterate(left - 1, carried)
                    false -> left
                }
            }
        }
    "#,
    );
}
