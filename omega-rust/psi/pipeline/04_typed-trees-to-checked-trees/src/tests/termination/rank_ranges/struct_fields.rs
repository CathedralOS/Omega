use crate::tests::front_end::typed_program;

const COUNTDOWN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/termination/measure_field_rank_range/main.omg"
));

#[test]
fn direct_field_rank_uses_only_the_selected_fields_enforced_bounds() {
    let field_bounds = " where 0 <= remaining, remaining <= 5";
    assert!(COUNTDOWN.contains(field_bounds));
    let unrelated = COUNTDOWN.replace(field_bounds, "").replace(
        "data Countdown",
        "data Other where 0 <= remaining, remaining <= 5 { remaining: u64; } data Countdown",
    );
    crate::checks::termination::check_machine_termination(&typed_program(&unrelated))
        .expect_err("another owner's same-named field cannot supply this rank's bounds");
    let wrapping = COUNTDOWN.replace("remaining: u64;", "remaining: u64 in Wrapping;");
    assert_ne!(wrapping, COUNTDOWN, "field policy mutation must apply");
    crate::checks::termination::check_machine_termination(&typed_program(&wrapping))
        .expect_err("a wrapping field is not an exact arithmetic rank");
}
