use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::{typed_program, typed_program_result};

const NAMED_QUOTIENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/named_computed_rank_endpoints/main.omg"
));

fn accepts_named(source: &str) {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

const SYMBOLIC_QUOTIENT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/symbolic_quotient_endpoints/main.omg"
));

#[test]
fn symbolic_remainder_preserves_both_inputs_and_each_operation() {
    let source = SYMBOLIC_QUOTIENT.replace("cap / divisor", "cap % divisor");
    accepts_named(&source);
    for changed in [
        source.replace("iterate(width, limit - 0", "iterate(1, limit - 0"),
        source.replace("iterate(width, limit - 0", "iterate(width, 0"),
        source.replace("pending - 1", "pending"),
        source.replace("[1..=5]", "[0..=5]"),
        source.replace("% divisor", "% (divisor - divisor)"),
        source.replace("u64 [1..=5]", "u8 [1..=5]"),
        format!("operator % u64::chosen(left: u64, right: u64) -> u64; {source}"),
    ] {
        rejects_named(&changed);
    }
    let nested = source.replace("cap % divisor", "(cap % divisor) % divisor");
    accepts_named(&nested);
    rejects_named(&nested.replace("iterate(width, limit - 0", "iterate(1, limit - 0"));
    let canceled = source.replace("cap % divisor + 6", "cap % divisor - cap % divisor + 6");
    rejects_named(&canceled.replace("[1..=5]", "[0..=5]"));
    let zero_from_bounds = source.replace("u64 [1..=5]", "u64").replace(
        "terminates by",
        "requires divisor >= 0 && divisor < 1; terminates by",
    );
    rejects_named(&zero_from_bounds);
    let overflowing = source
        .replace("[0..=20]", "[0..=18446744073709551615]")
        .replace("cap % divisor", "(cap + 1) % divisor");
    rejects_named(&overflowing);
}

#[test]
fn symbolic_remainder_keeps_signed_quotient_formation() {
    let source = SYMBOLIC_QUOTIENT
        .replace("cap: u64 [0..=20]", "cap: i8 [-7..=-5]")
        .replace("limit: u64 [0..=20]", "limit: i8 [-7..=-5]")
        .replace("u64 [1..=5]", "i8 [-3..=-2]")
        .replace("cap / divisor + 6", "cap % divisor + 8");
    accepts_named(&source);
    accepts_named(&source.replace("[-3..=-2]", "[2..=3]"));
    rejects_named(&source.replace("+ 8", "+ 7"));
    let canceled = source
        .replace("cap % divisor + 8", "cap % divisor - cap % divisor + 6")
        .replace("[-7..=-5]", "[-128..=-128]");
    accepts_named(&canceled);
    rejects_named(&canceled.replace("[-3..=-2]", "[-3..=-1]"));
    accepts_named(
        &canceled
            .replace("[-3..=-2]", "[-3..=-1]")
            .replace("[-128..=-128]", "[-127..=-127]"),
    );
}

#[test]
fn remainder_length_coordinates_keep_constant_modulus_support() {
    accepts_named(
        "machine walk(remaining: u64 [0..=5], values: &[u8])
        terminates by remaining in 0..(values.len % 5 + 6);
        -> u64 {
            transition remaining > 0 {
                true -> walk(remaining - 1, values)
                false -> remaining
            }
        }",
    );
}

#[test]
fn symbolic_quotient_endpoint_preserves_both_operands_at_named_arrivals() {
    accepts_named(SYMBOLIC_QUOTIENT);
    rejects_named(&SYMBOLIC_QUOTIENT.replace("iterate(width, limit - 0", "iterate(1, limit - 0"));
    rejects_named(&SYMBOLIC_QUOTIENT.replace("iterate(width, limit - 0", "iterate(width, 0"));
    rejects_named(&SYMBOLIC_QUOTIENT.replace("pending - 1", "pending"));
}

#[test]
fn symbolic_quotient_endpoint_formation_keeps_each_operation() {
    let unbounded = SYMBOLIC_QUOTIENT
        .replace("u64 [1..=5]", "u64")
        .replace("terminates by", "requires divisor > 0; terminates by");
    accepts_named(&unbounded);
    rejects_named(&unbounded.replace("requires divisor > 0;", ""));
    rejects_named(&SYMBOLIC_QUOTIENT.replace("/ divisor", "/ (divisor - divisor)"));
    rejects_named(&format!(
        "operator / u64::chosen(left: u64, right: u64) -> u64; {SYMBOLIC_QUOTIENT}"
    ));
    rejects_named(&SYMBOLIC_QUOTIENT.replace("u64 [1..=5]", "u8 [1..=5]"));
    let nested = SYMBOLIC_QUOTIENT.replace("/ divisor", "/ (divisor / 2)");
    accepts_named(&nested.replace("[1..=5]", "[2..=5]"));
    rejects_named(&nested);
    let canceled =
        SYMBOLIC_QUOTIENT.replace("cap / divisor + 6", "cap / divisor - cap / divisor + 6");
    rejects_named(&canceled.replace("[1..=5]", "[0..=5]"));
    rejects_named(&canceled.replace("/ divisor", "/ (divisor / 2)"));
    let signed = canceled
        .replace("cap: u64 [0..=20]", "cap: i8 [-128..=-128]")
        .replace("limit: u64 [0..=20]", "limit: i8 [-128..=-128]")
        .replace("u64 [1..=5]", "i8 [-1..=-1]");
    rejects_named(&signed);
    accepts_named(&signed.replace("[-128..=-128]", "[-127..=-127]"));
}

#[test]
fn symbolic_quotient_endpoint_uses_signed_truncation_bounds() {
    let signed = SYMBOLIC_QUOTIENT
        .replace("cap: u64 [0..=20]", "cap: i8 [-7..=-5]")
        .replace("limit: u64 [0..=20]", "limit: i8 [-7..=-5]")
        .replace("u64 [1..=5]", "i8 [-3..=-2]")
        .replace("cap / divisor + 6", "cap / divisor + 5");
    accepts_named(&signed);
    rejects_named(&signed.replace("cap / divisor + 5", "cap / divisor + 4"));
    accepts_named(&signed.replace("[-3..=-2]", "[2..=3]").replace("+ 5", "+ 9"));
}

#[test]
fn symbolic_quotient_endpoint_tracks_projected_denominators() {
    let source = r#"
        data Limits { cap: u64 [0..=20]; divisor: u64 [1..=5]; }
        machine walk(remaining: u64 [0..=5], limits: Limits)
        terminates by remaining in 0..(limits.cap / limits.divisor + 6);
        -> u64 {
            transition { _ -> iterate(limits, remaining) }
            state iterate(held: Limits, pending: u64 [0..=5]) {
                transition pending > 0 {
                    true -> iterate(Limits { cap: held.cap, divisor: held.divisor }, pending - 1)
                    false -> pending
                }
            }
        }
    "#;
    accepts_named(source);
    rejects_named(&source.replace("divisor: held.divisor", "divisor: 1"));
    rejects_named(
        &source
            .replace("state iterate(held:", "state iterate(mut held:")
            .replace(
                "transition pending > 0",
                "held.divisor = 1; transition pending > 0",
            ),
    );
}

#[test]
fn symbolic_division_endpoint_rechecks_formation_after_arrival() {
    let source = r#"
        machine walk(remaining: u64 [0..=5], cap: u64 [0..=20], divisor: u64)
        requires divisor > 0;
        terminates by remaining in 0..(cap / divisor - cap / divisor + 6);
        -> u64 {
            transition remaining > 0 && divisor > 0 && cap <= 20 {
                true -> walk(remaining - 1, cap - 0, divisor - divisor)
                false -> remaining
            }
        }
    "#;
    // The rank-only invariant can justify descent, but no zero-divisor
    // arrival. Reject in the rank judgment itself, independently of ordinary
    // call-contract replay also rejecting the lost `requires` premise.
    for operator in ["/", "%"] {
        let source = source.replace(" / ", &format!(" {operator} "));
        rejects_named(&source);
        rejects_named(&source.replace("&& divisor > 0", "&& divisor == 1"));
        accepts_named(&source.replace("divisor - divisor", "divisor - 0"));
    }
}

#[test]
fn symbolic_shift_endpoint_preserves_value_and_count_at_named_arrivals() {
    // An exact `<<` endpoint keeps the shifted operand AND its count as
    // transported terms: the count's defined range is the shifted carrier's
    // width (F8), proved under the same hypotheses a nonzero divisor needs.
    let source = SYMBOLIC_QUOTIENT.replace("cap / divisor", "(cap << divisor)");
    accepts_named(&source);
    for changed in [
        source.replace("iterate(width, limit - 0", "iterate(1, limit - 0"),
        source.replace("iterate(width, limit - 0", "iterate(width, 0"),
        source.replace("pending - 1", "pending"),
    ] {
        rejects_named(&changed);
    }
    // The count's carrier is independent of the shifted operand's: a `u8`
    // count beside a `u64` value is an ordinary exact shift.
    accepts_named(
        &source
            .replace("divisor: u64 [1..=5]", "divisor: u8 [1..=5]")
            .replace("width: u64 [1..=5]", "width: u8 [1..=5]"),
    );
    // A spelled zero count is defined (`value << 0` is the value); a
    // nested non-polynomial endpoint transports innermost-first.
    accepts_named(&source.replace("(cap << divisor)", "(cap << 0)"));
    accepts_named(&source.replace("(cap << divisor)", "((cap % divisor) << divisor)"));
    // An anonymous shifted value selects no width, so only a typed
    // carrier's shift mints an endpoint term.
    rejects_named(&source.replace("(cap << divisor)", "(20 << divisor)"));
    // Cancellation cannot hide the count: `x << k - x << k` still owes
    // `k < width` at every constituent operation.
    let canceled = source.replace(
        "(cap << divisor) + 6",
        "(cap << divisor) - (cap << divisor) + 6",
    );
    accepts_named(&canceled);
    rejects_named(&canceled.replace("[1..=5]", "[1..=70]"));
    // Out-of-width counts reject: a spelled constant at the width fails
    // its own formation, and a count interval reaching the width cannot
    // prove `< 64` under the endpoint's hypotheses.
    for endpoint in ["(cap << 64)", "(cap << (divisor + 60))"] {
        let source = source.replace("(cap << divisor)", endpoint);
        lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).expect_err(&source);
    }
    rejects_named(&source.replace("[1..=5]", "[1..=70]"));
}

#[test]
fn symbolic_shift_right_endpoint_preserves_value_and_count_at_named_arrivals() {
    // An exact `>>` endpoint keeps the shifted operand AND its count as
    // transported terms under the same F8 count ruling as `<<`; its result
    // is `floor(value / 2^count)`, tightening the operand's interval rather
    // than growing it.
    let source = SYMBOLIC_QUOTIENT.replace("cap / divisor", "(cap >> divisor)");
    accepts_named(&source);
    for changed in [
        source.replace("iterate(width, limit - 0", "iterate(1, limit - 0"),
        source.replace("iterate(width, limit - 0", "iterate(width, 0"),
        source.replace("pending - 1", "pending"),
    ] {
        rejects_named(&changed);
    }
    // The count's carrier is independent of the shifted operand's: a `u8`
    // count beside a `u64` value is an ordinary exact shift.
    accepts_named(
        &source
            .replace("divisor: u64 [1..=5]", "divisor: u8 [1..=5]")
            .replace("width: u64 [1..=5]", "width: u8 [1..=5]"),
    );
    // A spelled zero count is defined (`value >> 0` is the value); nested
    // non-polynomial endpoints transport innermost-first, including a `<<`
    // operand inside the `>>` value.
    accepts_named(&source.replace("(cap >> divisor)", "(cap >> 0)"));
    accepts_named(&source.replace("(cap >> divisor)", "((cap % divisor) >> divisor)"));
    accepts_named(&source.replace("(cap >> divisor)", "((cap << divisor) >> divisor)"));
    // An anonymous shifted value selects no width, so only a typed
    // carrier's shift mints an endpoint term.
    rejects_named(&source.replace("(cap >> divisor)", "(20 >> divisor)"));
    // Cancellation cannot hide the count: `x >> k - x >> k` still owes
    // `k < width` at every constituent operation.
    let canceled = source.replace(
        "(cap >> divisor) + 6",
        "(cap >> divisor) - (cap >> divisor) + 6",
    );
    accepts_named(&canceled);
    rejects_named(&canceled.replace("[1..=5]", "[1..=70]"));
    // Out-of-width counts reject: a spelled constant at the width fails
    // its own formation, and a count interval reaching the width cannot
    // prove `< 64` under the endpoint's hypotheses.
    for endpoint in ["(cap >> 64)", "(cap >> (divisor + 60))"] {
        let source = source.replace("(cap >> divisor)", endpoint);
        lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).expect_err(&source);
    }
    rejects_named(&source.replace("[1..=5]", "[1..=70]"));
}

#[test]
fn signed_shift_right_endpoint_uses_floor_division() {
    // `-21 >> 1` is `-11` -- floor division by `2^count`, one below the
    // truncating quotient `-10`. The signed endpoint's interval keeps that
    // floor corner through the named arrival, so `(cap >> divisor) + 17`
    // lands the ceiling at `6`.
    let signed = SYMBOLIC_QUOTIENT
        .replace("cap: u64 [0..=20]", "cap: i8 [-21..=-21]")
        .replace("limit: u64 [0..=20]", "limit: i8 [-21..=-21]")
        .replace("u64 [1..=5]", "i8 [1..=1]")
        .replace("cap / divisor + 6", "(cap >> divisor) + 17");
    accepts_named(&signed);
    // `+ 16` puts the ceiling at `5`, which `pending = 5` violates; a
    // truncating model would reach `6` and accept, so this rejection is the
    // floor-division witness.
    rejects_named(&signed.replace("+ 17", "+ 16"));
    // The named-state transport keeps the same floor corner: a
    // nondecreasing cycle still rejects, and a count interval reaching the
    // `i8` width cannot prove `< 8`.
    rejects_named(&signed.replace("pending - 1", "pending"));
    rejects_named(&signed.replace("i8 [1..=1]", "i8 [1..=9]"));
}

#[test]
fn symbolic_bitwise_and_endpoint_preserves_both_operands_at_named_arrivals() {
    // `&` is total inside its shared carrier: the endpoint owes no divisor-
    // or count-style side condition, but both operands still transport as
    // exact terms. `[0, 20] & [1, 5]` reads `[0, 5]` -- a provably
    // nonnegative operand bounds the result below itself and above zero.
    let source = SYMBOLIC_QUOTIENT.replace("cap / divisor", "(cap & divisor)");
    accepts_named(&source);
    for changed in [
        source.replace("iterate(width, limit - 0", "iterate(1, limit - 0"),
        source.replace("iterate(width, limit - 0", "iterate(width, 0"),
        source.replace("pending - 1", "pending"),
    ] {
        rejects_named(&changed);
    }
    // A literal mask and a nested non-polynomial operand mint the same way:
    // `(cap % divisor) & divisor` transports innermost-first.
    accepts_named(&source.replace("(cap & divisor)", "(cap & 7)"));
    accepts_named(&source.replace("(cap & divisor)", "((cap % divisor) & divisor)"));
    accepts_named(&source.replace("(cap & divisor)", "((cap << divisor) & divisor)"));
    // Totality needs no side condition, so the canceled endpoint is an
    // ordinary `+ 6`; what still rejects above is lost operand
    // correspondence, not the operation's own formation.
    let canceled = source.replace(
        "(cap & divisor) + 6",
        "(cap & divisor) - (cap & divisor) + 6",
    );
    accepts_named(&canceled);
    // Both operands share one integer carrier: a `u8` mask beside the `u64`
    // subject selects no builtin `&`, and a literal outside the carrier or
    // a negative one cannot land in it. These are malformed endpoints --
    // whichever stage sees them first rejects.
    for endpoint in ["(cap & 5u8)", "(cap & 18446744073709551616)", "(cap & -1)"] {
        rejects_any_stage(&source.replace("(cap & divisor)", endpoint));
    }
    rejects_any_stage(&source.replace("divisor: u64 [1..=5]", "divisor: u8 [1..=5]"));
}

#[test]
fn signed_bitwise_and_endpoint_uses_operand_sign_bounds() {
    // A provably nonnegative mask clears the result's sign bit even beside a
    // provably negative operand: `cap & divisor` over `i8` lands `[0, 5]`,
    // not the dividend's `[-21, -1]` and not the whole carrier.
    let signed = SYMBOLIC_QUOTIENT
        .replace("cap: u64 [0..=20]", "cap: i8 [-21..=-1]")
        .replace("limit: u64 [0..=20]", "limit: i8 [-21..=-1]")
        .replace("u64 [1..=5]", "i8 [1..=5]")
        .replace("cap / divisor + 6", "(cap & divisor) + 6");
    accepts_named(&signed);
    rejects_named(&signed.replace("pending - 1", "pending"));
    // Two provably negative operands keep the result below `min(a, b)` at
    // the carrier floor -- `-21 & -1` is `-21`, so `+ 6` cannot bound the
    // positive pending values the nonnegative mask could.
    rejects_named(&signed.replace("i8 [1..=5]", "i8 [-5..=-1]"));
}

/// A malformed endpoint can fail typing or checking; either rejection is the
/// closed behavior. Unlike [`rejects_named`] this makes no claim about which
/// stage owns the diagnostic.
fn rejects_any_stage(source: &str) {
    match typed_program_result(source) {
        Ok(program) => {
            lower_typed_trees(program, &CheckingRequest::settled()).expect_err(source);
        }
        Err(_) => {}
    }
}

fn rejects_named(source: &str) {
    let Err(diagnostics) = lower_typed_trees(typed_program(source), &CheckingRequest::settled())
    else {
        panic!("invalid rank range accepted:\n{source}");
    };
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
    lower_typed_trees(typed_program(COUNTDOWN), &CheckingRequest::settled())
        .expect("both ceiling inputs stay fixed");
    for endpoint in [
        "ceiling + 1",
        "ceiling * 2",
        "ceiling + padding - 0",
        "ceiling / 1 + padding",
    ] {
        lower_typed_trees(
            typed_program(&COUNTDOWN.replace("ceiling + padding;", &format!("{endpoint};"))),
            &CheckingRequest::settled(),
        )
        .expect(endpoint);
    }
    let remainder = COUNTDOWN
        .replace("ceiling: u64 [5..=10]", "ceiling: u64")
        .replace("ceiling + padding;", "ceiling % 5 + 6;");
    lower_typed_trees(typed_program(&remainder), &CheckingRequest::settled())
        .expect("the result is bounded even when its input exceeds i64");
}

#[test]
fn computed_rank_endpoint_requires_every_input_to_stay_pinned() {
    for arguments in ["5, padding)", "ceiling, 1)"] {
        let source = COUNTDOWN.replace("ceiling, padding)", arguments);
        let diagnostics =
            crate::checks::termination::check_machine_termination(&typed_program(&source))
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
        crate::checks::termination::check_machine_termination(&typed_program(&source))
            .expect_err("every endpoint operation needs defined and representable bounds");
    }
    let authored = format!("operator + u64::sum(left: u64, right: u64) -> u64; {COUNTDOWN}");
    crate::checks::termination::check_machine_termination(&typed_program(&authored))
        .expect_err("authored addition cannot inherit builtin interval laws");
    let hidden_overflow = COUNTDOWN
        .replace("ceiling: u64 [5..=10]", "ceiling: u64")
        .replace("ceiling + padding;", "(ceiling + 1) % 5 + 6;");
    crate::checks::termination::check_machine_termination(&typed_program(&hidden_overflow))
        .expect_err("a small final endpoint cannot hide an overflowing intermediate");
}

#[test]
fn constant_rank_endpoints_preserve_landing_and_rational_meaning() {
    for endpoint in ["255u8 + 1u8", "6 + 1 / 2", "12 % 7", "6u8 + 1u64"] {
        let source = COUNTDOWN.replace("ceiling + padding;", &format!("{endpoint};"));
        crate::checks::termination::check_machine_termination(&typed_program(&source))
            .expect_err(endpoint);
    }
    let changed_floor = COUNTDOWN.replace("0..ceiling + padding", "(1 / 2 * 2)..6");
    crate::checks::termination::check_machine_termination(&typed_program(&changed_floor))
        .expect_err("the exact rational floor is one, not zero");
    for endpoint in ["1 / 2 * 12", "6u8 + 1u8", "ceiling + (1 / 2 * 2)"] {
        let source = COUNTDOWN.replace("ceiling + padding;", &format!("{endpoint};"));
        lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).expect(endpoint);
    }
}
