//! Checked lowering only: deliberately nonterminating controls are never run.

use super::*;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize scalar ranking");
    let syntax = parse_syntax_trees(&tokens).expect("parse scalar ranking");
    let resolved = lower_syntax_trees(&syntax).expect("resolve scalar ranking");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type scalar ranking");
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "unproved scalar call cycle accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("machine call cycle")),
                "must reject the cycle, not an unrelated source error: {diagnostics:#?}\n{source}"
            );
        }
    }
}

fn pair(mutable: bool, prefix: &str, first_argument: &str, second_argument: &str) -> String {
    let binding = if mutable {
        "mut remaining"
    } else {
        "remaining"
    };
    format!("data Main {{}}
        machine Main::main(&mut self) -> u64 {{ transition {{ _ -> self.scan_a(1) }} }}
        machine Main::scan_a(&mut self, {binding}: u64) -> u64
        terminates by remaining;
        {{ {prefix} transition remaining == 0 {{ true -> 0 false -> self.scan_b({first_argument}) }} }}
        machine Main::scan_b(&mut self, {binding}: u64) -> u64
        terminates by remaining;
        {{ {prefix} transition remaining == 0 {{ true -> 0 false -> self.scan_a({second_argument}) }} }}")
}

#[test]
fn resetting_a_mutable_rank_before_each_call_is_not_descent() {
    check(
        &pair(true, "remaining = 2;", "remaining - 1", "remaining - 1"),
        false,
    );
}

#[test]
fn declared_subtraction_is_not_builtin_rank_evidence() {
    check(
        &format!(
            "operator - u64::replacement(left: u64, right: u64) -> u64;\n{}",
            pair(false, "", "remaining - 1", "remaining - 1")
        ),
        false,
    );
}

#[test]
fn genuine_scalar_countdown_preserves_immutable_and_mutable_bindings() {
    for mutable in [false, true] {
        check(&pair(mutable, "", "remaining - 1", "remaining - 1"), true);
    }
}

#[test]
fn unrelated_operator_and_inert_local_do_not_erase_scalar_evidence() {
    check(
        &format!(
            "operator - f64::unrelated(left: f64, right: f64) -> f64;\n{}",
            pair(
                false,
                "let unrelated: bool = true;",
                "remaining - 1",
                "remaining - 1"
            )
        ),
        true,
    );
}

#[test]
fn forwarding_decreases_over_the_complete_cycle_but_stalled_cycles_reject() {
    check(&pair(false, "", "remaining", "remaining - 1"), true);
    check(&pair(false, "", "remaining", "remaining"), false);
}

#[test]
fn measured_scalar_recursion_still_requires_tail_calls() {
    let source = pair(false, "", "remaining - 1", "remaining - 1").replace(
        "false -> self.scan_b(remaining - 1)",
        "false -> 1 + self.scan_b(remaining - 1)",
    );
    check(&source, false);
}

#[test]
fn disjoint_local_stores_preserve_the_exact_rank() {
    check(
        &pair(
            true,
            "let mut unrelated: u64 = 0; unrelated = 2;",
            "remaining - 1",
            "remaining - 1",
        ),
        true,
    );
    check(
        &pair(
            true,
            "let mut unrelated: u64 = 0; unrelated = 1u64 + 2u64;",
            "remaining",
            "remaining - 1",
        ),
        true,
    );
}

#[test]
fn a_disjoint_store_cannot_hide_a_rank_mutating_argument_call() {
    let source = format!(
        "{} machine Main::reset(&mut self, value: &mut u64) -> u64 {{ value = 2; 0 }}",
        pair(
            true,
            "let mut unrelated: u64 = 0; unrelated = self.reset(&mut remaining);",
            "remaining - 1",
            "remaining - 1",
        ),
    );
    check(&source, false);
}

#[test]
fn a_disjoint_store_cannot_hide_an_authored_arithmetic_effect() {
    let source = format!(
        "operator + u64::replacement(left: u64, right: u64) -> u64; {}",
        pair(
            true,
            "let mut unrelated: u64 = 0; unrelated = 1u64 + 2u64;",
            "remaining - 1",
            "remaining - 1",
        ),
    );
    check(&source, false);
}

#[test]
fn authored_zero_equality_cannot_supply_a_base_arm_fact() {
    check(
        &format!(
            "operator == u64::replacement(left: u64, right: u64) -> bool;\n{}",
            pair(false, "", "remaining - 1", "remaining - 1")
        ),
        false,
    );
}

#[test]
fn an_additional_same_pair_call_cannot_hide_behind_a_strict_tail_call() {
    let source = pair(false, "", "remaining - 1", "remaining - 1").replace(
        "false -> self.scan_b(remaining - 1)",
        "false -> 0 + self.scan_b(remaining) + self.scan_b(remaining - 1)",
    );
    check(&source, false);
}

#[test]
fn non_first_rank_arguments_bind_exact_renamed_parameters() {
    check(
        "data Main {}
        machine Main::main(&mut self) -> u64 { transition { _ -> self.scan_a(true, 1) } }
        machine Main::scan_a(&mut self, payload: bool, remaining: u64) -> u64
        terminates by remaining;
        { transition remaining == 0 { true -> 0 false -> self.scan_b(payload, remaining - 1) } }
        machine Main::scan_b(&mut self, carried: bool, countdown: u64) -> u64
        terminates by countdown;
        { transition countdown == 0 { true -> 0 false -> self.scan_a(carried, countdown - 1) } }",
        true,
    );
}

#[test]
fn qualified_non_tail_alternative_cannot_borrow_a_strict_pair_summary() {
    let source = pair(false, "", "remaining - 1", "remaining - 1").replace(
        "false -> self.scan_b(remaining - 1)",
        "false -> 0 + Main::scan_b(self, remaining) + self.scan_b(remaining - 1)",
    );
    check(&source, false);
}
