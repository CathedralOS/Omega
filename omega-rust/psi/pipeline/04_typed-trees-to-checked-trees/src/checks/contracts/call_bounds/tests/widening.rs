use super::{assert_call_requirement_rejected, checked};

#[test]
fn widened_actuals_preserve_live_integer_bounds_and_nonzero_premises() {
    for (source_type, target_type, argument) in [
        ("u8", "u64", "input as u64"),
        ("u8", "u64", "(input as u16) as u64"),
        ("u8", "i64", "input as i64"),
        ("i8", "i64", "input as i64"),
        ("u16", "u64", "(input + 0) as u64"),
        ("u8", "u64", "(input as u64) + 0"),
        ("u8 in Wrapping", "u64", "input as u64"),
    ] {
        for requirement in ["input != 0", "input >= 2 && input <= 4"] {
            let source = format!(
                "machine demand(value: {target_type}) requires {} {{}}
                 machine caller(input: {source_type}) requires {requirement} {{ demand({argument}); }}",
                requirement.replace("input", "value")
            );
            checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        }
    }
}

#[test]
fn signed_nonzero_transport_requires_the_exact_live_value() {
    for source in [
        "machine demand(value: i64) requires value != 0 {} machine caller(input: i8, other: i8) requires input != 0 { demand(other as i64); }",
        "machine demand(value: i64) requires value != 0 {} machine caller(input: i8) requires input != 0 { demand((input as i64) + 1); }",
        "machine demand(value: i64) requires value > 0 {} machine caller(input: i8) requires input != 0 { demand(input as i64); }",
        "machine demand(value: i64) requires value != 0 {} machine caller(mut input: i8) requires input != 0 { input = 0; demand(input as i64); }",
        "machine demand(value: i64) requires value != 0 {} machine caller(input: i8 in Wrapping) requires input != 0 { demand((input + 1) as i64); }",
    ] {
        assert_call_requirement_rejected(source);
    }
}

#[test]
fn signed_disequality_does_not_license_legacy_float_polynomials() {
    assert!(
        checked(
            "machine invalid(input: f64, other: f64)
         requires input != 0 ensures input + other != other {}"
        )
        .is_err(),
        "a nonzero finite input can disappear when added to infinity"
    );
}

#[test]
fn widening_does_not_borrow_another_values_premise_or_erase_operand_policy() {
    for source in [
        "machine demand(value: u64) requires value != 0 {} machine caller(input: u8, other: u8) requires input != 0 { demand(other as u64); }",
        "machine demand(value: u64) requires value != 0 {} machine caller(input: u8) { demand(input as u64); }",
        "machine demand(value: u64) requires value != 0 {} machine caller(input: u8 in Wrapping) requires input != 0 { demand((input + 1) as u64); }",
        "machine demand(value: u64) requires value != 0 {} machine caller(mut input: u8) requires input != 0 { input = 0; demand(input as u64); }",
        "boundary operator + Meaning::add(left: u8, right: u8) -> u8; machine demand(value: u64) requires value != 0 {} machine caller(input: u8) requires input != 0 { demand((input + 0) as u64); }",
    ] {
        assert_call_requirement_rejected(source);
    }
    for (source_type, target_type) in [("u64", "u8"), ("i8", "u64")] {
        let source = format!(
            "machine demand(value: {target_type}) requires value != 0 {{}}
             machine caller(input: {source_type}) requires input != 0 {{ demand(input as {target_type}); }}"
        );
        assert!(checked(&source).is_err(), "unproved conversion: {source}");
    }
}

#[test]
fn widening_consumes_a_fresh_guard_after_mutation() {
    let source = "machine demand(value: u64) -> u64 requires value != 0 { value }
        machine caller(mut input: u8, other: u8) -> u64 {
            input = other;
            transition input != 0 {
                true -> demand(input as u64)
                false -> 0
            }
        }";
    checked(source).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"));
    assert_call_requirement_rejected(
        &source.replace("false -> 0", "false -> demand(input as u64)"),
    );
}

#[test]
fn argument_widening_does_not_reinterpret_wrapping_symbolic_range_results() {
    let source = "machine wrong(input: u8 in Wrapping, lower: u64[1..=1])
        -> u64[lower..=256] { (input + 1) as u64 }";
    assert!(
        checked(source).is_err(),
        "input 255 returns zero, outside the promised range"
    );
}

#[test]
fn argument_widening_does_not_reinterpret_saved_wrapping_computations() {
    assert_call_requirement_rejected(
        "machine demand(value: u64) -> u64 requires value != 0 { value }
         machine forward(input: u8) -> u64 {
             let saved: u8 in Wrapping = input;
             let widened: u64 = (saved + 1) as u64;
             demand(widened)
         }",
    );
}

#[test]
fn widening_preserves_capture_order_across_later_operand_effects() {
    for (changed, accepted) in [("input", false), ("other", true)] {
        let source = format!(
            "machine overwrite(value: &mut u8) -> bool {{ value = 0; true }}
             machine demand(left: u64, ignored: bool, right: u64)
             requires left <= right && right <= left {{}}
             machine caller(mut input: u8, mut other: u8) {{
                 demand(input as u64, overwrite(&mut {changed}), input as u64);
             }}"
        );
        if accepted {
            checked(&source).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"));
        } else {
            assert_call_requirement_rejected(&source);
        }
    }
}
