//! Checked lowering only: deliberately nonterminating controls are never run.
use crate::tests::front_end::checked_program_result;

fn check(source: &str, accepted: bool) {
    match checked_program_result(source) {
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
