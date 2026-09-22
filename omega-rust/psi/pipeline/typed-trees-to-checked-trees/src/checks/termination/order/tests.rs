//! A canonical ranking view is keyed on its catalog identity and consulted at
//! its declaration: a program that declares its own `Nat::Descending` does
//! not turn the spelling into the builtin natural countdown.

use crate::tests::front_end::checked_program_result;

fn check(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    checked_program_result(source).map(|_| ())
}

const COUNTDOWN: &str = r#"
machine walk(remaining: u32 [1..=5])
terminates by remaining -> Nat::Descending in 1..=5;
-> u32 {
    transition remaining > 1 {
        true -> walk(remaining - 1)
        false -> remaining
    }
}
"#;

#[test]
fn a_source_free_program_keeps_the_builtin_reading() {
    // No declaration at the catalog path: the canonical identity stands.
    check(COUNTDOWN).expect("builtin natural countdown");
}

#[test]
fn a_declared_lookalike_at_the_catalog_path_is_not_the_builtin_view() {
    // A bodied user declaration spelled `Nat::Descending` is not the sealed
    // toolchain declaration, so the explicit view no longer selects the
    // builtin countdown and the machine's termination is unproven.
    let source = format!(
        "data Nat {{ value: u64; }}
         machine Nat::Descending(value: u64) -> u64 {{ value }}
         {COUNTDOWN}"
    );
    let diagnostics = check(&source).expect_err("a lookalike declaration grants no ranking view");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("cannot prove the `terminates by` ranking for machine walk")),
        "{diagnostics:#?}"
    );
}
