//! A caller supplies the selected proposition, not a same-spelled numeric law.

use super::*;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_trees(source)) {
        Ok(_) => assert!(
            accepted,
            "authored requires supplied a builtin bound: {source}"
        ),
        Err(diagnostics) => {
            assert!(!accepted, "{source}\n{diagnostics:#?}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("may overflow")),
                "reject the unsafe arithmetic, not unrelated syntax: {source}\n{diagnostics:#?}"
            );
        }
    }
}

const LITERAL_CASES: &[(&str, &str, &str, &str)] = &[
    (">", "n > 0", "n - 1", "u64"),
    ("<", "0 < n", "n - 1", "u64"),
    (">=", "n >= 1", "n - 1", "u64"),
    ("<=", "1 <= n", "n - 1", "u64"),
    ("<", "n < 255", "n + 1", "u8"),
    (">", "255 > n", "n + 1", "u8"),
];

#[test]
fn ordered_requires_literals_need_their_selected_builtin_meaning() {
    for &(spelling, condition, result, carrier) in LITERAL_CASES {
        check(
            &format!(
                "operator {spelling} {carrier}::custom(left: {carrier}, right: {carrier}) -> bool;
            machine calculate(n: {carrier}) -> {carrier} requires {condition}; {{ {result} }}"
            ),
            false,
        );
    }
}

#[test]
fn builtin_and_unrelated_ordered_requires_keep_literal_bounds() {
    for &(spelling, condition, result, carrier) in LITERAL_CASES {
        for declaration in [
            String::new(),
            format!("operator {spelling} f64::unrelated(left: f64, right: f64) -> bool;"),
        ] {
            check(
                &format!(
                    "{declaration} machine calculate(n: {carrier}) -> {carrier} requires {condition}; {{ {result} }}"
                ),
                true,
            );
        }
    }
}

#[test]
fn authored_requires_cannot_seed_joint_subtraction_in_either_direction() {
    for (spelling, condition) in [(">=", "left >= right"), ("<=", "right <= left")] {
        check(&format!("operator {spelling} u64::custom(left: u64, right: u64) -> bool;
            machine subtract(left: u64, right: u64) -> u64 requires {condition}; {{ left - right }}"), false);
    }
}

#[test]
fn authored_requires_cannot_seed_joint_addition_bounds() {
    for (spelling, condition) in [("<=", "left <= 255 - right"), (">=", "255 - right >= left")] {
        check(
            &format!(
                "operator {spelling} u8::custom(left: u8, right: u8) -> bool;
            machine add(left: u8, right: u8) -> u8 requires {condition}; {{ left + right }}"
            ),
            false,
        );
    }
}

#[test]
fn builtin_joint_requires_remain_available_with_unrelated_operators() {
    for (spelling, condition, carrier, result) in [
        (">=", "left >= right", "u64", "left - right"),
        ("<=", "right <= left", "u64", "left - right"),
        ("<=", "left <= 255 - right", "u8", "left + right"),
        (">=", "255 - right >= left", "u8", "left + right"),
    ] {
        check(&format!("operator {spelling} f64::unrelated(left: f64, right: f64) -> bool;
            machine calculate(left: {carrier}, right: {carrier}) -> {carrier} requires {condition}; {{ {result} }}"), true);
    }
}

#[test]
fn genuine_requires_remain_live_for_mutable_entry_parameters() {
    check(
        "operator > f64::unrelated(left: f64, right: f64) -> bool;
        machine decrement(mut n: u64) -> u64 requires n > 0; { n - 1 }",
        true,
    );
}

#[test]
fn conjunctions_do_not_hide_an_authored_ordering_from_subtraction() {
    for condition in ["left >= right && true", "true && left >= right"] {
        for (declaration, accepted) in [
            (
                "operator >= u64::custom(left: u64, right: u64) -> bool;",
                false,
            ),
            (
                "operator >= f64::unrelated(left: f64, right: f64) -> bool;",
                true,
            ),
        ] {
            check(
                &format!(
                    "{declaration}
                machine subtract(left: u64, right: u64) -> u64
                requires {condition}; {{ left - right }}"
                ),
                accepted,
            );
        }
    }
}

fn check_dependent_product(declaration: &str, accepted: bool) {
    check(
        &format!(
            "{declaration}
            data Grid {{ rows: u32; cols: u32; j: u32; k: u32; }}
            machine Grid::walk(&self)
            requires (self.rows as u64) * (self.cols as u64) <= 12 {{
                transition self.j < self.rows && self.k < self.cols {{
                    true -> read(self.j, self.k)
                    _ -> done()
                }}
                state read(&self, y: u32[0..self.rows], x: u32[0..self.cols]) {{
                    let index: u32[0..=11] = y * self.cols + x;
                }}
                state done(&self) {{}}
            }}"
        ),
        accepted,
    );
}

#[test]
fn dependent_product_requires_need_builtin_comparison() {
    check_dependent_product(
        "operator <= u64::custom(left: u64, right: u64) -> bool;",
        false,
    );
}

#[test]
fn dependent_product_requires_need_builtin_product() {
    check_dependent_product(
        "operator * u64::custom(left: u64, right: u64) -> u64;",
        false,
    );
}

#[test]
fn dependent_product_requires_keep_builtin_and_unrelated_meanings() {
    check_dependent_product("", true);
    check_dependent_product(
        "operator * f64::unrelated(left: f64, right: f64) -> f64;",
        true,
    );
}

fn check_bound_arithmetic(spelling: &str, condition: &str, result: &str, carrier: &str) {
    for (declaration, accepted) in [
        (
            format!("operator {spelling} f64::unrelated(left: f64, right: f64) -> f64;"),
            true,
        ),
        (
            format!(
                "operator {spelling} {carrier}::custom(left: {carrier}, right: {carrier}) -> {carrier};"
            ),
            false,
        ),
    ] {
        check(
            &format!(
                "{declaration}
                machine calculate(left: {carrier}, right: {carrier}) -> {carrier}
                requires {condition}; {{ {result} }}"
            ),
            accepted,
        );
    }
}

#[test]
fn joint_addition_requires_need_builtin_bound_subtraction() {
    check_bound_arithmetic("-", "left <= 255 - right", "left + right", "u8");
}

#[test]
fn joint_multiplication_requires_need_builtin_bound_division() {
    check_bound_arithmetic("/", "right >= 1; left <= 255 / right", "left * right", "u8");
}

#[test]
fn joint_subtraction_requires_need_builtin_bound_addition() {
    check_bound_arithmetic(
        "+",
        "right >= 0; -128 + right <= left",
        "left - right",
        "i8",
    );
}

#[test]
fn folded_requires_bounds_need_builtin_constant_arithmetic() {
    check_bound_arithmetic("+", "left >= 1u8 + 0u8", "left - 1", "u8");
    check_bound_arithmetic("+", "left >= (1u8 + 0u8) * 1u8", "left - 1", "u8");
}
