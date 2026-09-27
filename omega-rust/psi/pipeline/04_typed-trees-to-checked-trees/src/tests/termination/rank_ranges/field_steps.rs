use crate::tests::front_end::typed_program;

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_rank_step/main.omg"
));

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
