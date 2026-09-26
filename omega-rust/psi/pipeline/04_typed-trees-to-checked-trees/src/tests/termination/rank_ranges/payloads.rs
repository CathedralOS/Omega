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
fn uninterpreted_integer_payloads_cannot_supply_cancelled_rank_terms() {
    let source = CLIMB
        .replace("payload: Payload", "payload: u64 in Wrapping")
        .replace("carried: Payload", "carried: u64 in Wrapping");
    for cancelled in ["payload - payload", "0 * payload"] {
        reject(&source.replace("limit + 1", &format!("limit + ({cancelled}) + 1")));
    }
}
