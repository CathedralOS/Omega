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

const ORDERED_ASSIGNMENT: &str = "use {omega}/language/core/layout.omg;
data Cells { value: AtomicU32; calls: AtomicU32; }
machine next(cells: @Cells) -> u32 {
    let previous: u32 = cells.calls.fetch_add(1, NoOrdering);
    previous
}
machine attempt(cells: @Cells) -> bool {
    let mutable outcome: |layout|::AtomicCompareExchangeOnceOutcome<u32> =
        |layout|::AtomicCompareExchangeOnceOutcome::Exchanged;
    outcome = cells.value.compare_exchange_once(next(cells), next(cells), ReceivePublish, Receive);
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
}";

#[test]
fn exchange_evaluates_the_expected_and_replacement_once_in_source_order() {
    program::assert_silent_return(ORDERED_ASSIGNMENT);
}

#[test]
fn a_saved_program_can_replace_its_owned_exchange_result() {
    let outputs = platform_custody::temporary_directory::TemporaryDirectory::create(
        "saved-exchange-result",
        false,
    )
    .unwrap();
    let saved = outputs.path().join("program.psi");
    let (output, report) =
        program::invoke(ORDERED_ASSIGNMENT, &["--emit-psi", saved.to_str().unwrap()]);
    assert!(output.status.success(), "{report}");
    let bytes = std::fs::read(saved).unwrap();
    terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
        .expect("the received artifact admits the same owned assignment as its producer");
}
