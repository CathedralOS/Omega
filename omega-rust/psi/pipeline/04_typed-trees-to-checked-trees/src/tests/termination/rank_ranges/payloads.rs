use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const CLIMB: &str = r#"
data Payload { value: u64; }
machine climb(flag: bool, limit: u64 [0..=10], payload: Payload, index: u64)
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
    crate::checks::termination::check_machine_termination(&typed_program(source))
        .unwrap_or_else(|diagnostics| panic!("termination: {source}\n{diagnostics:#?}"));
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("complete checking: {source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed_program(source))
        .expect_err(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
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
    crate::checks::termination::check_machine_termination(&typed_program(&source))
        .expect("the numeric rank does not prove payload compatibility");
    let diagnostics = lower_typed_trees(typed_program(&source), &CheckingRequest::settled())
        .expect_err("incompatible payload arrival");
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

    crate::checks::termination::check_machine_termination(&typed_program(&source))
        .expect("rank evidence does not authorize copying an affine payload");
    let diagnostics = lower_typed_trees(typed_program(&source), &CheckingRequest::settled())
        .expect_err("affine payload copied");
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
                machine climb(limit: u64 [0..=10], index: u64)
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
fn duplicated_rank_or_endpoint_slots_cannot_choose_a_convenient_copy() {
    // A moved copy is named only by its strict step; it cannot be swapped for
    // a stale sibling when the step would make the stale copy the carrier.
    for (actual, arrival) in [
        ("index", "cursor + 1, cursor, enabled, ceiling"),
        ("limit", "ceiling + 1, cursor + 1, enabled, ceiling"),
        ("limit", "ceiling, cursor + 1, enabled, ceiling + 1"),
    ] {
        let source = CLIMB
            .replace(
                "iterate(payload, index, flag, limit)",
                &format!("iterate({actual}, index, flag, limit)"),
            )
            .replace("carried: Payload", "carried: u64")
            .replace("carried, cursor + 1, enabled, ceiling", arrival);
        reject(&source);
    }
    // Naming the stepped copy is not a convenience: `cursor` genuinely keeps
    // climbing toward `ceiling`, so the increasing distance still descends.
    let source = CLIMB
        .replace(
            "iterate(payload, index, flag, limit)",
            "iterate(index, index, flag, limit)",
        )
        .replace("carried: Payload", "carried: u64")
        .replace(
            "carried, cursor + 1, enabled, ceiling",
            "cursor, cursor + 1, enabled, ceiling",
        );
    prove(&source);
}

