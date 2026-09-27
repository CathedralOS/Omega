use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

const QUOTIENT_PAIR: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/quotient_endpoint_call_component/main.omg"
));

// These cases transform a corpus fixture. A missing anchor must fail here,
// rather than silently leaving a positive or negative case unchanged.
fn rewrite_fixture(source: &str, replacements: &[(&str, &str)]) -> String {
    replacements
        .iter()
        .fold(source.to_owned(), |source, (before, after)| {
            assert!(source.contains(before), "missing fixture anchor: {before}");
            source.replace(before, after)
        })
}

#[test]
fn recursive_quotient_endpoints_preserve_selection_and_formation() {
    for operation in ["/", "%"] {
        let source = QUOTIENT_PAIR.replace("/ 5", &format!("{operation} 5"));
        prove(&source);
        reject(&source.replace(&format!("{operation} 5"), &format!("{operation} 0")));
        reject(&format!(
            "operator {operation} u64::selected(left: u64, right: u64) -> u64; {source}"
        ));
        let signed_minimum = rewrite_fixture(
            &source,
            &[
                ("cap: u64", "cap: i8"),
                ("limit: u64", "limit: i8"),
                ("0 <= cap && cap <= 20", "-128 <= cap && cap <= -128"),
                (
                    "0 <= limit && limit <= 20",
                    "-128 <= limit && limit <= -128",
                ),
                (
                    &format!("cap {operation} 5 + 6"),
                    &format!("(cap {operation} -1) / 2 + 6"),
                ),
                (
                    &format!("limit {operation} 5 + 6"),
                    &format!("(limit {operation} -1) / 2 + 6"),
                ),
            ],
        );
        reject(&signed_minimum);
        prove(&rewrite_fixture(&signed_minimum, &[("-128", "-127")]));
    }
    reject(&QUOTIENT_PAIR.replace("/ 5", "/ 5u8"));
    let bounded = rewrite_fixture(
        QUOTIENT_PAIR,
        &[
            ("cap / 5", "(cap + 1) / 5"),
            ("limit / 5", "(limit + 1) / 5"),
        ],
    );
    prove(&bounded);
    reject(&rewrite_fixture(
        &bounded,
        &[(" && 0 <= cap && cap <= 20", "")],
    ));
}

#[test]
fn recursive_quotient_endpoints_follow_nested_terms_and_record_coordinates() {
    prove(
        &QUOTIENT_PAIR
            .replace("cap / 5", "(cap / 2) / 3")
            .replace("limit / 5", "(limit / 2) / 3"),
    );
    let source = format!(
        "data Limits {{ bound: u64 [0..=20]; other: u64 [0..=20]; }} {}",
        rewrite_fixture(
            QUOTIENT_PAIR,
            &[
                ("cap: u64", "cap: Limits"),
                ("limit: u64", "limit: Limits"),
                ("0 <= cap && cap <= 20", "0 <= cap.bound && cap.bound <= 20"),
                (
                    "0 <= limit && limit <= 20",
                    "0 <= limit.bound && limit.bound <= 20"
                ),
                ("cap / 5", "cap.bound / 5"),
                ("limit / 5", "limit.bound / 5"),
                (
                    "self.second(cap, remaining)",
                    "self.second(Limits { bound: cap.bound, other: 0 }, remaining)"
                ),
            ]
        )
    );
    prove(&source);
    prove(&source.replace(" in 0..(limit.bound / 5 + 6)", ""));
    reject(&source.replace("bound: cap.bound, other: 0", "bound: cap.other, other: 0"));
    let partial = format!(
        "data Limits {{ bound: u64 [0..=20]; other: u64 [0..=20]; }}
         data Envelope {{ inner: Limits; sibling: Limits; }} {}",
        rewrite_fixture(
            QUOTIENT_PAIR,
            &[
                ("cap: u64", "cap: Envelope"),
                ("limit: u64", "limit: Envelope"),
                (
                    "0 <= cap && cap <= 20",
                    "0 <= cap.inner.bound && cap.inner.bound <= 20"
                ),
                (
                    "0 <= limit && limit <= 20",
                    "0 <= limit.inner.bound && limit.inner.bound <= 20"
                ),
                ("cap / 5", "cap.inner.bound / 5"),
                ("limit / 5", "limit.inner.bound / 5"),
                (
                    "self.second(cap, remaining)",
                    "self.second(Envelope { inner: cap.inner, sibling: cap.sibling }, remaining)"
                ),
            ]
        )
    );
    for omitted in [
        " in 0..(cap.inner.bound / 5 + 6)",
        " in 0..(limit.inner.bound / 5 + 6)",
    ] {
        let mixed = partial.replace(omitted, "");
        prove(&mixed);
        reject(&mixed.replace("inner: cap.inner", "inner: cap.sibling"));
    }
}

const SHIFT_ACTUAL: &str = r#"
data Main { observed: u64; }

machine Main::main(&mut self) {}

machine Main::first(&mut self, remaining: u64 [0..=5], cap: u64 [0..=20], step: u64 [1..=5], spare: u64)
terminates by remaining in 0..(cap / step + 6);
-> u64 {
    transition remaining > 0 {
        true -> self.second(cap, remaining - 1, step, cap << step)
        false -> remaining
    }
}

machine Main::second(&mut self, limit: u64 [0..=20], pending: u64 [0..=5], width: u64 [1..=5], extra: u64)
requires extra <= 640;
terminates by pending in 0..(limit / width + 6);
-> u64 {
    transition pending > 0 {
        true -> self.first(pending, limit, width, extra)
        false -> pending
    }
}
"#;

#[test]
fn runtime_shift_actuals_substitute_through_call_sites() {
    // A left shift is an exact non-polynomial term like division: `spare`
    // rides outside every endpoint atom while the transported actual keeps
    // the exact `cap`/`step` operands, and the callee's
    // `requires extra <= 640` discharges the shift's live interval
    // `[0, 20] * 2^[1, 5]` rather than the formal's name.
    prove(SHIFT_ACTUAL);
    // A nested non-polynomial actual transports innermost-first: the
    // remainder's tight interval is already live when the enclosing shift
    // mints.
    prove(&SHIFT_ACTUAL.replace("cap << step)", "(cap % step) << step)"));
    // A spelled zero count is defined -- `value << 0` is the value -- where
    // the same spelled expression as a modulus would reject.
    prove(&SHIFT_ACTUAL.replace("cap << step)", "cap << (step - step))"));
    // An out-of-width or unproven count never produces a value: the
    // actual's own formation rejects it, independently of the call-range
    // substitution it would have fed.
    for source in [
        SHIFT_ACTUAL.replace("cap << step)", "cap << 64)"),
        SHIFT_ACTUAL.replace("cap << step)", "cap << (step - 6))"),
        SHIFT_ACTUAL.replace("step: u64 [1..=5]", "step: u64 [1..=70]"),
    ] {
        let diagnostics = lower_typed_trees(typed_program(&source), &CheckingRequest::settled())
            .expect_err(&source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("shift count")),
            "{source}\n{diagnostics:#?}"
        );
    }
    // Transporting a shift into the endpoint-read `limit` slot changes the
    // ceiling: `(step << 1) / width` is not `cap / step`.
    reject(&SHIFT_ACTUAL.replace(
        "self.second(cap, remaining - 1, step, cap << step)",
        "self.second(step << 1, remaining - 1, step, cap)",
    ));
    // The callee's requirement is a real obligation discharged against the
    // substituted actual, not the formal name: moving the bound it reads
    // past the transported shift's interval rejects the call, and the
    // diagnostic names the transported `cap << step` term.
    for bound in ["requires extra <= 639;", "requires extra <= width - 1;"] {
        let tightened = SHIFT_ACTUAL.replace("requires extra <= 640;", bound);
        let diagnostics = lower_typed_trees(typed_program(&tightened), &CheckingRequest::settled())
            .expect_err(&tightened);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires")
                && diagnostic.message.contains("cap << step")),
            "{tightened}\n{diagnostics:#?}"
        );
    }
}

const SHIFT_RIGHT_ACTUAL: &str = r#"
data Main { observed: u64; }

machine Main::main(&mut self) {}

machine Main::first(&mut self, remaining: u64 [0..=5], cap: u64 [0..=20], step: u64 [1..=5], spare: u64)
terminates by remaining in 0..(cap / step + 6);
-> u64 {
    transition remaining > 0 {
        true -> self.second(cap, remaining - 1, step, cap >> step)
        false -> remaining
    }
}

machine Main::second(&mut self, limit: u64 [0..=20], pending: u64 [0..=5], width: u64 [1..=5], extra: u64)
requires extra <= 10;
terminates by pending in 0..(limit / width + 6);
-> u64 {
    transition pending > 0 {
        true -> self.first(pending, limit, width, extra)
        false -> pending
    }
}
"#;

#[test]
fn runtime_shift_right_actuals_substitute_through_call_sites() {
    // A right shift transports like a left shift: `cap >> step` keeps its
    // exact operands, and the callee's `requires extra <= 10` discharges the
    // shift's own live interval `[0, 20] >> [1, 5]` = `[0, 10]` -- not the
    // operand's `[0, 20]` and not a `<<` growth bound.
    prove(SHIFT_RIGHT_ACTUAL);
    // Nested non-polynomial actuals transport innermost-first, including a
    // `<<` operand inside the `>>` value: `(cap << step) >> step` carries
    // `[0, 640] >> [1, 5]` = `[0, 320]`, discharging `<= 320` exactly at its
    // corner and failing one tighter with both operations named.
    prove(&SHIFT_RIGHT_ACTUAL.replace("cap >> step)", "(cap % step) >> step)"));
    let nested = SHIFT_RIGHT_ACTUAL
        .replace("cap >> step)", "(cap << step) >> step)")
        .replace("requires extra <= 10;", "requires extra <= 320;");
    prove(&nested);
    let tightened = nested.replace("requires extra <= 320;", "requires extra <= 319;");
    let diagnostics = lower_typed_trees(typed_program(&tightened), &CheckingRequest::settled())
        .expect_err(&tightened);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("cannot prove requires")
                && diagnostic.message.contains("cap << step >> step")
        ),
        "{tightened}\n{diagnostics:#?}"
    );
    // A spelled zero count is defined -- `value >> 0` is the value -- so
    // the transported term then carries the operand's own `[0, 20]`.
    prove(
        &SHIFT_RIGHT_ACTUAL
            .replace("cap >> step)", "cap >> (step - step))")
            .replace("requires extra <= 10;", "requires extra <= 20;"),
    );
    // An out-of-width or unproven count never produces a value, exactly as
    // for `<<`: the actual's own formation rejects it, independently of the
    // call-range substitution it would have fed.
    for source in [
        SHIFT_RIGHT_ACTUAL.replace("cap >> step)", "cap >> 64)"),
        SHIFT_RIGHT_ACTUAL.replace("cap >> step)", "cap >> (step - 6))"),
        SHIFT_RIGHT_ACTUAL.replace("step: u64 [1..=5]", "step: u64 [1..=70]"),
    ] {
        let diagnostics = lower_typed_trees(typed_program(&source), &CheckingRequest::settled())
            .expect_err(&source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("shift count")),
            "{source}\n{diagnostics:#?}"
        );
    }
    // Transporting a shift into the endpoint-read `limit` slot changes the
    // ceiling: `(cap >> step) / width` is not `cap / step`.
    reject(&SHIFT_RIGHT_ACTUAL.replace(
        "self.second(cap, remaining - 1, step, cap >> step)",
        "self.second(cap >> step, remaining - 1, step, cap)",
    ));
    // The callee's requirement is discharged against the substituted
    // actual: `cap >> step` proves `<= 10` where the operand alone would
    // only prove `<= 20`, and a bound one tighter rejects with the
    // transported term named.
    for bound in ["requires extra <= 9;", "requires extra <= width - 1;"] {
        let tightened = SHIFT_RIGHT_ACTUAL.replace("requires extra <= 10;", bound);
        let diagnostics = lower_typed_trees(typed_program(&tightened), &CheckingRequest::settled())
            .expect_err(&tightened);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires")
                && diagnostic.message.contains("cap >> step")),
            "{tightened}\n{diagnostics:#?}"
        );
    }
}

const BITWISE_AND_SIGNED: &str = r#"
data Main { observed: i64; }

machine Main::main(&mut self) {}

machine Main::first(&mut self, remaining: u64 [0..=5], level: i64 [-21..=-1], mask: i64 [1..=5], spare: i64)
terminates by remaining in 0..(level + 30);
-> u64 {
    transition remaining > 0 {
        true -> self.second(level, remaining - 1, mask, level & mask)
        false -> remaining
    }
}

machine Main::second(&mut self, depth: i64 [-21..=-1], pending: u64 [0..=5], wall: i64 [1..=5], extra: i64)
requires extra >= 0 && extra <= 5;
terminates by pending in 0..(depth + 30);
-> u64 {
    transition pending > 0 {
        true -> self.first(pending, depth, wall, extra)
        false -> pending
    }
}
"#;

#[test]
fn signed_bitwise_and_actuals_use_mask_sign_bounds_through_call_sites() {
    // `level & mask` with `level` provably negative and `mask` provably
    // nonnegative lands `[0, 5]`: the mask's clear sign bit bounds the
    // result below itself and above zero, so `extra >= 0` discharges where
    // the transported `level` alone never could.
    prove(BITWISE_AND_SIGNED);
    // Either tightened side rejects against the transported term: the mask
    // quadrant leaves `0` reachable below `>= 1` and `5` above `<= 4`, and
    // each diagnostic names the `level & mask` actual.
    for bound in ["requires extra >= 1;", "requires extra <= 4;"] {
        let tightened = BITWISE_AND_SIGNED.replace("requires extra >= 0 && extra <= 5;", bound);
        let diagnostics = lower_typed_trees(typed_program(&tightened), &CheckingRequest::settled())
            .expect_err(&tightened);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires")
                && diagnostic.message.contains("level & mask")),
            "{tightened}\n{diagnostics:#?}"
        );
    }
    // The cycle still owes strict descent on its carried rank.
    reject(&BITWISE_AND_SIGNED.replace("remaining - 1", "remaining"));
}

const REMAINDER_LITERAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/remainder_literal_call_component/main.omg"
));

#[test]
fn record_literal_remainder_leaves_discharge_field_requirements() {
    // The `spare` leaf of the rebuilt `Limits` literal is the runtime
    // remainder `limits.bound % limits.step`: `walk`'s
    // `requires held.spare <= 4` is discharged against that transported
    // term through the record actual, not the destination formal's name.
    prove(REMAINDER_LITERAL);
    // A quotient leaf keeps the same path while its interval stays inside
    // the requirement.
    prove(&REMAINDER_LITERAL.replace("% limits.step", "/ 5"));
    // Rebinding `spare` to a different transported value still discharges,
    // but only when the requirement can be proven against it: each changed
    // leaf rejects and the diagnostic names the transported term.
    for leaf in ["limits.bound / limits.step", "limits.bound % 6"] {
        let source = REMAINDER_LITERAL.replace("limits.bound % limits.step", leaf);
        let diagnostics =
            lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).expect_err(leaf);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires")
                && diagnostic.message.contains(leaf)),
            "{leaf}\n{diagnostics:#?}"
        );
    }
    // A requirement tighter than the remainder's live interval rejects the
    // same way: `limits.bound % limits.step` only proves `<= 4`.
    let tightened =
        REMAINDER_LITERAL.replace("requires held.spare <= 4;", "requires held.spare <= 3;");
    let diagnostics = lower_typed_trees(typed_program(&tightened), &CheckingRequest::settled())
        .expect_err(&tightened);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("cannot prove requires")
                && diagnostic.message.contains("limits.bound % limits.step")
        ),
        "{tightened}\n{diagnostics:#?}"
    );
    // The cycle still owes strict descent on its carried rank.
    reject(&REMAINDER_LITERAL.replace("pending - 1", "pending"));
}

const SHIFT_LITERAL: &str = r#"
data Limits {
    bound: u64 [0..=20];
    spare: u64;
    step: u64 [1..=5];
}

data Main { pad: u64; }

machine Main::main(&mut self) {}

machine Main::walk(&mut self, remaining: u64 [0..=5], held: Limits)
requires held.spare <= 640;
terminates by remaining in 0..(held.bound + 6);
-> u64 {
    transition remaining > 0 {
        true -> self.step(remaining, held)
        false -> remaining
    }
}

machine Main::step(&mut self, pending: u64 [0..=5], limits: Limits)
requires limits.spare <= 640;
terminates by pending in 0..(limits.bound + 6);
-> u64 {
    transition pending > 0 {
        true -> self.walk(pending - 1, Limits {
            bound: limits.bound,
            spare: limits.bound << limits.step,
            step: limits.step
        })
        false -> pending
    }
}
"#;

#[test]
fn record_literal_shift_leaves_discharge_field_requirements() {
    // The `spare` leaf of the rebuilt `Limits` literal is the runtime left
    // shift `limits.bound << limits.step`: `walk`'s
    // `requires held.spare <= 640` is discharged against that transported
    // term through the record actual, not the destination formal's name.
    prove(SHIFT_LITERAL);
    // A `>>` leaf transports the same way at its own tighter interval
    // `[0, 10]`: `<= 10` discharges and `<= 9` rejects.
    prove(
        &SHIFT_LITERAL
            .replace("limits.bound << limits.step", "limits.bound >> limits.step")
            .replace("requires held.spare <= 640;", "requires held.spare <= 10;")
            .replace(
                "requires limits.spare <= 640;",
                "requires limits.spare <= 10;",
            ),
    );
    let right_tightened = SHIFT_LITERAL
        .replace("limits.bound << limits.step", "limits.bound >> limits.step")
        .replace("requires held.spare <= 640;", "requires held.spare <= 9;")
        .replace(
            "requires limits.spare <= 640;",
            "requires limits.spare <= 9;",
        );
    let diagnostics =
        lower_typed_trees(typed_program(&right_tightened), &CheckingRequest::settled())
            .expect_err(&right_tightened);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("cannot prove requires")
                && diagnostic.message.contains("limits.bound >> limits.step")
        ),
        "{right_tightened}\n{diagnostics:#?}"
    );
    // A requirement tighter than the shift's live interval rejects the same
    // way: `limits.bound << limits.step` only proves `<= 640`.
    let tightened =
        SHIFT_LITERAL.replace("requires held.spare <= 640;", "requires held.spare <= 639;");
    let diagnostics = lower_typed_trees(typed_program(&tightened), &CheckingRequest::settled())
        .expect_err(&tightened);
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("cannot prove requires")
                && diagnostic.message.contains("limits.bound << limits.step")
        ),
        "{tightened}\n{diagnostics:#?}"
    );
    // Rebinding `spare` to a leaf whose interval outgrows the requirement
    // rejects the discharge, and an out-of-width count fails the leaf's own
    // formation.
    let outgrown = SHIFT_LITERAL.replace(
        "limits.bound << limits.step",
        "limits.bound << (limits.step + 2)",
    );
    let diagnostics = lower_typed_trees(typed_program(&outgrown), &CheckingRequest::settled())
        .expect_err(&outgrown);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{outgrown}\n{diagnostics:#?}"
    );
    // An out-of-width count fails the leaf's mint the same way a spelled
    // `limits.bound % 0` fails a remainder leaf: the term stays unobserved
    // and the discharge reports the unmet fact.
    let out_of_width = SHIFT_LITERAL.replace("limits.bound << limits.step", "limits.bound << 64");
    let diagnostics = lower_typed_trees(typed_program(&out_of_width), &CheckingRequest::settled())
        .expect_err(&out_of_width);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove requires")),
        "{out_of_width}\n{diagnostics:#?}"
    );
    // The cycle still owes strict descent on its carried rank.
    reject(&SHIFT_LITERAL.replace("pending - 1", "pending"));
}

const PAIR: &str = r#"
data Main { observed: u64; }
machine Main::first(&mut self, floor: u64, remaining: u64, ceiling: u64)
requires floor <= remaining && remaining <= ceiling;
terminates by remaining in floor..=ceiling;
-> u64 {
    transition remaining > floor {
        true -> self.second(ceiling, remaining, floor)
        false -> remaining
    }
}
machine Main::second(&mut self, upper: u64, pending: u64, lower: u64)
requires lower <= pending && pending <= upper;
terminates by pending in lower..=upper;
-> u64 {
    transition pending > lower {
        true -> self.first(lower, pending - 1, upper)
        false -> pending
    }
}
"#;

#[test]
fn symbolic_division_endpoints_cross_recursive_components() {
    let source = rewrite_fixture(
        QUOTIENT_PAIR,
        &[
            ("cap: u64)", "cap: u64, divisor: u64)"),
            ("pending: u64)", "pending: u64, width: u64)"),
            ("cap <= 20", "cap <= 20 && 1 <= divisor && divisor <= 5"),
            ("pending <= 5", "pending <= 5 && 1 <= width && width <= 5"),
            ("cap / 5", "cap / divisor"),
            ("limit / 5", "limit / width"),
            (
                "self.second(cap, remaining)",
                "self.second(cap, remaining, divisor)",
            ),
            (
                "self.first(pending - 1, limit)",
                "self.first(pending - 1, limit, width)",
            ),
        ],
    );
    for operator in ["/", "%"] {
        let source = source.replace(" / ", &format!(" {operator} "));
        for omitted in [
            "",
            " in 0..(cap / divisor + 6)",
            " in 0..(limit / width + 6)",
        ] {
            let source = if omitted.is_empty() {
                source.clone()
            } else {
                source.replace(&omitted.replace(" / ", &format!(" {operator} ")), "")
            };
            prove(&source);
            reject(&source.replace("remaining, divisor)", "remaining, 1)"));
            reject(&source.replace("limit, width)", "limit, 1)"));
            reject(&source.replace("pending - 1", "pending"));
        }
        reject(&rewrite_fixture(
            &source,
            &[
                ("1 <= divisor", "0 <= divisor"),
                ("1 <= width", "0 <= width"),
            ],
        ));
    }
}

fn prove(source: &str) {
    lower_typed_trees(typed_program(source), &CheckingRequest::settled())
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject(source: &str) {
    let Err(diagnostics) = lower_typed_trees(typed_program(source), &CheckingRequest::settled())
    else {
        panic!("invalid call component accepted:\n{source}");
    };
    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message.contains("machine call cycle")
                || diagnostic.message.contains("cannot prove rank range")
                || diagnostic
                    .message
                    .contains("cannot prove the `terminates by` ranking")
        ),
        "{source}\n{diagnostics:#?}"
    );
}

#[test]
fn prefix_let_call_bindings_keep_the_call_frame_bar() {
    // A `let` bound to a checked-body call keeps the statement-call bar: the
    // fresh local cannot name a premise carrier, so the initializer's
    // complete write frame must still miss every protected carrier. The bar
    // covers the whole initializer tree -- a nested call argument and a
    // builtin-composed initializer each name only checked-body callees with
    // pure subterms, so the aggregate frame still sees every call's writes.
    let helpers = "machine Main::recall(&self, value: u64) -> u64 {
    transition { _ -> value }
}
machine Main::bump(&mut self, slot: &mut u64) -> u64 {
    slot = 0;
    transition { _ -> 1 }
}
";
    let bound = format!("{helpers}{PAIR}");
    prove(&bound.replace(
        "    transition remaining > floor",
        "    let seen: u64 = self.recall(remaining);\n    transition remaining > floor",
    ));
    prove(&bound.replace(
        "    transition remaining > floor",
        "    let seen: u64 = self.recall(self.recall(remaining));\n    transition remaining > floor",
    ));
    prove(&bound.replace(
        "    transition remaining > floor",
        "    let seen: u64 = self.recall(remaining) + 0;\n    transition remaining > floor",
    ));
    // A signature-only callee answers through its selected contract instead
    // of a body summary: a public boundary requirement satisfied by a checked
    // provider may execute, and its declared exclusive reach is the whole
    // frame. A by-value read of a premise carrier writes nothing and keeps
    // the telescope; the same call spelling behind an exclusive parameter
    // writes the borrowed carrier and rejects.
    let requirement = "pub data Ops {}
pub boundary requirement Ops::note(value: u64) -> u64;
data OpsProvider {}
machine OpsProvider::note_impl(input: u64) -> u64
satisfies Ops::note
{
    transition { _ -> input }
}
";
    prove(&format!("{requirement}{helpers}{PAIR}").replace(
        "    transition remaining > floor",
        "    let seen: u64 = Ops::note(remaining);\n    transition remaining > floor",
    ));
    // The same contract answers at statement position: a value-only
    // requirement call writes no caller place, so the prefix stays provable.
    prove(&format!("{requirement}{helpers}{PAIR}").replace(
        "    transition remaining > floor",
        "    _ = Ops::note(remaining);\n    transition remaining > floor",
    ));
    let writing = requirement
        .replace("note(value: u64)", "note(value: &mut u64)")
        .replace(
            "note_impl(input: u64) -> u64",
            "note_impl(input: &mut u64) -> u64",
        );
    let writing = writing.replace(
        "transition { _ -> input }",
        "input = input + 0; transition { _ -> 0 }",
    );
    reject(
        &format!("{writing}{helpers}{PAIR}")
            .replace(
                "    transition remaining > floor",
                "    let seen: u64 = Ops::note(&mut remaining);\n    transition remaining > floor",
            )
            .replace("remaining: u64", "mut remaining: u64"),
    );
    reject(
        &bound
            .replace("remaining: u64", "mut remaining: u64")
            .replace(
                "    transition remaining > floor",
                "    let seen: u64 = self.bump(&mut remaining);\n    transition remaining > floor",
            ),
    );
}

fn without_ranges(source: &str) -> String {
    source
        .replace(" in floor..=ceiling", "")
        .replace(" in lower..=upper", "")
}

/// A call component reads the nested carriage too: `drain`'s `bag.items`
/// rank enters `hold`'s `pair` at its `bag` field, and `step` returns it to
/// `drain`'s own `bag` formal through a rebuilt literal. The cycle's one
/// strict subslice is the descent the component owes.
const NESTED_SLICE: &str = r#"
data Bag { items: &[u32]; }
data Pair { bag: Bag; }
data Main { pad: u64; }

machine Main::main(&mut self) -> u64 {
    let bag: Bag = Bag { items: &[7, 8, 9] };
    transition {
        _ -> self.drain(bag)
    }
}

machine Main::drain(&mut self, bag: Bag)
terminates by bag.items -> Slice::Length;
-> u64 {
    transition bag.items.len > 0 {
        true -> hold(Pair { bag: bag })
        false -> 0
    }
    state hold(pair: Pair) {
        transition pair.bag.items.len > 0 {
            true -> self.step(pair)
            false -> 0
        }
    }
}

machine Main::step(&mut self, pair: Pair)
terminates by pair.bag.items -> Slice::Length;
-> u64 {
    transition pair.bag.items.len > 0 {
        true -> self.drain(Bag { items: pair.bag.items[1..] })
        false -> 0
    }
}
"#;

#[test]
fn nested_slice_carriers_transport_through_the_call_component() {
    prove(NESTED_SLICE);
    // Forwarding the nested carriage is a move, not a descent: the cycle
    // keeps the produced length and no complete pass decreases it.
    reject(&NESTED_SLICE.replace(
        "self.drain(Bag { items: pair.bag.items[1..] })",
        "self.drain(pair.bag)",
    ));
    // An ambiguous nested carrier -- `pair` offers two `Bag` paths to the
    // `bag` role -- forms no coordinate rather than picking one.
    reject(
        &NESTED_SLICE
            .replace(
                "data Pair { bag: Bag; }",
                "data Pair { left: Bag; right: Bag; }",
            )
            .replace(
                "drain(&mut self, bag: Bag)",
                "drain(&mut self, bag: Bag, other: Bag)",
            )
            .replace("self.drain(bag)", "self.drain(bag, bag)")
            .replace("Pair { bag: bag }", "Pair { left: bag, right: other }")
            .replace("pair.bag.items", "pair.left.items")
            .replace(
                "self.drain(Bag { items: pair.left.items[1..] })",
                "self.drain(Bag { items: pair.left.items[1..] }, pair.right)",
            ),
    );
}

#[test]
fn unranged_natural_call_guards_preserve_boolean_wrapper_orientation() {
    let source = without_ranges(PAIR);
    for guard in [
        "pending > lower",
        "(pending > lower) == true",
        "true == (pending > lower)",
        "(pending > lower) != false",
        "false != (pending > lower)",
    ] {
        prove(&source.replace("transition pending > lower", &format!("transition {guard}")));
    }
    for guard in [
        "(pending > lower) == false",
        "false == (pending > lower)",
        "(pending > lower) != true",
        "true != (pending > lower)",
    ] {
        reject(&source.replace("transition pending > lower", &format!("transition {guard}")));
    }
}

const STATEFUL: &str = r#"
data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining in 0..=9;
-> u32 {
    transition remaining > 0 { true -> hold(remaining) false -> remaining }
    state hold(pending: u32 [0..=9]) {
        transition pending > 1 { true -> hold(pending - 1) false -> self.step(pending) }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u32 {
    transition n > 0 { true -> self.count(n - 1) false -> n }
}
"#;

#[test]
fn component_members_keep_internal_cycles_under_the_local_ranking_rule() {
    // The whole-component judgment witnesses cross-machine call edges only;
    // every internal arrival still answers to the member's own ranking
    // judgment. A strictly decreasing internal loop beside the weak
    // `hold -> step` call edge is admitted.
    prove(STATEFUL);
    // A member that can loop internally without descent is rejected even
    // though the component's call edges are well-formed: `hold(pending)`
    // arrives with the rank unchanged, and `self.count(pending)` re-enters
    // the entry without descent.
    reject(&STATEFUL.replace("hold(pending - 1)", "hold(pending)"));
    reject(&STATEFUL.replace(
        "false -> self.step(pending)",
        "false -> self.count(pending)",
    ));
    // An unranged member is held to the same local rule: its internal loop
    // cannot borrow the component's transported entry telescope, so even the
    // decreasing arrival stays unproven, matching the standalone judgment.
    let unranged = STATEFUL.replace(" in 0..=9", "");
    reject(&unranged);
    reject(&unranged.replace("hold(pending - 1)", "hold(pending)"));
}

const DUPLICATED: &str = r#"
data Main {}
machine Main::count(&mut self, remaining: u32 [0..=9])
terminates by remaining in 0..=9;
-> u32 {
    transition remaining > 0 { true -> pair(remaining, remaining) false -> remaining }
    state pair(left: u32 [0..=9], right: u32 [0..=9]) {
        transition left > 0 { true -> self.step(right) false -> left }
    }
}
machine Main::step(&mut self, n: u32 [0..=9])
terminates by n in 0..=9;
-> u32 {
    transition n > 0 { true -> self.count(n - 1) false -> n }
}
"#;

#[test]
fn component_calls_carry_rank_through_duplicated_arrival_copies() {
    // `pair(left, right)` holds two copies of `remaining`; discovery kept both
    // claims because every arrival forwarded a bare name, so the copies are
    // equal and the call may read either carrier.
    prove(DUPLICATED);
    // `remaining - 1` names `right` as the moved copy of `remaining`; the
    // stale `left` demotes and `step(right)` transports the descended copy,
    // so the component still terminates.
    prove(&DUPLICATED.replace(
        "pair(remaining, remaining)",
        "pair(remaining, remaining - 1)",
    ));
    // The stale copy cannot stand in for the rank: once `left` demotes,
    // `step(left)` hands the callee a slot carrying no `remaining` role.
    reject(
        &DUPLICATED
            .replace(
                "pair(remaining, remaining)",
                "pair(remaining, remaining - 1)",
            )
            .replace("self.step(right)", "self.step(left)"),
    );
    reject(&DUPLICATED.replace("self.step(right)", "self.step(right + 1)"));
    // An intervening write to a carrier invalidates the copied premise.
    reject(
        &DUPLICATED
            .replace("right: u32 [0..=9]", "mut right: u32 [0..=9]")
            .replace(
                "        transition left > 0 { true -> self.step(right)",
                "        right = 0; transition left > 0 { true -> self.step(right)",
            ),
    );
}

fn reject_range(source: &str) {
    let diagnostics =
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).expect_err(source);
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.message.contains("cannot prove rank range")
                || diagnostic.message.contains("machine call cycle")
        }),
        "{source}\n{diagnostics:#?}"
    );
}

fn reject_requires(source: &str) {
    let diagnostics =
        lower_typed_trees(typed_program(source), &CheckingRequest::settled()).expect_err(source);
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove requires contract")),
        "{source}\n{diagnostics:#?}"
    );
}

const SLICE_STATEFUL: &str = r#"
data Entry { value: u32; }
data Main {}
machine Main::scan(&mut self, entries: &[Entry], capacity: u64)
requires entries.len <= capacity;
terminates by entries -> Slice::Length in 0..=capacity;
-> u64 {
    transition entries.len > 0 { true -> hold(entries, capacity) false -> 0 }
    state hold(pending: &[Entry], bound: u64) {
        transition pending.len > 0 {
            true -> self.step(pending, bound)
            false -> 0
        }
    }
}
machine Main::step(&mut self, rest: &[Entry], capacity: u64)
terminates by rest -> Slice::Length;
-> u64 {
    transition rest.len > 0 && rest.len <= capacity {
        true -> self.scan(rest[1..], capacity)
        false -> 0
    }
}
"#;

#[test]
fn ranged_slice_member_calls_from_a_subordinate_state_under_its_own_invariant() {
    // `hold -> step(pending, bound)` forwards the carried collection and the
    // authored ceiling; the site reads `scan`'s range invariant
    // `pending.len <= bound` from the arrival rather than a respelled guard.
    prove(SLICE_STATEFUL);
    prove(&SLICE_STATEFUL.replace("pending.len > 0", "pending.len > 0 && pending.len <= bound"));
    // A changed endpoint, an intervening carrier write, an arrival that does
    // not pin the ceiling, and a missing entry premise keep rejecting.
    reject(&SLICE_STATEFUL.replace("self.step(pending, bound)", "self.step(pending, bound + 1)"));
    reject(&SLICE_STATEFUL.replace(
        "self.step(pending, bound)",
        "self.step(pending, pending.len)",
    ));
    reject(
        &SLICE_STATEFUL
            .replace(
                "pending: &[Entry], bound: u64)",
                "pending: &[Entry], mut bound: u64)",
            )
            .replace(
                "        transition pending.len > 0",
                "        bound = bound; transition pending.len > 0",
            ),
    );
    reject_range(
        &SLICE_STATEFUL
            .replace(
                "transition entries.len > 0 {",
                "transition entries.len > 0 && capacity > 0 {",
            )
            .replace("hold(entries, capacity)", "hold(entries, capacity - 1)"),
    );
    reject(&SLICE_STATEFUL.replace("requires entries.len <= capacity;\n", ""));
    // A ranged callee's public `requires` is discharged by the caller
    // member's own proven invariant: the carried collection's produced
    // length is the rank, so `pending.len <= bound` is established site
    // evidence for `rest.len <= capacity`.
    let public = SLICE_STATEFUL
        .replace(
            "terminates by rest -> Slice::Length;",
            "requires rest.len <= capacity;\nterminates by rest -> Slice::Length in 0..=capacity;",
        )
        .replace("rest.len > 0 && rest.len <= capacity", "rest.len > 0");
    prove(&public);
    prove(&public.replace("pending.len > 0", "pending.len > 0 && pending.len <= bound"));
}

const FIELD_SUBORDINATE_REQUIRES: &str = r#"
data Countdown {
    remaining: u64 [0..=9];
    limit: u64 [0..=9];
}

measure Countdown::Remaining(countdown: Countdown) -> u64 { countdown.remaining }

data Main {}

machine Main::main(&mut self) -> u64 {
    transition { _ -> self.outer(Countdown { remaining: 4, limit: 6 }) }
}

machine Main::outer(&mut self, countdown: Countdown)
requires countdown.remaining <= countdown.limit;
terminates by countdown -> Countdown::Remaining in 0..=countdown.limit;
-> u64 {
    transition countdown.remaining > 0 {
        true -> hold(countdown)
        false -> countdown.remaining
    }
    state hold(pending: Countdown) {
        transition pending.remaining > 0 {
            true -> self.inner(pending)
            false -> pending.remaining
        }
    }
}

machine Main::inner(&mut self, current: Countdown)
requires current.remaining <= current.limit;
terminates by current -> Countdown::Remaining in 0..=current.limit;
-> u64 {
    transition current.remaining > 0 {
        true -> self.outer(Countdown { remaining: current.remaining - 1, limit: current.limit })
        false -> current.remaining
    }
}
"#;

#[test]
fn field_measure_member_reads_its_proven_invariant_at_a_subordinate_call() {
    // `hold(countdown)` carries the ranked record whole: the site reads
    // `outer`'s proven invariant `pending.remaining <= pending.limit` through
    // the telescoped field coordinate, discharging `inner`'s public requires
    // without a respelled guard.
    prove(FIELD_SUBORDINATE_REQUIRES);
    prove(&FIELD_SUBORDINATE_REQUIRES.replace(
        "pending.remaining > 0",
        "pending.remaining > 0 && pending.remaining <= pending.limit",
    ));
    // The invariant supplies exactly the membership it proves: a strictly
    // stronger requires, or a conjunct the carried facts do not establish,
    // still has no site evidence.
    reject_requires(&FIELD_SUBORDINATE_REQUIRES.replace(
        "requires current.remaining <= current.limit;",
        "requires current.remaining < current.limit;",
    ));
    reject_requires(&FIELD_SUBORDINATE_REQUIRES.replace(
        "requires current.remaining <= current.limit;",
        "requires current.remaining <= current.limit && current.limit <= 5;",
    ));
    // A decreasing internal arrival keeps the endpoint pinned, so the
    // re-established invariant still discharges the same requires.
    prove(&FIELD_SUBORDINATE_REQUIRES.replace(
        "true -> hold(countdown)",
        "true -> hold(Countdown { remaining: countdown.remaining - 1, limit: countdown.limit })",
    ));
    // An intervening write to the record carrier invalidates the premise the
    // invariant was read from.
    reject(
        &FIELD_SUBORDINATE_REQUIRES
            .replace("state hold(pending: Countdown)", "state hold(mut pending: Countdown)")
            .replace(
                "        transition pending.remaining > 0",
                "        pending = Countdown { remaining: 0, limit: 0 }; transition pending.remaining > 0",
            ),
    );
    // An arrival whose rebuilt record moves the authored endpoint fails the
    // member's own state-edge judgment rather than feeding a stale premise:
    // `pending.limit` there is `countdown.remaining - 1`, not the pinned
    // `countdown.limit`.
    reject(&FIELD_SUBORDINATE_REQUIRES.replace(
        "true -> hold(countdown)",
        "true -> hold(Countdown { remaining: countdown.remaining, limit: countdown.remaining - 1 })",
    ));
}

#[test]
fn mixed_endpoint_arithmetic_equality_uses_only_live_caller_premises() {
    let source = PAIR
        .replace(" in lower..=upper", "")
        .replace("ceiling: u64)", "ceiling: u64, shift: u64)")
        .replace(
            "ceiling, remaining, floor)",
            "ceiling + shift, remaining, floor)",
        )
        .replace(
            "lower, pending - 1, upper)",
            "lower, pending - 1, upper, 0)",
        );
    let required = source.replace(
        "requires floor <= remaining",
        "requires shift == 0 && floor <= remaining",
    );
    prove(&required);
    let guarded = source.replace(
        "transition remaining > floor",
        "transition remaining > floor && shift == 0",
    );
    prove(&guarded);
    reject(&guarded.replace("shift == 0", "shift != 0"));
    reject(&source);
    reject(&required.replace("shift: u64", "mut shift: u64").replace(
        "    transition remaining > floor",
        "    shift = 1; transition remaining > floor",
    ));
}
