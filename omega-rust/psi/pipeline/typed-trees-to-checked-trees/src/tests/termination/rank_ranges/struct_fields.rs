use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_rank_range/main.omg"
));

#[test]
fn direct_field_measure_proves_its_enforced_rank_range() {
    let program = typed(COUNTDOWN);
    crate::checks::termination::check_machine_termination(&program)
        .expect("declared field range proves the produced rank's bounds");
    lower_typed_trees(program).expect("guarded reconstruction preserves the constrained field");
}

#[test]
fn direct_field_measure_checks_rank_endpoints_not_storage_or_guard_spelling() {
    for range in ["0..=4", "1..=5", "0..5", "6..=5"] {
        let source = COUNTDOWN.replace("in 0..=5", &format!("in {range}"));
        let program = typed(&source);
        let diagnostics = crate::checks::termination::check_machine_termination(&program)
            .expect_err("the field's full declared range must fit");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("cannot prove rank range"))
        );
    }
    lower_typed_trees(typed(&COUNTDOWN.replace("in 0..=5", "in 0..6")))
        .expect("exclusive upper endpoint");
    let nonzero_floor = COUNTDOWN
        .replace("0..=5", "1..=5")
        .replace("remaining > 0", "remaining > 1");
    lower_typed_trees(typed(&nonzero_floor)).expect("nonzero rank floor");
    lower_typed_trees(typed(
        &nonzero_floor.replace("remaining > 1", "remaining >= 2"),
    ))
    .expect("inclusive positive guard");
}

#[test]
fn direct_field_rank_membership_does_not_excuse_bad_reconstruction_or_descent() {
    for actual in ["countdown.remaining + 1", "countdown.remaining - 2", "6"] {
        let source = COUNTDOWN.replace("countdown.remaining - 1", actual);
        assert!(lower_typed_trees(typed(&source)).is_err(), "{actual}");
    }
    for next in ["walk(countdown)", "self"] {
        let source = COUNTDOWN.replace("false -> countdown.remaining", &format!("false -> {next}"));
        crate::checks::termination::check_machine_termination(&typed(&source))
            .expect_err("every cyclic occurrence still needs strict descent");
    }
}

#[test]
fn direct_field_rank_uses_only_the_selected_fields_enforced_bounds() {
    let unrelated = COUNTDOWN
        .replace(
            "data Countdown",
            "data Other { remaining: u64 [0..=5]; } data Countdown",
        )
        .replace(
            "data Countdown { remaining: u64 [0..=5]; }",
            "data Countdown { remaining: u64; }",
        );
    crate::checks::termination::check_machine_termination(&typed(&unrelated))
        .expect_err("another owner's same-named field cannot supply this rank's bounds");
    let wrapping = COUNTDOWN.replace("u64 [0..=5]", "u64 [0..=5] in Wrapping");
    crate::checks::termination::check_machine_termination(&typed(&wrapping))
        .expect_err("permissive wrapping storage has no enforced declared interval");
}

#[test]
fn field_descent_requires_selected_builtin_comparison_and_subtraction() {
    let source = COUNTDOWN.replace("in 0..=5", "");
    for declaration in [
        "operator > u64::compare(left: u64, right: u64) -> bool;",
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
    ] {
        let program = typed(&format!("{declaration}\n{source}"));
        crate::checks::termination::check_machine_termination(&program)
            .expect_err("an authored operation cannot inherit builtin descent laws");
    }
}

#[test]
fn field_rank_cannot_restart_before_each_decrement() {
    for reset in [
        "countdown.remaining = 5;",
        "countdown = Countdown { remaining: 5 };",
        "reset(&mut countdown);",
    ] {
        let source = COUNTDOWN
            .replace(
                "walk(countdown: Countdown)",
                "walk(mut countdown: Countdown)",
            )
            .replace("    transition", &format!("    {reset}\n    transition"));
        let source = format!(
            "{source}\nmachine reset(countdown: &mut Countdown) {{ countdown.remaining = 5; }}"
        );
        let program = typed(&source);
        assert_eq!(
            crate::infer_machine_termination_summary(&program, program.machines()[0].symbol),
            Some(language_semantics::TerminationGuarantee::NoGuarantee),
            "{reset}"
        );
    }
    let disjoint = COUNTDOWN.replace(
        "    transition",
        "    let mut scratch: u64 = 0;\n    scratch = 5;\n    transition",
    );
    lower_typed_trees(typed(&disjoint)).expect("a disjoint local write preserves the ranked field");
}
