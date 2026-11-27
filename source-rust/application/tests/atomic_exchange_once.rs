//! A single attempt publishes the value the instruction observed or retained.
//! Exercise the same storage through a shared borrow and check that neither a
//! look-alike result nor another resident type can receive the event.
#[path = "support/program.rs"]
mod program;

const OBSERVING: &str =
    include_str!("../../../samples/cli/facts/atomic_compare_exchange_once/main.omg");
const RETAINING: &str =
    include_str!("../../../samples/cli/facts/atomic_try_exchange_once/local/main.omg");

#[test]
fn compare_exchange_reports_its_commit_and_observation() {
    program::assert_silent_return(OBSERVING);
}

#[test]
fn try_exchange_returns_the_proposal_or_displaced_value() {
    program::assert_silent_return(RETAINING);
}

#[test]
fn a_lookalike_result_cannot_receive_an_atomic_outcome() {
    let source = OBSERVING
        .replace(
            "use {omega}/language/core/layout.omg;",
            "data AtomicCompareExchangeOnceOutcome<T [copy]> [copy] {
        case Mismatched(observed: T);
        case Exchanged;
        case Uncommitted(observed: T);
    }",
        )
        .replace("|layout|::", "");
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("exact core outcome"), "{report}");
}

#[test]
fn an_outcome_with_another_resident_cannot_receive_the_event() {
    let source = OBSERVING.replace("Outcome<u32>", "Outcome<u64>");
    let (output, report) = program::invoke(&source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("exact core outcome"), "{report}");
}

#[test]
fn an_authored_record_can_require_a_copyable_type_argument() {
    let source = "data Envelope<T [copy]> [copy] { value: T; }
        machine read(envelope: Envelope<u32>) -> u32 { envelope.value }
        machine main() crashes Trap {
            let envelope: Envelope<u32>;
            transition read(envelope) == 0 { true -> done(), _ -> bad() }
            machine done() {}
            machine bad() { crash Trap; }
        }";
    let (output, report) = program::invoke(source, &["--check"]);
    assert!(output.status.success(), "{report}");
}

#[test]
fn exchange_evaluates_the_expected_and_replacement_once_in_source_order() {
    program::assert_silent_return(
        "use {omega}/language/core/layout.omg;
data Cells { value: AtomicU32; calls: AtomicU32; }
machine next(cells: @Cells) -> u32 {
    let previous: u32 = cells.calls.fetch_add(1, NoOrdering);
    previous
}
machine attempt(cells: @Cells) -> bool {
    let outcome: |layout|::AtomicCompareExchangeOnceOutcome<u32> =
        cells.value.compare_exchange_once(next(cells), next(cells), ReceivePublish, Receive);
    transition outcome {
        |layout|::AtomicCompareExchangeOnceOutcome::Exchanged -> (cells.value.load(NoOrdering) == 1),
        |layout|::AtomicCompareExchangeOnceOutcome::Mismatched { observed } -> (false),
        |layout|::AtomicCompareExchangeOnceOutcome::Uncommitted { observed } -> (observed == 0 && cells.value.load(NoOrdering) == 0)
    }
}
machine main() crashes Trap {
    let cells: Cells;
    transition attempt(@cells) && cells.calls.load(NoOrdering) == 2 { true -> done(), _ -> bad() }
    machine done() {}
    machine bad() { crash Trap; }
}",
    );
}
