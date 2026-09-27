use super::lower_typed_trees;
use crate::CheckingRequest;
use crate::tests::front_end::typed_program;

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
        lower_typed_trees(typed_program(&source), &CheckingRequest::settled()).expect(guard);
    }
    lower_typed_trees(
        typed_program(&COUNTDOWN.replace("}, amount)", "}, 1)")),
        &CheckingRequest::settled(),
    )
    .expect("batch sizes may change while remaining positive at every arrival");
    let computed = COUNTDOWN
        .replace("remaining >= amount", "remaining >= amount + 1")
        .replace("remaining - amount", "remaining - (amount + 1)");
    lower_typed_trees(typed_program(&computed), &CheckingRequest::settled())
        .expect("computed positive step");
    let literal = COUNTDOWN
        .replace("remaining >= amount", "remaining >= 2")
        .replace("remaining - amount", "remaining - 2");
    lower_typed_trees(typed_program(&literal), &CheckingRequest::settled())
        .expect("literal batches larger than one");
}

#[test]
fn field_rank_rejects_zero_steps_weak_guards_and_non_decreasing_continuations() {
    for source in [
        COUNTDOWN.replace("1 <= amount", "0 <= amount"),
        COUNTDOWN.replace("remaining >= amount", "remaining > 0"),
        COUNTDOWN.replace("remaining >= amount", "remaining < amount"),
        COUNTDOWN.replace("remaining - amount", "remaining + amount"),
        COUNTDOWN.replace("false -> countdown.remaining", "false -> self"),
        COUNTDOWN.replace(
            "false -> countdown.remaining",
            "false -> walk(countdown, amount)",
        ),
    ] {
        assert_ne!(source, COUNTDOWN, "negative fixture mutation must apply");
        crate::checks::termination::check_machine_termination(&typed_program(&source))
            .expect_err(&source);
    }
}

#[test]
fn batch_descent_requires_preserved_inputs_and_builtin_operations() {
    for prefix in ["amount = 0;", "countdown.remaining = 5;"] {
        let source = COUNTDOWN.replace("    transition", &format!("    {prefix}\n    transition"));
        crate::checks::termination::check_machine_termination(&typed_program(&source))
            .expect_err(prefix);
    }
    for declaration in [
        "operator >= u64::compare(left: u64, right: u64) -> bool;",
        "operator - u64::subtract(left: u64, right: u64) -> u64;",
    ] {
        crate::checks::termination::check_machine_termination(&typed_program(&format!(
            "{declaration} {COUNTDOWN}"
        )))
        .expect_err(declaration);
    }
    let wrapped = COUNTDOWN.replace(
        "countdown.remaining >= amount",
        "(countdown.remaining >= amount) == true",
    );
    crate::checks::termination::check_machine_termination(&typed_program(&format!(
        "operator == bool::compare(left: bool, right: bool) -> bool; {wrapped}"
    )))
    .expect_err("an authored Boolean wrapper is not builtin comparison evidence");
}
