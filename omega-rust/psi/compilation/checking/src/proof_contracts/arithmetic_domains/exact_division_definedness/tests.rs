use super::{BinaryOperator, ExactDefinednessConditions, exact_definedness_conditions};

fn check(source: &str) -> Result<(), String> {
    let program = crate::front_end::typed_program(source);
    crate::validate_program(&program).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

#[test]
fn exact_division_definedness_follows_the_shared_policy_catalog() {
    assert_eq!(
        exact_definedness_conditions(BinaryOperator::Divide),
        ExactDefinednessConditions {
            nonzero_divisor: true,
            signed_result_representable: true
        }
    );
}

#[test]
fn exact_division_definedness_uses_integer_constraints_without_an_affine_shape_gate() {
    for (numerator, divisor, operator) in [
        ("(value + 16384i16) * -2i16", "value * 2i16 + 1i16", "/"),
        ("(value - 16383i16) * 2i16", "value * 2i16 - 1i16", "%"),
        ("-2i16 * (16384i16 + value)", "1i16 + 2i16 * value", "/"),
    ] {
        let source = format!(
            "machine compute(value: i16) -> i16 requires -1i16 <= value, value <= 0i16 {{ ({numerator}) {operator} ({divisor}) }}"
        );
        assert_eq!(check(&source), Ok(()), "{source}");
    }
    let different_roots = "machine compute(value: i16, other: i16) -> i16
        requires -1i16 <= value, value <= 0i16, -1i16 <= other, other <= 0i16 {
        ((value + 16384i16) * -2i16) / (other * 2i16 + 1i16)
    }";
    let error = check(different_roots).expect_err("independent roots can reach MIN / -1");
    assert!(error.contains("MIN / -1"), "{error}");
    let even_divisor = "machine compute(value: i16) -> i16
        requires -1i16 <= value, value <= 0i16 { 42i16 / (value * 2i16) }";
    let error = check(even_divisor).expect_err("an even polynomial can be zero");
    assert!(error.contains("divisor must be proven nonzero"), "{error}");
}

#[test]
fn exact_division_definedness_requires_nonzero_and_excludes_signed_pair_in_bodies() {
    for operator in ["/", "%"] {
        let source = format!(
            "machine compute(numerator: i32, divisor: i32) -> i32 {{ numerator {operator} divisor }}"
        );
        let error = check(&source).expect_err("unconstrained operands cannot form Exact division");
        assert!(error.contains("divisor must be proven nonzero"), "{error}");
        assert!(error.contains("MIN / -1"), "{error}");
        let source = format!(
            "machine compute(numerator: i32, divisor: i32) -> i32 requires divisor != 0 {{ numerator {operator} divisor }}"
        );
        let error = check(&source).expect_err("nonzero does not exclude signed overflow");
        assert!(!error.contains("divisor must be proven nonzero"), "{error}");
        assert!(error.contains("MIN / -1"), "{error}");
    }
}

#[test]
fn exact_division_definedness_accepts_signed_exclusions_without_selecting_a_sign() {
    for operator in ["/", "%"] {
        for requirement in [
            "divisor != 0, divisor != -1",
            "0 != divisor, numerator != -2147483648",
        ] {
            let source = format!(
                "machine compute(numerator: i32, divisor: i32) -> i32 requires {requirement} {{ numerator {operator} divisor }}"
            );
            assert_eq!(check(&source), Ok(()), "{source}");
        }
    }
}

#[test]
fn exact_division_definedness_transports_false_equality_guards_through_state_edges() {
    let source = "
machine compute(numerator: i32, divisor: i32) -> i32 {
    transition divisor == 0 { true -> (0) _ -> nonzero(numerator, divisor) }
    state nonzero(numerator: i32, divisor: i32) -> i32 {
        transition divisor == -1 { true -> (0) _ -> divide(numerator, divisor) }
    }
    state divide(numerator: i32, divisor: i32) -> i32 { numerator / divisor }
}";
    assert_eq!(check(source), Ok(()));
}

#[test]
fn exact_division_definedness_invalidates_exclusions_on_operand_writes() {
    let source = "
data Divisor { value: i32; }
machine compute(numerator: i32, divisor: &mut Divisor, replacement: i32) -> i32
requires divisor.value != 0, divisor.value != -1
{
    divisor.value = replacement;
    numerator / divisor.value
}";
    let error = check(source).expect_err("the guard described the value before mutation");
    assert!(error.contains("divisor must be proven nonzero"), "{error}");
}

#[test]
fn exact_division_definedness_observes_effects_before_later_operands() {
    let source = "
data Divisor { value: i32; }
machine reset(divisor: &mut Divisor) -> i32 { divisor.value = 0; 42 }
machine compute(divisor: &mut Divisor) -> i32
requires divisor.value != 0, divisor.value != -1
{
    reset(divisor) / divisor.value
}";
    let error = check(source).expect_err("the left operand overwrites the right operand");
    assert!(error.contains("divisor must be proven nonzero"), "{error}");
}

#[test]
fn exact_division_definedness_retains_stores_across_state_edges() {
    let source = "
data Operands { numerator: u32; divisor: u32; }
machine compute(operands: &mut Operands) -> u32 {
    operands.numerator = 42;
    operands.divisor = 3;
    transition { _ -> divide(operands) }
    state divide(operands: &mut Operands) -> u32 { operands.numerator / operands.divisor }
}";
    assert_eq!(check(source), Ok(()));
    let unsafe_source = source.replace("operands.divisor = 3;", "operands.divisor = 0;");
    assert!(check(&unsafe_source).is_err());
}

#[test]
fn exact_division_definedness_does_not_keep_store_bounds_after_alias_writes() {
    let source = "
data Operands { divisor: u32; }
machine compute(operands: &mut Operands) -> u32 {
    operands.divisor = 3;
    let alias: &mut u32 = &mut operands.divisor;
    alias = 0;
    transition { _ -> divide(operands) }
    state divide(operands: &mut Operands) -> u32 { 42u32 / operands.divisor }
}";
    let error = check(source).expect_err("the alias replaces the stored divisor");
    assert!(
        error.contains("zero") || error.contains("nonzero"),
        "{error}"
    );
}

#[test]
fn exact_division_definedness_includes_reset_backedges_in_state_arrivals() {
    let source = "
data Operands { divisor: u32; }
machine compute(operands: &mut Operands, retry: bool) -> u32 {
    operands.divisor = 3;
    transition { _ -> divide(operands, retry) }
    state divide(operands: &mut Operands, retry: bool) -> u32 {
        let result: u32 = 42u32 / operands.divisor;
        transition retry { true -> reset(operands) _ -> (result) }
    }
    state reset(operands: &mut Operands) -> u32 {
        operands.divisor = 0;
        transition { _ -> divide(operands, false) }
    }
}";
    let error = check(source).expect_err("the backedge can deliver a zero divisor");
    assert!(error.contains("divisor must be proven nonzero"), "{error}");
}

#[test]
fn exact_division_definedness_joins_symmetric_exclusions() {
    let source = "
machine compute(numerator: i32, divisor: i32, choose: bool) -> i32
requires numerator != -2147483648
{
    transition choose { true -> first(numerator, divisor) _ -> second(numerator, divisor) }
    state first(numerator: i32, divisor: i32) -> i32 {
        transition divisor != 0 { true -> divide(numerator, divisor) _ -> (0) }
    }
    state second(numerator: i32, divisor: i32) -> i32 {
        transition 0 != divisor { true -> divide(numerator, divisor) _ -> (0) }
    }
    state divide(numerator: i32, divisor: i32) -> i32 { numerator / divisor }
}";
    assert_eq!(check(source), Ok(()));
}

#[test]
fn exact_division_definedness_projects_total_widening_contracts() {
    let source = "
machine widen(value: u32) -> u64 ensures result == (value as u64) { value as u64 }
machine compute(divisor: u32) -> u64 requires divisor > 0 {
    let widened: u64 = widen(divisor);
    (42 as u64) / widened
}";
    assert_eq!(check(source), Ok(()));
    assert!(check(&source.replace("requires divisor > 0", "")).is_err());
}

#[test]
fn exact_division_definedness_preserves_clamped_bounds_through_widening() {
    let direct = "machine compute(divisor: u32) -> u32 {
        let safe: u32 = clamp(divisor, 1, 4294967295);
        42 / safe
    }";
    assert_eq!(check(direct), Ok(()), "direct clamp");
    let ranged = "machine widen(value: u32) -> u64 [0..=4294967295]
        ensures result == (value as u64) { value as u64 }
        machine compute(divisor: u32) -> u64 requires divisor > 0 { (42 as u64) / widen(divisor) }";
    assert_eq!(check(ranged), Ok(()), "ranged return contract");
    let source = "
machine widen(value: u32) -> u64 [0..=4294967295]
ensures result == (value as u64) { value as u64 }
machine compute(divisor: u32) -> u64 {
    let safe: u32 = clamp(divisor, 1, 4294967295);
    let widened: u64 = widen(safe);
    (42 as u64) / widened
}";
    assert_eq!(check(source), Ok(()));
}
