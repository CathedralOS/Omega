use super::{lower_typed_trees, typed};

const NAMED_QUOTIENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/named_computed_rank_endpoints/main.omg"
));

fn accepts_named(source: &str) {
    lower_typed_trees(typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn rejects_named(source: &str) {
    let diagnostics = lower_typed_trees(typed(source)).expect_err("invalid rank range");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn quotient_rank_endpoint_follows_renamed_and_reordered_state_inputs() {
    accepts_named(NAMED_QUOTIENT);
    for endpoint in [
        "(cap / 2) / 3 + 6",
        "(cap % 5) / 2 + 6",
        "(cap / 2) % 5 + 6",
        "12u64 / 2u64",
        "cap / (10u64 / 2u64) + 6",
    ] {
        accepts_named(&NAMED_QUOTIENT.replace("cap / 5 + 6", endpoint));
    }
    // A subordinate call consumes a genuine declared range at the arrival;
    // it is not replaced by a termination-only assumption.
    accepts_named(&format!(
        "machine within(value: u64) -> u64 requires value <= 5 {{ value }} {}",
        NAMED_QUOTIENT.replace("false -> pending", "false -> within(pending)")
    ));
}

#[test]
fn quotient_endpoint_follows_exact_record_fields_at_named_arrivals() {
    let source = r#"
        data Limits { bound: u64 [0..=20]; other: u64 [0..=20]; }
        machine walk(remaining: u64 [0..=5], limits: Limits)
        terminates by remaining in 0..(limits.bound / 5 + 6);
        -> u64 {
            transition { _ -> iterate(limits, remaining) }
            state iterate(held: Limits, pending: u64 [0..=5]) {
                transition pending > 0 {
                    true -> iterate(Limits { bound: held.bound, other: 0 }, pending - 1)
                    false -> pending
                }
            }
        }
    "#;
    accepts_named(source);
    rejects_named(&source.replace("bound: held.bound", "bound: held.other"));
    let written = source
        .replace("state iterate(held:", "state iterate(mut held:")
        .replace(
            "transition pending > 0",
            "held.bound = 0; transition pending > 0",
        );
    accepts_named(&written.replace("limits.bound / 5 + 6", "6"));
    rejects_named(&written);
}

#[test]
fn quotient_endpoint_requires_exact_conservation_and_strict_descent() {
    for arguments in ["0, pending - 1", "limit / 2, pending - 1", "limit, pending"] {
        rejects_named(&NAMED_QUOTIENT.replace("limit - 0, pending - 1", arguments));
    }
    rejects_named(
        &NAMED_QUOTIENT
            .replace("pending > 0", "pending > 0 && pending < 5")
            .replace("limit - 0, pending - 1", "limit, pending + 1"),
    );
    rejects_named(&NAMED_QUOTIENT.replace("iterate(cap, remaining)", "iterate(0, remaining)"));
}

#[test]
fn quotient_endpoint_preserves_anonymous_rationals_and_selected_meaning() {
    accepts_named(&NAMED_QUOTIENT.replace("cap / 5 + 6", "cap / 5 + (1 / 2 * 12)"));
    for endpoint in ["cap / 0 + 6", "cap / 5u8 + 6", "cap / 5 + (1 / 2)"] {
        rejects_named(&NAMED_QUOTIENT.replace("cap / 5 + 6", endpoint));
    }
    rejects_named(&format!(
        "operator / u64::quotient(left: u64, right: u64) -> u64; {NAMED_QUOTIENT}"
    ));
}

#[test]
fn quotient_endpoint_cannot_hide_an_overflowing_intermediate() {
    for endpoint in [
        "(cap + 18446744073709551615u64) / 5 + 6",
        "(cap + 18446744073709551615u64 - 18446744073709551615u64) / 5 + 6",
    ] {
        rejects_named(&NAMED_QUOTIENT.replace("cap / 5 + 6", endpoint));
    }
    // This input is not bounded in its type. Endpoint formation must use
    // independently transported entry requirements, including at named states.
    let bounded = NAMED_QUOTIENT
        .replace("u64 [0..=20]", "u64")
        .replace("terminates by", "requires cap <= 20;\nterminates by")
        .replace("cap / 5 + 6", "(cap + 1) / 5 + 6");
    accepts_named(&bounded);
    rejects_named(&bounded.replace("requires cap <= 20;", ""));
}

#[test]
fn signed_quotient_endpoints_use_truncation_and_check_minimum_overflow() {
    let signed = NAMED_QUOTIENT
        .replace("cap: u64 [0..=20]", "cap: i8 [-7..=-7]")
        .replace("limit: u64 [0..=20]", "limit: i8 [-7..=-7]");
    // -7 / 2 is -3, not -4; -7 / -2 is 3, not 4.
    accepts_named(&signed.replace("cap / 5 + 6", "cap / 2 + 9"));
    accepts_named(&signed.replace("cap / 5 + 6", "cap / -2 + 3"));
    rejects_named(&signed.replace("cap / 5 + 6", "cap / -2 + 2"));
    let minimum = signed
        .replace("[-7..=-7]", "[-128..=-128]")
        .replace("cap / 5 + 6", "(cap / -1) / 2");
    rejects_named(&minimum);
}

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_rank_arithmetic/main.omg"
));

#[test]
fn field_rank_accepts_bounded_arithmetic_over_pinned_inputs() {
    lower_typed_trees(typed(COUNTDOWN)).expect("both ceiling inputs stay fixed");
    for endpoint in [
        "ceiling + 1",
        "ceiling * 2",
        "ceiling + padding - 0",
        "ceiling / 1 + padding",
    ] {
        lower_typed_trees(typed(
            &COUNTDOWN.replace("ceiling + padding;", &format!("{endpoint};")),
        ))
        .expect(endpoint);
    }
    let remainder = COUNTDOWN
        .replace("ceiling: u64 [5..=10]", "ceiling: u64")
        .replace("ceiling + padding;", "ceiling % 5 + 6;");
    lower_typed_trees(typed(&remainder))
        .expect("the result is bounded even when its input exceeds i64");
}

#[test]
fn computed_rank_endpoint_requires_every_input_to_stay_pinned() {
    for arguments in ["5, padding)", "ceiling, 1)"] {
        let source = COUNTDOWN.replace("ceiling, padding)", arguments);
        let diagnostics = crate::checks::termination::check_machine_termination(&typed(&source))
            .expect_err("each endpoint input has independent pinning");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .contains("cannot prove rank range `0..ceiling + padding`")
            }),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn computed_rank_endpoint_does_not_hide_partial_or_authored_arithmetic() {
    for endpoint in [
        "ceiling / 0",
        "ceiling % 0",
        "ceiling - 11",
        "ceiling + 18446744073709551615u64",
    ] {
        let source = COUNTDOWN.replace("ceiling + padding;", &format!("{endpoint};"));
        crate::checks::termination::check_machine_termination(&typed(&source))
            .expect_err("every endpoint operation needs defined and representable bounds");
    }
    let authored = format!("operator + u64::sum(left: u64, right: u64) -> u64; {COUNTDOWN}");
    crate::checks::termination::check_machine_termination(&typed(&authored))
        .expect_err("authored addition cannot inherit builtin interval laws");
    let hidden_overflow = COUNTDOWN
        .replace("ceiling: u64 [5..=10]", "ceiling: u64")
        .replace("ceiling + padding;", "(ceiling + 1) % 5 + 6;");
    crate::checks::termination::check_machine_termination(&typed(&hidden_overflow))
        .expect_err("a small final endpoint cannot hide an overflowing intermediate");
}

#[test]
fn constant_rank_endpoints_preserve_landing_and_rational_meaning() {
    for endpoint in ["255u8 + 1u8", "6 + 1 / 2", "12 % 7", "6u8 + 1u64"] {
        let source = COUNTDOWN.replace("ceiling + padding;", &format!("{endpoint};"));
        crate::checks::termination::check_machine_termination(&typed(&source)).expect_err(endpoint);
    }
    let changed_floor = COUNTDOWN.replace("0..ceiling + padding", "(1 / 2 * 2)..6");
    crate::checks::termination::check_machine_termination(&typed(&changed_floor))
        .expect_err("the exact rational floor is one, not zero");
    for endpoint in ["1 / 2 * 12", "6u8 + 1u8", "ceiling + (1 / 2 * 2)"] {
        let source = COUNTDOWN.replace("ceiling + padding;", &format!("{endpoint};"));
        lower_typed_trees(typed(&source)).expect(endpoint);
    }
}
