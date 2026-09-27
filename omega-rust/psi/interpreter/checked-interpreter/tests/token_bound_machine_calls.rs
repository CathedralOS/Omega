//! A token-bearing machine (`machine + Wrapped::add`) executes through its own
//! checked body: the checked stage binds the selected `+` use to an ordinary
//! call, so the interpreter runs the declaration like any named call.

use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;

fn interpret_main(source: &str) -> checked_interpreter::InterpretOutcome {
    let checked = crate::front_end::checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:?}"));
    checked_interpreter::interpret_entry(
        &checked,
        BuildMachineEntry::Name("main"),
        &[],
        InterpretOptions::default(),
    )
}

#[test]
fn token_call_runs_the_declaration_body_and_returns_the_wrapped_sum() {
    let outcome = interpret_main(
        "data Wrapped { value: u8; }
         machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
             (left.value as u64) + (right.value as u64)
         }
         machine by_token(left: Wrapped, right: Wrapped) -> u64 { left + right }
         machine main() -> i32 {
             let left: Wrapped = Wrapped { value: 250u8 };
             let right: Wrapped = Wrapped { value: 10u8 };
             transition by_token(left, right) == 260u64 { true -> 7 false -> 11 }
         }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}

#[test]
fn token_and_named_calls_share_one_body() {
    let outcome = interpret_main(
        "data Wrapped { value: u8; }
         machine + Wrapped::add(left: Wrapped, right: Wrapped) -> u64 {
             (left.value as u64) + (right.value as u64)
         }
         machine by_token(left: Wrapped, right: Wrapped) -> u64 { left + right }
         machine by_name(left: Wrapped, right: Wrapped) -> u64 { Wrapped::add(left, right) }
         machine main() -> i32 {
             let a: Wrapped = Wrapped { value: 3u8 };
             let b: Wrapped = Wrapped { value: 4u8 };
             let c: Wrapped = Wrapped { value: 3u8 };
             let d: Wrapped = Wrapped { value: 4u8 };
             transition by_token(a, b) == by_name(c, d) { true -> 7 false -> 11 }
         }",
    );
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.exit_code, 7);
}
