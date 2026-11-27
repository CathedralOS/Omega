//! Check one lock implemented with ordinary Omega atomic operations through the CLI.

#[path = "support/program.rs"]
mod program;

const RELEASED: &str =
    include_str!("../../../tests/fixtures/synchronization/run/private_atomic_lock/main.omg");

#[test]
fn releasing_the_flag_allows_reacquisition_in_psi_and_native_code() {
    program::assert_silent_return(RELEASED);
}

#[test]
fn reacquiring_a_private_flag_that_is_still_held_rejects_before_execution() {
    let held = RELEASED.replace("flag.held.store(0, Publish);", "");
    let (outcome, diagnostic) = program::invoke(&held, &["--check"]);
    assert!(!outcome.status.success(), "{diagnostic}");
    assert!(
        diagnostic.contains("synchronization deadlock"),
        "{diagnostic}"
    );
}

#[test]
fn a_forwarding_machine_preserves_the_lock_across_both_calls() {
    let source = RELEASED
        .replace(
            "machine main()",
            "machine enter(flag: @Flag) { acquire(flag); }\nmachine main()",
        )
        .replace("acquire(@flag);", "enter(@flag);")
        .replace("flag.held.store(0, Publish);", "");
    let (outcome, diagnostic) = program::invoke(&source, &["--check"]);
    assert!(!outcome.status.success(), "{diagnostic}");
    assert!(
        diagnostic.contains("synchronization deadlock"),
        "{diagnostic}"
    );
}
