//! When the checker on the lowered form rejects a program, its message states
//! the condition that was not proved: the callee and caller of a requirement
//! with the clause over the caller's actuals, the machine and clause of a
//! guarantee, and the operation, carrier and binder of an arithmetic safety
//! obligation. Each fixture here is a `fail/` program with a machine the
//! route rejects, and each message must contain the fragment the fixture's
//! `expected.txt` pins: every nonempty line occurs in some message.
//!
//! Before these names were carried, every message read `unproved obligation:
//! <kind>: <cause>`, and 1 of 292 measured `fail/` rejections contained its
//! pinned fragment (`wiki/drafts/designs/route_one_diagnostics.md`).
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use unbuilt_frontier::repository as repo_root_at;
use lowered_discharge::{RouteVerdict, judge};
use structure_elaboration::elaborate_closure;

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Each fixture and the machine whose closure the route rejects.
const REJECTIONS: &[(&str, &str)] = &[
    // A callee clause stated over the caller's actual. The actual is the
    // formal itself: a clause over an exclusive or or a mask is one the
    // route's rules cannot decide, which it reports as `Incomplete`, not as
    // a rejection (`lowered-discharge`, `prove/reach`).
    (
        "tests/fixtures/fail/arithmetic/signed_zero_does_not_prove_positive",
        "check",
    ),
    // A nested callee.
    (
        "tests/fixtures/fail/calls/state_arrival_call_result_above_bound",
        "main",
    ),
    // An `ensures` clause, authored text over the machine's own formals.
    (
        "tests/fixtures/fail/arithmetic/truncating_reconstruction_negative_dividend",
        "shifted",
    ),
    // An overflow, with the local the value lands in.
    (
        "tests/fixtures/fail/arithmetic/unformed_contradiction_cannot_hide_overflow",
        "unsafe",
    ),
    // A division whose divisor is not shown nonzero.
    (
        "tests/fixtures/fail/arithmetic/exact_divisor_unproven",
        "quotient",
    ),
    // A case payload read where no case was selected.
    (
        "tests/fixtures/fail/contracts/case_payload_requires_declared_premise",
        "main",
    ),
    // A shift count.
    (
        "tests/fixtures/fail/arithmetic/boolean_shift_guard_missing_lower_bound",
        "calculate",
    ),
];

/// The route's messages for the closure of the machine `root` names, or the
/// verdict that is not a rejection.
fn route_messages(fixture: &str, root: &str) -> Result<Vec<String>, String> {
    let frontier = unbuilt_frontier::fixture_frontier(fixture);
    let typed = &frontier.typed;
    let symbol = unbuilt_frontier::program_machine(typed, root)
        .unwrap_or_else(|| panic!("{fixture}: no machine `{root}`"));
    match judge(elaborate_closure(typed, symbol)) {
        Ok(RouteVerdict::Rejected {
            unproved,
            unestablished,
            conflicts,
            exceeded,
            ..
        }) => Ok(unproved
            .iter()
            .map(|entry| entry.message())
            .chain(unestablished.iter().map(|entry| entry.message()))
            .chain(conflicts.iter().map(|entry| entry.message()))
            .chain(exceeded.iter().map(|entry| entry.message()))
            .collect()),
        verdict => Err(format!(
            "{fixture}: the route must reject `{root}`: {verdict:?}"
        )),
    }
}

#[test]
fn a_rejection_states_the_condition_its_fixture_pins() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut missing = Vec::new();
    for (fixture, root) in REJECTIONS {
        let expected = std::fs::read_to_string(repo_root_at(fixture).join("expected.txt"))
            .expect("expected.txt");
        let messages = match route_messages(fixture, root) {
            Ok(messages) => messages,
            Err(verdict) => {
                missing.push(verdict);
                continue;
            }
        };
        for line in expected
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            if !messages.iter().any(|message| message.contains(line)) {
                missing.push(format!("{fixture}: `{line}` not in {messages:?}"));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}
