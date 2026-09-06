use super::*;

fn check(source: &str, accepted: bool) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    match lower_typed_trees(typed) {
        Ok(_) => assert!(accepted, "unproved joint ranking accepted: {source}"),
        Err(diagnostics) => {
            assert!(!accepted, "{diagnostics:#?}\n{source}");
            assert!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("machine call cycle")),
                "expected call-cycle rejection: {diagnostics:#?}\n{source}"
            );
        }
    }
}

fn pair(first: &str, second: &str, second_view: &str, second_parameter: &str) -> String {
    format!(
        "data Progress {{ outer: u64; inner: u64; }}
         measure Progress::Steps lexicographic {{ outer, inner }}
         measure Progress::Reverse lexicographic {{ inner, outer }}
         data Main {{}}
         machine Main::main(&mut self) -> u64 {{
             transition {{ _ -> self.scan_a(Progress {{ outer: 1, inner: 4 }}) }}
         }}
         machine Main::scan_a(&mut self, progress: Progress) -> u64
         terminates by progress -> Progress::Steps;
         {{ {first} }}
         machine Main::scan_b(&mut self, {second_parameter}: Progress) -> u64
         terminates by {second_parameter} -> Progress::{second_view};
         {{ {second} }}"
    )
}

#[test]
fn unchanged_step_is_legal_when_every_cycle_strictly_descends() {
    for parameter in ["progress", "remaining"] {
        check(&pair(
            "transition progress.inner > 0 { true -> self.scan_b(progress) false -> 0 }",
            &format!("transition {parameter}.inner > 0 {{
                 true -> self.scan_a(Progress {{ outer: {parameter}.outer, inner: {parameter}.inner - 1 }})
                 false -> 0
             }}"),
            "Steps", parameter,
        ), true);
    }
}

#[test]
fn earlier_strict_component_permits_a_later_reset() {
    check(
        &pair(
            "transition progress.outer > 0 { true -> self.scan_b(progress) false -> 0 }",
            "transition remaining.outer > 0 {
             true -> self.scan_a(Progress { outer: remaining.outer - 1, inner: 100 })
             false -> 0
         }",
            "Steps",
            "remaining",
        ),
        true,
    );
}

#[test]
fn a_cycle_of_unchanged_arguments_has_no_progress() {
    check(
        &pair(
            "transition progress.inner > 0 { true -> self.scan_b(progress) false -> 0 }",
            "transition remaining.inner > 0 { true -> self.scan_a(remaining) false -> 0 }",
            "Steps",
            "remaining",
        ),
        false,
    );
}

#[test]
fn different_declared_orders_cannot_supply_one_joint_rank() {
    check(
        &pair(
            "transition progress.inner > 0 { true -> self.scan_b(progress) false -> 0 }",
            "transition remaining.inner > 0 {
             true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
             false -> 0
         }",
            "Reverse",
            "remaining",
        ),
        false,
    );
}

#[test]
fn a_decreasing_rank_does_not_admit_non_tail_recursion() {
    check(
        &pair(
            "transition progress.inner > 0 { true -> (1 + self.scan_b(progress)) false -> 0 }",
            "transition remaining.inner > 0 {
             true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
             false -> 0
         }",
            "Steps",
            "remaining",
        ),
        false,
    );
}

#[test]
fn ranking_arithmetic_requires_the_builtin_operator_meaning() {
    let body = pair(
        "transition progress.inner > 0 { true -> self.scan_b(progress) false -> 0 }",
        "transition remaining.inner > 0 {
             true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
             false -> 0
         }",
        "Steps",
        "remaining",
    );
    for (declaration, accepted) in [
        (
            "operator - u64::replacement(left: u64, right: u64) -> u64;",
            false,
        ),
        (
            "operator > u64::replacement(left: u64, right: u64) -> bool;",
            false,
        ),
        (
            "operator - f64::unrelated(left: f64, right: f64) -> f64;",
            true,
        ),
    ] {
        check(&format!("{declaration}\n{body}"), accepted);
    }
}

#[test]
fn a_strict_edge_elsewhere_does_not_admit_an_unchanged_subcycle() {
    let mut source = pair(
        "transition { _ -> self.scan_b(progress) }",
        "transition remaining.inner > 0 {
             true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
             false -> self.scan_c(remaining)
         }",
        "Steps",
        "remaining",
    );
    source.push_str(
        "machine Main::scan_c(&mut self, progress: Progress) -> u64
         terminates by progress -> Progress::Steps;
         { transition { _ -> self.scan_b(progress) } }",
    );
    check(&source, false);
}

#[test]
fn one_strict_occurrence_cannot_hide_a_forwarding_alternative() {
    check(
        &pair(
            "transition { _ -> self.scan_b(progress) }",
            "transition remaining.inner > 0 {
             true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
             false -> self.scan_a(remaining)
         }",
            "Steps",
            "remaining",
        ),
        false,
    );
}

#[test]
fn an_entry_relative_rank_is_not_replayed_after_a_parameter_write() {
    check(
        &pair(
            "transition { _ -> self.scan_b(progress) }",
            "remaining.inner = 4;
             transition remaining.inner > 0 {
                 true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
                 false -> 0
             }",
            "Steps",
            "remaining",
        ),
        false,
    );
}

#[test]
fn an_unrelated_inert_local_does_not_erase_the_joint_rank() {
    check(
        &pair(
            "let unrelated: bool = true;
             transition { _ -> self.scan_b(progress) }",
            "transition remaining.inner > 0 {
                 true -> self.scan_a(Progress { outer: remaining.outer, inner: remaining.inner - 1 })
                 false -> 0
             }",
            "Steps",
            "remaining",
        ),
        true,
    );
}

#[test]
fn a_declared_rank_cannot_fall_back_to_scalar_operator_spelling() {
    check(
        "data Progress { outer: u64; inner: u64; }
         measure Progress::Steps lexicographic { outer, inner }
         data Main {}
         machine Main::scan_a(&mut self, remaining: u64) -> u64
         terminates by remaining -> Progress::Steps;
         { transition remaining == 0 { true -> 0 false -> self.scan_b(remaining - 1) } }
         machine Main::scan_b(&mut self, remaining: u64) -> u64
         terminates by remaining -> Progress::Steps;
         { transition remaining == 0 { true -> 0 false -> self.scan_a(remaining - 1) } }",
        false,
    );
}
