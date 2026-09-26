//! A caller supplies the selected proposition, not a same-spelled numeric law.
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;

fn check(source: &str, accepted: bool) {
    match lower_typed_trees(typed_program(source), &CheckingRequest::settled()) {
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
fn folded_requires_bounds_need_builtin_constant_arithmetic() {
    check_bound_arithmetic("+", "left >= 1u8 + 0u8", "left - 1", "u8");
    check_bound_arithmetic("+", "left >= (1u8 + 0u8) * 1u8", "left - 1", "u8");
}
