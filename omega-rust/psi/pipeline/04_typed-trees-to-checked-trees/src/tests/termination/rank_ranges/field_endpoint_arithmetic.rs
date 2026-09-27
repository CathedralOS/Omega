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

// Corpus fixtures evolve independently of these derived cases. Check every
// rewritten anchor so a range/carrier mutation cannot silently become a no-op.
fn rewrite_fixture(source: &str, replacements: &[(&str, &str)]) -> String {
    replacements
        .iter()
        .fold(source.to_owned(), |source, (before, after)| {
            assert!(source.contains(before), "missing fixture anchor: {before}");
            source.replace(before, after)
        })
}

#[test]
fn symbolic_remainder_keeps_signed_quotient_formation() {
    let source = rewrite_fixture(
        SYMBOLIC_QUOTIENT,
        &[
            ("cap: u64", "cap: i8"),
            ("limit: u64", "limit: i8"),
            ("divisor: u64", "divisor: i8"),
            ("width: u64", "width: i8"),
            ("0 <= cap && cap <= 20", "-7 <= cap && cap <= -5"),
            ("0 <= limit && limit <= 20", "-7 <= limit && limit <= -5"),
            (
                "1 <= divisor && divisor <= 5",
                "-3 <= divisor && divisor <= -2",
            ),
            ("1 <= width && width <= 5", "-3 <= width && width <= -2"),
            ("cap / divisor + 6", "cap % divisor + 8"),
        ],
    );
    accepts_named(&source);
    accepts_named(&rewrite_fixture(
        &source,
        &[
            (
                "-3 <= divisor && divisor <= -2",
                "2 <= divisor && divisor <= 3",
            ),
            ("-3 <= width && width <= -2", "2 <= width && width <= 3"),
        ],
    ));
    rejects_named(&source.replace("+ 8", "+ 7"));
    let canceled = source
        .replace("cap % divisor + 8", "cap % divisor - cap % divisor + 6")
        .replace("-7 <=", "-128 <=")
        .replace("<= -5", "<= -128");
    accepts_named(&canceled);
    rejects_named(&rewrite_fixture(&canceled, &[("<= -2", "<= -1")]));
    accepts_named(&canceled.replace("<= -2", "<= -1").replace("-128", "-127"));
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
    accepts_named(&rewrite_fixture(
        &source,
        &[("divisor: u64", "divisor: u8"), ("width: u64", "width: u8")],
    ));
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
    rejects_named(&rewrite_fixture(
        &canceled,
        &[
            ("divisor <= 5", "divisor <= 70"),
            ("width <= 5", "width <= 70"),
        ],
    ));
    // Out-of-width counts reject: a spelled constant at the width fails
    // its own formation, and a count interval reaching the width cannot
    // prove `< 64` under the endpoint's hypotheses.
    for endpoint in ["(cap << 64)", "(cap << (divisor + 60))"] {
        let source = source.replace("(cap << divisor)", endpoint);
        lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).expect_err(&source);
    }
    rejects_named(&rewrite_fixture(
        &source,
        &[
            ("divisor <= 5", "divisor <= 70"),
            ("width <= 5", "width <= 70"),
        ],
    ));
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
    accepts_named(&rewrite_fixture(
        &source,
        &[("divisor: u64", "divisor: u8"), ("width: u64", "width: u8")],
    ));
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
    rejects_named(&rewrite_fixture(
        &canceled,
        &[
            ("divisor <= 5", "divisor <= 70"),
            ("width <= 5", "width <= 70"),
        ],
    ));
    // Out-of-width counts reject: a spelled constant at the width fails
    // its own formation, and a count interval reaching the width cannot
    // prove `< 64` under the endpoint's hypotheses.
    for endpoint in ["(cap >> 64)", "(cap >> (divisor + 60))"] {
        let source = source.replace("(cap >> divisor)", endpoint);
        lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).expect_err(&source);
    }
    rejects_named(&rewrite_fixture(
        &source,
        &[
            ("divisor <= 5", "divisor <= 70"),
            ("width <= 5", "width <= 70"),
        ],
    ));
}

#[test]
fn signed_shift_right_endpoint_uses_floor_division() {
    // `-21 >> 1` is `-11` -- floor division by `2^count`, one below the
    // truncating quotient `-10`. The signed endpoint's interval keeps that
    // floor corner through the named arrival, so `(cap >> divisor) + 17`
    // lands the ceiling at `6`.
    let signed = rewrite_fixture(
        SYMBOLIC_QUOTIENT,
        &[
            ("cap: u64", "cap: i8"),
            ("limit: u64", "limit: i8"),
            ("divisor: u64", "divisor: i8"),
            ("width: u64", "width: i8"),
            ("0 <= cap && cap <= 20", "-21 <= cap && cap <= -21"),
            ("0 <= limit && limit <= 20", "-21 <= limit && limit <= -21"),
            ("divisor <= 5", "divisor <= 1"),
            ("width <= 5", "width <= 1"),
            ("cap / divisor + 6", "(cap >> divisor) + 17"),
        ],
    );
    accepts_named(&signed);
    // `+ 16` puts the ceiling at `5`, which `pending = 5` violates; a
    // truncating model would reach `6` and accept, so this rejection is the
    // floor-division witness.
    rejects_named(&signed.replace("+ 17", "+ 16"));
    // The named-state transport keeps the same floor corner: a
    // nondecreasing cycle still rejects, and a count interval reaching the
    // `i8` width cannot prove `< 8`.
    rejects_named(&signed.replace("pending - 1", "pending"));
    rejects_named(&rewrite_fixture(
        &signed,
        &[
            ("divisor <= 1", "divisor <= 9"),
            ("width <= 1", "width <= 9"),
        ],
    ));
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
    rejects_any_stage(&rewrite_fixture(
        &source,
        &[("divisor: u64", "divisor: u8")],
    ));
}

/// A malformed endpoint can fail typing or checking; either rejection is the
/// closed behavior. Unlike [`rejects_named`] this makes no claim about which
/// stage owns the diagnostic.
fn rejects_any_stage(source: &str) {
    if let Ok(program) = typed_program_result(source) {
        lower_typed_trees(program, &CheckingRequest::settled()).expect_err(source);
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

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_rank_arithmetic/main.omg"
));

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
