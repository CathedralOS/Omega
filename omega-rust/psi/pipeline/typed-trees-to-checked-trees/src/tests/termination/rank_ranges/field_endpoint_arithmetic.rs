use super::{lower_typed_trees, typed};

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
