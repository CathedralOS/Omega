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

const DECISIVE: &str =
    include_str!("../../../tests/fixtures/synchronization/run/decisive_atomic_lock/main.omg");

#[test]
fn a_decisive_exchange_acquires_the_released_flag() {
    program::assert_silent_return(DECISIVE);
}

#[test]
fn a_decisive_exchange_keeps_observing_a_private_held_flag() {
    let held = DECISIVE.replace("flag.held.store(0, Publish);", "");
    let (outcome, diagnostic) = program::invoke(&held, &["--check"]);
    assert!(!outcome.status.success(), "{diagnostic}");
    assert!(
        diagnostic.contains("synchronization deadlock"),
        "{diagnostic}"
    );
}

#[test]
fn a_failed_exchange_cannot_write_the_value_that_would_release_its_wait() {
    let source = DECISIVE
        .replace("compare_exchange(0, 1,", "compare_exchange(0, 0,")
        .replace(
            "    acquire(@flag);\n    flag.held.store(0, Publish);",
            "    flag.held.store(1, Publish);",
        );
    let (outcome, diagnostic) = program::invoke(&source, &["--check"]);
    assert!(!outcome.status.success(), "{diagnostic}");
    assert!(
        diagnostic.contains("synchronization deadlock"),
        "{diagnostic}"
    );
}

#[test]
fn a_decisive_exchange_must_receive_the_observing_outcome() {
    let template = r#"
        use {omega}/language/core/layout.omg;
        data Flag { held: AtomicU32; }
        data Impostor [copy] { case Mismatched(observed: u32); case Exchanged; }
        machine main() {
            let mutable flag: Flag;
            let outcome: RESULT = flag.held.compare_exchange(0, 1, ReceivePublish, Receive);
        }
    "#;
    for wrong in [
        "u32",
        "Impostor",
        "|layout|::AtomicCompareExchangeOutcome<u64>",
        "|layout|::AtomicCompareExchangeOnceOutcome<u32>",
    ] {
        let source = template.replace("RESULT", wrong);
        let (outcome, diagnostic) = program::invoke(&source, &["--check"]);
        assert!(!outcome.status.success(), "{wrong}: {diagnostic}");
        assert!(
            diagnostic.contains("AtomicCompareExchangeOutcome"),
            "{wrong}: {diagnostic}"
        );
    }
}

#[test]
fn a_mismatch_returns_the_observed_word_and_leaves_the_cell_unchanged() {
    let source = r#"
        use {omega}/language/core/layout.omg;
        data Flag { held: AtomicU32; }

        machine main() crashes Trap {
            let mutable flag: Flag;
            flag.held.store(17, Publish);
            let mutable attempt: |layout|::AtomicCompareExchangeOutcome<u32> =
                |layout|::AtomicCompareExchangeOutcome::Exchanged;
            attempt = flag.held.compare_exchange(0, 9, ReceivePublish, Receive);
            transition attempt {
                |layout|::AtomicCompareExchangeOutcome::Exchanged -> bad(),
                |layout|::AtomicCompareExchangeOutcome::Mismatched { observed } ->
                    check(@flag, observed)
            }
            machine check(flag: @Flag, observed: u32) crashes Trap {
                let stored: u32 = flag.held.load(Receive);
                transition observed == 17 && stored == 17 {
                    true -> done(),
                    _ -> bad()
                }
            }
            machine done() {}
            machine bad() crashes Trap { crash Trap; }
        }
    "#;
    program::assert_silent_return(source);
}

#[test]
fn an_exchange_in_the_entry_supplies_the_actual_bits_and_case_to_a_later_wait() {
    let (library, _) = DECISIVE.split_once("machine main()").unwrap();
    let entry = r#"
        machine main() {
            let mutable flag: Flag;
            flag.held.store(1, Publish);
            let first: |layout|::AtomicCompareExchangeOutcome<u32> =
                flag.held.compare_exchange(0, 0, ReceivePublish, Receive);
            transition first {
                |layout|::AtomicCompareExchangeOutcome::Exchanged -> finished(),
                |layout|::AtomicCompareExchangeOutcome::Mismatched { observed } ->
                    retry(@flag, observed)
            }
            machine retry(flag: @Flag, observed: u32) {
                transition observed == 1 {
                    true -> acquire(flag),
                    _ -> finished()
                }
            }
            machine finished() {}
        }
    "#;
    let (outcome, diagnostic) = program::invoke(&format!("{library}{entry}"), &["--check"]);
    assert!(!outcome.status.success(), "{diagnostic}");
    assert!(
        diagnostic.contains("synchronization deadlock"),
        "{diagnostic}"
    );

    // With the expected value corrected, the same operation releases the
    // flag, returns Exchanged, and the caller's mismatch arm cannot run.
    let released = entry
        .replace("compare_exchange(0, 0,", "compare_exchange(1, 0,")
        .replace("::Exchanged -> finished()", "::Exchanged -> acquire(@flag)");
    program::assert_silent_return(&format!("{library}{released}"));
}

#[test]
fn atomic_read_modify_write_returns_the_old_word_and_installs_the_typed_result() {
    let source = include_str!(
        "../../../tests/fixtures/synchronization/reject/read_modify_write_before_wait/main.omg"
    );
    for (initial, operation) in [
        (0, "fetch_or(1"),
        (3, "fetch_and(1"),
        (u32::MAX, "fetch_xor(4294967294"),
        (u32::MAX, "fetch_add(2"),
        (0, "fetch_sub(4294967295"),
    ] {
        let held = source
            .replace("held.store(0", &format!("held.store({initial}"))
            .replace("fetch_or(1", operation)
            .replace("prior == 0", &format!("prior == {initial}"));
        let (output, diagnostic) = program::invoke(&held, &["--check"]);
        assert!(!output.status.success(), "{operation}: {diagnostic}");
        assert!(
            diagnostic.contains("synchronization deadlock"),
            "{operation}: {diagnostic}"
        );
    }
    // Updating a different cell leaves the flag clear. Clearing a flag that
    // started held enables the same wait and still returns the old word.
    program::assert_silent_return(&source.replace("held.fetch_or", "other.fetch_or"));
    program::assert_silent_return(
        &source
            .replace("held.store(0", "held.store(1")
            .replace("fetch_or(1", "fetch_and(0")
            .replace("prior == 0", "prior == 1"),
    );
}
