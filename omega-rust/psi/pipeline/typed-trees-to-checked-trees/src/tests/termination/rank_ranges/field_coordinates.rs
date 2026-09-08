use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_pinned_limit/main.omg"
));
const PRECONDITION: &str = "requires countdown.remaining <= countdown.limit;";
const NEXT_LIMIT: &str = "limit: countdown.limit";

fn prove_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source))
        .unwrap_or_else(|diagnostics| panic!("{source}\n{diagnostics:#?}"));
}

fn reject_range(source: &str) {
    let diagnostics = crate::checks::termination::check_machine_termination(&typed(source))
        .expect_err("the exact field endpoint must remain proved and pinned");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove rank range")),
        "{source}\n{diagnostics:#?}"
    );
}

fn reject_termination(source: &str) {
    crate::checks::termination::check_machine_termination(&typed(source)).expect_err(source);
}

fn second_record() -> String {
    COUNTDOWN
        .replace(
            "machine walk(countdown: Countdown)",
            "machine walk(countdown: Countdown, bounds: Countdown)",
        )
        .replace("<= countdown.limit;", "<= bounds.limit;")
        .replace("in 0..=countdown.limit;", "in 0..=bounds.limit;")
        .replace("        })", "        }, bounds)")
}

#[test]
fn pinned_field_customer_checks_through_complete_lowering() {
    lower_typed_trees(typed(COUNTDOWN))
        .expect("entry membership and the reconstructed remaining and limit fields all check");
}

#[test]
fn ordinary_field_requirements_do_not_depend_on_a_ranking_witness() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../tests/omega/fail/termination/measure_field_arrival_contract/main.omg"
    ));
    for source in [
        source.to_owned(),
        source
            .replace(
                "machine walk(countdown: Countdown)",
                "machine walk(countdown: Countdown) -> u64",
            )
            .replace("-> u64 {\n    transition", "{\n    transition")
            .replace(
                "terminates by countdown -> Countdown::Remaining in 0..=countdown.limit;",
                "",
            ),
    ] {
        lower_typed_trees(typed(&source.replace("spare: 0", "spare: countdown.spare")))
            .expect("simultaneous field substitution preserves the entry requirement");
        let diagnostics = lower_typed_trees(typed(&source))
            .expect_err("an invalid entry requirement rejects with or without a rank");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires contract for call walk")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn ordinary_field_requirements_keep_caller_and_callee_coordinates_separate() {
    let source = r#"
        data Bounds { value: u64 [0..=5]; }
        machine consume(first: Bounds, second: Bounds) -> u64
        requires first.value <= second.value;
        { 0 }
        machine caller(first: Bounds, second: Bounds) -> u64
        requires second.value <= first.value;
        { consume(second, first) }
    "#;
    lower_typed_trees(typed(source))
        .expect("actuals map simultaneously despite reversed same-named formals");
    for arguments in ["first, second", "Bounds { value: 5 }, Bounds { value: 0 }"] {
        let changed = source.replace("consume(second, first)", &format!("consume({arguments})"));
        let diagnostics = lower_typed_trees(typed(&changed))
            .expect_err("neither caller names nor an invalid constructor supplies the callee fact");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("cannot prove requires contract for call consume")),
            "{diagnostics:#?}"
        );
    }
}

#[test]
fn reconstructed_endpoint_accepts_proved_identity_and_field_order() {
    prove_termination(&COUNTDOWN.replace(NEXT_LIMIT, "limit: countdown.limit + 0"));
    let reordered = COUNTDOWN.replace(
        "remaining: countdown.remaining - 1,\n            limit: countdown.limit",
        "limit: countdown.limit,\n            remaining: countdown.remaining - 1",
    );
    assert_ne!(reordered, COUNTDOWN);
    prove_termination(&reordered);
}

#[test]
fn related_sibling_coordinates_contribute_entry_facts() {
    let source = COUNTDOWN
        .replace("limit: u64 [0..=5];", "limit: u64 [0..=5]; spare: u64 [0..=5];")
        .replace(
            PRECONDITION,
            "requires countdown.remaining <= countdown.spare && countdown.spare <= countdown.limit;",
        )
        .replace(NEXT_LIMIT, "limit: countdown.limit, spare: countdown.spare");
    prove_termination(&source);
    reject_range(&source.replace("countdown.remaining <= countdown.spare && ", ""));
    reject_range(&source.replace(
        "countdown.spare <= countdown.limit",
        "countdown.spare >= countdown.limit",
    ));
    // The established rank invariant suffices without renewing the sibling
    // relation. Ordinary checking independently rejects the invalid arrival.
    for (value, expected) in [
        ("0", "cannot prove requires contract for call walk"),
        (
            "6",
            "field `spare`: value 6 is outside its declared range `0..=5`",
        ),
    ] {
        let changed = source.replace("spare: countdown.spare", &format!("spare: {value}"));
        prove_termination(&changed);
        let diagnostics = lower_typed_trees(typed(&changed))
            .expect_err("the constructor and recursive contract still require ordinary checking");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(expected)),
            "{changed}\n{diagnostics:#?}"
        );
    }

    let equal = source
        .replace(
            "countdown.spare <= countdown.limit",
            "countdown.spare == countdown.limit",
        )
        .replace(NEXT_LIMIT, "limit: countdown.spare");
    prove_termination(&equal);
    reject_range(&equal.replace(
        "countdown.spare == countdown.limit",
        "countdown.spare <= countdown.limit",
    ));
}

#[test]
fn endpoint_in_second_record_tracks_only_its_exact_field() {
    let source = second_record();
    prove_termination(&source);
    prove_termination(&source.replace(
        "}, bounds)",
        "}, Countdown { remaining: 0, limit: bounds.limit + 0 })",
    ));
    // The ranked record's unrelated limit is not the endpoint in this witness.
    prove_termination(&source.replace(NEXT_LIMIT, "limit: 5"));
    for argument in [
        "countdown",
        "Countdown { remaining: bounds.remaining, limit: 5 }",
        "Countdown { remaining: bounds.remaining, limit: bounds.remaining }",
    ] {
        reject_range(&source.replace("}, bounds)", &format!("}}, {argument})")));
    }
}

#[test]
fn field_coordinates_do_not_confuse_subjects_or_siblings() {
    let source = second_record();
    for requirement in [
        "requires bounds.remaining <= bounds.limit;",
        "requires countdown.limit <= bounds.limit;",
        "requires countdown.remaining <= countdown.limit;",
    ] {
        reject_range(&source.replace("requires countdown.remaining <= bounds.limit;", requirement));
    }
    reject_range(&source.replace("in 0..=bounds.limit", "in 0..=bounds.remaining"));
    reject_termination(&source.replace("countdown.remaining - 1", "bounds.remaining - 1"));
    reject_termination(&COUNTDOWN.replace("countdown.remaining - 1", "countdown.limit - 1"));
}

#[test]
fn reconstructed_endpoint_requires_the_declared_owner() {
    let source = second_record().replace(
        "}, bounds)",
        "}, Other { remaining: bounds.remaining, limit: bounds.limit })",
    );
    reject_range(&format!(
        "data Other {{ remaining: u64 [0..=5]; limit: u64 [0..=5]; }} {source}"
    ));
}

#[test]
fn changed_field_endpoint_rejects_even_when_the_next_rank_fits() {
    // Each replacement still contains remaining - 1 under the original guard
    // and entry relation. Membership alone cannot discharge endpoint pinning.
    for replacement in [
        "limit: 5",
        "limit: countdown.limit - 1",
        "limit: countdown.remaining",
    ] {
        reject_range(&COUNTDOWN.replace(NEXT_LIMIT, replacement));
    }
}

#[test]
fn field_floor_and_ceiling_are_pinned_simultaneously() {
    let source = COUNTDOWN
        .replace("limit: u64 [0..=5];", "limit: u64 [0..=5]; floor: u64 [0..=5];")
        .replace(
            PRECONDITION,
            "requires countdown.floor <= countdown.remaining && countdown.remaining <= countdown.limit;",
        )
        .replace("in 0..=countdown.limit", "in countdown.floor..=countdown.limit")
        .replace("remaining > 0", "remaining > countdown.floor")
        .replace(NEXT_LIMIT, "limit: countdown.limit, floor: countdown.floor");
    prove_termination(&source);
    for replacement in [
        "limit: countdown.limit, floor: 0",
        "limit: 5, floor: countdown.floor",
        "limit: 5, floor: 0",
        "limit: countdown.floor, floor: countdown.limit",
    ] {
        reject_range(&source.replace(
            "limit: countdown.limit, floor: countdown.floor",
            replacement,
        ));
    }
    reject_range(&source.replace("remaining > countdown.floor", "remaining > 0"));
}

#[test]
fn field_endpoint_requires_entry_membership_and_exclusive_ceiling_evidence() {
    reject_range(&COUNTDOWN.replace(PRECONDITION, ""));
    reject_range(&COUNTDOWN.replace(
        PRECONDITION,
        "requires countdown.limit <= countdown.remaining;",
    ));
    let exclusive = COUNTDOWN
        .replace(
            "remaining <= countdown.limit;",
            "remaining < countdown.limit;",
        )
        .replace("in 0..=countdown.limit", "in 0..countdown.limit");
    prove_termination(&exclusive);
    reject_range(&exclusive.replace(
        "remaining < countdown.limit;",
        "remaining <= countdown.limit;",
    ));
    let guarded = COUNTDOWN.replace(PRECONDITION, "").replace(
        "remaining > 0 {",
        "remaining > 0 && countdown.remaining <= countdown.limit {",
    );
    reject_range(&guarded);
}

#[test]
fn each_recursive_edge_must_preserve_the_field_endpoint_and_descend() {
    let source = COUNTDOWN.replace(
        "    transition countdown.remaining > 0 {",
        "    transition countdown.remaining > 1 {\n        true -> walk(Countdown { remaining: countdown.remaining - 2, limit: countdown.limit })\n    }\n    transition countdown.remaining > 0 {",
    );
    prove_termination(&source);
    for source in [
        source.replace(
            "remaining - 2, limit: countdown.limit",
            "remaining - 2, limit: 5",
        ),
        source.replace("limit: countdown.limit\n", "limit: 5\n"),
    ] {
        reject_range(&source);
    }
    for step in ["countdown.remaining - 1", "countdown.remaining - 2"] {
        reject_termination(&source.replace(step, "countdown.remaining"));
    }
}

#[test]
fn pinned_field_membership_does_not_prove_strict_descent() {
    for value in [
        "countdown.remaining",
        "countdown.remaining - 0",
        "countdown.remaining + 1",
    ] {
        reject_termination(&COUNTDOWN.replace("countdown.remaining - 1", value));
    }
    reject_termination(
        &COUNTDOWN.replace("false -> countdown.remaining", "false -> walk(countdown)"),
    );
}

#[test]
fn field_coordinates_require_selected_builtin_meanings() {
    for declaration in [
        "operator <= u64::compare(left: u64, right: u64) -> bool;",
        "operator > u64::compare(left: u64, right: u64) -> bool;",
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
    ] {
        reject_range(&format!("{declaration} {COUNTDOWN}"));
    }
    let reconstructed = COUNTDOWN.replace(NEXT_LIMIT, "limit: countdown.limit + 0");
    reject_range(&format!(
        "operator + u64::sum(left: u64, right: u64) -> u64; {reconstructed}"
    ));
    let wrapped = COUNTDOWN.replace(
        "countdown.remaining > 0",
        "(countdown.remaining > 0) == true",
    );
    prove_termination(&wrapped);
    reject_range(&format!(
        "operator == bool::compare(left: bool, right: bool) -> bool; {wrapped}"
    ));
}

#[test]
fn endpoint_coordinates_reject_prefix_writes_and_alias_writes() {
    for statement in [
        "countdown.limit = 5;",
        "countdown = Countdown { remaining: 5, limit: 5 };",
    ] {
        reject_range(&COUNTDOWN.replace(
            "    transition",
            &format!("    {statement}\n    transition"),
        ));
    }
    let helper = "machine reset(value: &mut u64) -> u64 { value = 5; 0 }";
    for (source, subject) in [
        (COUNTDOWN.to_owned(), "countdown.limit"),
        (second_record(), "bounds.limit"),
    ] {
        let source = source.replace(
            "    transition",
            &format!(
                "    let alias: &mut u64 = &mut {subject};\n    reset(alias);\n    transition"
            ),
        );
        reject_range(&format!("{helper} {source}"));
    }
    let disjoint = COUNTDOWN.replace(
        "    transition",
        "    let mut scratch: u64 = 0;\n    scratch = 5;\n    transition",
    );
    prove_termination(&disjoint);
}

#[test]
fn later_operands_cannot_invalidate_a_captured_field_endpoint() {
    let source = second_record()
        .replace("bounds: Countdown)", "bounds: Countdown, scratch: u64)")
        .replace("}, bounds)", "}, bounds, reset(&mut bounds.limit))");
    for helper in [
        "machine reset(value: &mut u64) -> u64 { value = 5; 0 }",
        "boundary machine reset(value: &mut u64) -> u64;",
    ] {
        reject_range(&format!("{helper} {source}"));
    }
    let field_operand = COUNTDOWN
        .replace("limit: u64 [0..=5];", "limit: u64 [0..=5]; scratch: u64;")
        .replace(
            NEXT_LIMIT,
            "limit: countdown.limit, scratch: reset(&mut countdown.limit)",
        );
    reject_range(&format!(
        "machine reset(value: &mut u64) -> u64 {{ value = 5; 0 }} {field_operand}"
    ));
}

#[test]
fn field_endpoints_require_defined_intermediates_and_exact_owned_carriers() {
    // Algebraic cancellation cannot establish that the Exact addition is
    // representable for every admitted limit in 0..=5.
    reject_range(&COUNTDOWN.replace(
        "in 0..=countdown.limit;",
        "in 0..=((countdown.limit + 18446744073709551615u64) - 18446744073709551615u64);",
    ));
    for parameter in ["bounds: &Countdown", "mut bounds: Countdown"] {
        reject_range(&second_record().replace("bounds: Countdown", parameter));
    }
    reject_range(&COUNTDOWN.replace("limit: u64 [0..=5];", "limit: u64 [0..=5] in Wrapping;"));
}
