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
