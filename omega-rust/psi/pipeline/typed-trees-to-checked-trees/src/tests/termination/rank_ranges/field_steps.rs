use super::{lower_typed_trees, typed};

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_rank_step/main.omg"
));

#[test]
fn field_rank_accepts_positive_batch_sizes_and_equivalent_guard_polarities() {
    for guard in [
        "countdown.remaining >= amount",
        "amount <= countdown.remaining",
        "!(countdown.remaining < amount)",
        "(countdown.remaining >= amount) == true",
    ] {
        let source = COUNTDOWN.replace("countdown.remaining >= amount", guard);
        lower_typed_trees(typed(&source)).expect(guard);
    }
    lower_typed_trees(typed(&COUNTDOWN.replace("}, amount)", "}, 1)")))
        .expect("batch sizes may change while remaining positive at every arrival");
    let computed = COUNTDOWN
        .replace("remaining >= amount", "remaining >= amount + 1")
        .replace("remaining - amount", "remaining - (amount + 1)");
    lower_typed_trees(typed(&computed)).expect("computed positive step");
    let literal = COUNTDOWN
        .replace("remaining >= amount", "remaining >= 2")
        .replace("remaining - amount", "remaining - 2");
    lower_typed_trees(typed(&literal)).expect("literal batches larger than one");
}

#[test]
fn batch_descent_requires_the_exact_subject_and_reconstructed_owner() {
    let other = COUNTDOWN
        .replace(
            "amount: u64 [1..=2]",
            "amount: u64 [1..=2], other: Countdown",
        )
        .replace("}, amount)", "}, amount, other)");
    for source in [
        other.replace("countdown.remaining >=", "other.remaining >="),
        other.replace("countdown.remaining -", "other.remaining -"),
        format!(
            "data Other {{ remaining: u64 [0..=5]; }} {}",
            COUNTDOWN.replace("walk(Countdown {", "walk(Other {")
        ),
    ] {
        crate::checks::termination::check_machine_termination(&typed(&source))
            .expect_err("another value or nominal owner cannot supply descent");
    }
}

#[test]
fn batch_descent_checks_each_recursive_branch() {
    let source = COUNTDOWN.replace(
        "    transition countdown.remaining >= amount {",
        "    transition countdown.remaining >= 3 {\n        true -> walk(Countdown { remaining: countdown.remaining - 3 }, amount)\n    }\n    transition countdown.remaining >= amount {",
    );
    lower_typed_trees(typed(&source)).expect("both recursive branches decrease");
    for unchanged in [
        source.replace("remaining - 3", "remaining"),
        source.replace("remaining - amount", "remaining"),
    ] {
        crate::checks::termination::check_machine_termination(&typed(&unchanged))
            .expect_err("one descending branch cannot cover the other");
    }
}

#[test]
fn field_rank_rejects_zero_steps_weak_guards_and_non_decreasing_continuations() {
    for source in [
        COUNTDOWN.replace("amount: u64 [1..=2]", "amount: u64 [0..=2]"),
        COUNTDOWN.replace("remaining >= amount", "remaining > 0"),
        COUNTDOWN.replace("remaining >= amount", "remaining < amount"),
        COUNTDOWN.replace("remaining - amount", "remaining + amount"),
        COUNTDOWN.replace("false -> countdown.remaining", "false -> self"),
        COUNTDOWN.replace(
            "false -> countdown.remaining",
            "false -> walk(countdown, amount)",
        ),
    ] {
        crate::checks::termination::check_machine_termination(&typed(&source)).expect_err(&source);
    }
}

#[test]
fn batch_descent_requires_preserved_inputs_and_builtin_operations() {
    for prefix in ["amount = 0;", "countdown.remaining = 5;"] {
        let source = COUNTDOWN.replace("    transition", &format!("    {prefix}\n    transition"));
        crate::checks::termination::check_machine_termination(&typed(&source)).expect_err(prefix);
    }
    for declaration in [
        "operator >= u64::compare(left: u64, right: u64) -> bool;",
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
    ] {
        crate::checks::termination::check_machine_termination(&typed(&format!(
            "{declaration} {COUNTDOWN}"
        )))
        .expect_err(declaration);
    }
    let wrapped = COUNTDOWN.replace(
        "countdown.remaining >= amount",
        "(countdown.remaining >= amount) == true",
    );
    crate::checks::termination::check_machine_termination(&typed(&format!(
        "operator == bool::compare(left: bool, right: bool) -> bool; {wrapped}"
    )))
    .expect_err("an authored Boolean wrapper is not builtin comparison evidence");
}

#[test]
fn batch_descent_rejects_alias_writes_in_prefix_and_edge_operands() {
    let helper = "machine reset(value: &mut u64) -> u64 [1..=2] { value = 5; 1 }";
    for source in [
        COUNTDOWN.replace(
            "    transition",
            "    let alias: &mut u64 = &mut countdown.remaining;\n    reset(alias);\n    transition",
        ),
        COUNTDOWN.replace("}, amount)", "}, reset(&mut countdown.remaining))"),
        COUNTDOWN.replace("}, amount)", "}, reset(&mut amount))"),
        COUNTDOWN
            .replace("remaining: u64 [0..=5];", "remaining: u64 [0..=5]; other: u64;")
            .replace("remaining - amount }", "remaining - amount, other: reset(&mut countdown.remaining) }"),
    ] {
        crate::checks::termination::check_machine_termination(&typed(&format!("{helper} {source}")))
            .expect_err("operand and alias writes cannot preserve the rank snapshot");
    }
}

#[test]
fn full_checking_rejects_an_incompatible_step_carrier() {
    let source = COUNTDOWN.replace("amount: u64 [1..=2]", "amount: i32 [1..=2]");
    lower_typed_trees(typed(&source))
        .expect_err("positive bounds do not establish carrier compatibility");
}
