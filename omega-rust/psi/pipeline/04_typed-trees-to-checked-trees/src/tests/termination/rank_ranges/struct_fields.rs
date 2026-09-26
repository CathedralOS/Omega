use crate::tests::front_end::typed_program;

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_rank_range/main.omg"
));

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
    crate::checks::termination::check_machine_termination(&typed_program(&unrelated))
        .expect_err("another owner's same-named field cannot supply this rank's bounds");
    let wrapping = COUNTDOWN.replace("u64 [0..=5]", "u64 [0..=5] in Wrapping");
    crate::checks::termination::check_machine_termination(&typed_program(&wrapping))
        .expect_err("permissive wrapping storage has no enforced declared interval");
}
