//! `f32` and `f64` as the types of values a program holds and hands on,
//! read from source as Build programs.
//!
//! numeric_values.md, "Floating formats and operations": "`f32` permanently
//! means IEEE binary32 and `f64` IEEE binary64 on every target providing
//! them." This route carries a float wherever it carries any scalar: a
//! field, a parameter, a result, a local, an array element, an arm's value.
//! It computes nothing with one yet: a float literal, operator, cast and
//! clause comparison are each refused by name, with no verdict.

use super::build_purpose::judge_build;
use super::*;

/// `accepted`, `rejected` with the rule broken, or the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected { violations, .. }) => match &violations[..] {
            [first, ..] => format!("rejected: {}", first.statement),
            [] => "rejected".to_owned(),
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_float_is_held_and_handed_on_like_any_scalar() {
    for program in [
        // Declared and never used, as a module of helpers declares it.
        "data Calibration { magnitude: f32; wide: f64; }
machine build_entry() { }",
        "machine same(value: f64) -> f64 { value }
machine build_entry(value: f64) { use(same(value)); }
machine use(value: f64) { }",
        "data Calibration { magnitude: f32; count: u32; }
machine magnitude(calibration: &Calibration) -> f32 { calibration.magnitude }
machine build_entry(calibration: &Calibration) { use(magnitude(calibration)); }
machine use(value: f32) { }",
        "data Calibration { magnitude: f32; count: u32; }
machine set(calibration: &mut Calibration, value: f32) { calibration.magnitude = value; }
machine build_entry(calibration: &mut Calibration, value: f32) { set(calibration, value); }",
        "data Calibration { magnitude: f32; count: u32; }
machine build_entry(value: f32) {
    let calibration: Calibration = Calibration { magnitude: value, count: 1 };
    let kept: f32 = calibration.magnitude;
    use(kept);
}
machine use(value: f32) { }",
        "machine build_entry(flag: bool, left: f32, right: f32, values: &[f32; 2]) {
    use(match flag { true -> left false -> right }, values[0]);
}
machine use(chosen: f32, first: f32) { }",
    ] {
        assert_eq!(verdict(program), "accepted", "{program}");
    }
}

/// numeric_values.md, "Integer equality and order": comparison "adds neither
/// implicit arithmetic widening nor conversion at assignments, arguments, or
/// returns", and the two float formats are two carriers. Each failing twin
/// differs from the accepted program in the one type it passes.
#[test]
fn a_float_of_one_format_is_rejected_where_another_carrier_is_received() {
    let program = |passed: &str, received: &str| {
        format!(
            "machine build_entry(value: {passed}) {{ use(value); }}
machine use(value: {received}) {{ }}"
        )
    };
    assert_eq!(verdict(&program("f32", "f32")), "accepted");
    for (passed, received) in [
        ("f32", "f64"),
        ("f64", "f32"),
        ("u32", "f32"),
        ("f64", "u64"),
    ] {
        assert_eq!(
            verdict(&program(passed, received)),
            format!(
                "rejected: this value has scalar carrier `{passed}` but the parameter it is \
                 passed for requires `{received}`: there is no implicit scalar conversion at a \
                 receiving type while reading the body of `build_entry`"
            )
        );
    }
}

/// What this route does not compute with a float is refused by name. A
/// clause comparison is among them on purpose: numeric_values.md, "Under
/// IEEE comparison, ordered relations are false on NaN, `==` is false", so
/// `value == value` must not be read as the mathematical equality that holds
/// of every value.
#[test]
fn what_is_not_computed_with_a_float_is_refused_by_name() {
    for (program, refusal) in [
        (
            "machine build_entry() { use(1.5); }
machine use(value: f32) { }",
            "expression float literal while reading the body of `build_entry`",
        ),
        (
            "machine build_entry(value: f32) { use(value + value); }
machine use(value: f32) { }",
            "typing operand types while reading the body of `build_entry`",
        ),
        (
            "machine build_entry(value: f32) { check(value == value); }
machine check(same: bool) { }",
            "typing operand types while reading the body of `build_entry`",
        ),
        (
            "machine build_entry(value: f32) { check(value < value); }
machine check(less: bool) { }",
            "typing operand types while reading the body of `build_entry`",
        ),
        (
            "machine build_entry(value: f32) { use(value as f64); }
machine use(value: f64) { }",
            "expression cast between carriers that are not integers \
             while reading the body of `build_entry`",
        ),
        (
            "machine need(value: f32) requires value == value { }
machine build_entry(value: f32) { need(value); }",
            "contract comparison of floats in a clause while reading the signature of `need`",
        ),
        (
            "machine make(value: f32) -> f32 ensures result == value { value }
machine build_entry(value: f32) { use(make(value)); }
machine use(value: f32) { }",
            "contract comparison of floats in a clause while reading the signature of `make`",
        ),
        (
            "data Calibration { magnitude: f32; }
machine build_entry() { let calibration: Calibration; use(calibration.magnitude); }
machine use(value: f32) { }",
            "completion a float local reached lowering",
        ),
    ] {
        assert_eq!(verdict(program), refusal, "{program}");
    }
}
