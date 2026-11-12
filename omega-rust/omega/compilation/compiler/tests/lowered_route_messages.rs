//! When the checker on the lowered form rejects a program, its message states
//! the condition that was not proved, in the current checker's words: the
//! callee and caller of a requirement with the clause over the caller's
//! actuals, the machine and clause of a guarantee, and the operation, carrier
//! and binder of an arithmetic safety obligation. Each fixture here is a
//! `fail/` program the route rejects, and each message must contain the
//! fragment its `expected.txt` pins, as the corpus runner matches it.
//!
//! Before these names were carried, every message read `unproved obligation:
//! <kind>: <cause>`, and 1 of 292 measured `fail/` rejections contained its
//! pinned fragment (`wiki/drafts/designs/route_one_diagnostics.md`).
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use lowered_discharge::{RouteVerdict, judge};
use structure_elaboration::elaborate_closure;

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Each fixture and the machine whose closure the route rejects.
const REJECTIONS: &[(&str, &str)] = &[
    // A callee clause stated over the caller's actual.
    (
        "tests/omega/fail/arithmetic/bitwise_non_singleton_endpoint_bounds",
        "probe",
    ),
    // The same, where the fixture pins the whole sentence: the fact the
    // callee asked for in its own words, and the caller's state.
    (
        "tests/omega/fail/facts/four_variant_enum_payload_bound_through_local_rejected",
        "main",
    ),
    // A nested callee.
    (
        "tests/omega/fail/calls/state_arrival_call_result_above_bound",
        "main",
    ),
    // An `ensures` clause, authored text over the machine's own formals.
    (
        "tests/omega/fail/arithmetic/truncating_reconstruction_negative_dividend",
        "shifted",
    ),
    // An overflow, with the local the value lands in.
    (
        "tests/omega/fail/arithmetic/unformed_contradiction_cannot_hide_overflow",
        "unsafe",
    ),
    // A division whose divisor is not shown nonzero.
    (
        "tests/omega/fail/arithmetic/exact_divisor_unproven",
        "quotient",
    ),
    // A shift count.
    (
        "tests/omega/fail/arithmetic/boolean_shift_guard_missing_lower_bound",
        "calculate",
    ),
];

/// The route's messages for the closure of the machine `root` names.
fn route_messages(fixture: &str, root: &str) -> Vec<String> {
    let main = repo_root().join(fixture).join("main.omg");
    // Discovery and checking recurse deeply; give them the compiler worker's
    // stack rather than the small one libtest gives each test thread.
    let checked = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    let FixtureFrontierCheck::Rejected { frontier, .. } = checked else {
        panic!("{fixture}: the current checker must reject this program after Psi 04");
    };
    let typed = &frontier.typed;
    let symbol = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == root)
        .unwrap_or_else(|| panic!("{fixture}: no machine `{root}`"))
        .symbol;
    match judge(elaborate_closure(typed, symbol)) {
        Ok(RouteVerdict::Rejected {
            unproved,
            unestablished,
        }) => unproved
            .iter()
            .map(|entry| entry.message())
            .chain(unestablished.iter().map(|entry| entry.message()))
            .collect(),
        verdict => panic!("{fixture}: the route must reject `{root}`: {verdict:?}"),
    }
}

#[test]
fn a_rejection_states_the_condition_the_current_checker_pins() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let mut missing = Vec::new();
    for (fixture, root) in REJECTIONS {
        let expected = std::fs::read_to_string(repo_root().join(fixture).join("expected.txt"))
            .expect("expected.txt");
        let messages = route_messages(fixture, root);
        // The corpus runner's rule: every nonempty line occurs in some
        // message.
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
