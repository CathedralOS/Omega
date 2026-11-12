//! Which checker's verdict a Product compilation reports while both ship.
//!
//! The owner ruled on 2026-11-12 (`lowered-route-governance-scope`): the
//! current checker judges the Build program and the target's entry-contract
//! source, the checker on the lowered form judges everything else, and a
//! compilation routes on the new verdict only when both accept
//! (`wiki/drafts/designs/check_on_lowered_form.md`, "The gate"). These run the
//! product check itself, so they pin what the default compile reports, not
//! what a harness computes beside it.
//!
//! A governed compilation still fails: no product exists yet for a program
//! the current checker rejects. So each case is told apart by whether the
//! failure opens with the governing statement. Both cases here keep the
//! current verdict. While the two checkers agree no program is governed; the
//! one known exception, `fail/machines/duplicate_local_in_one_body`, is a
//! disagreement the route has yet to close (a local declared twice).
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use lowered_discharge::{RouteVerdict, judge};
use structure_elaboration::{Compilation, elaborate};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const GOVERNS: &str = "the checker on the lowered form accepts this program";

/// The current checker's rejection of `fixture` as the product check reports
/// it, and the route's own verdict on what that check claims.
fn rejected(fixture: &str) -> (Vec<String>, RouteVerdict) {
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
    let FixtureFrontierCheck::Rejected {
        frontier,
        diagnostics,
    } = checked
    else {
        panic!("{fixture}: the current checker must reject this program after Psi 04");
    };
    let verdict = judge(elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subject: frontier.subject,
            selected: &frontier.selected,
        },
    ))
    .expect("the route reaches a verdict");
    (
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect(),
        verdict,
    )
}

/// Both checkers reject, so the current verdict stands unchanged.
#[test]
fn a_rejection_both_checkers_make_is_reported_as_the_current_checker_reports_it() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (messages, verdict) =
        rejected("tests/omega/fail/termination/productive_scalar_loop_arrival_unproved");
    assert!(
        matches!(verdict, RouteVerdict::Rejected { .. }),
        "{verdict:?}"
    );
    assert!(
        messages.iter().all(|message| !message.contains(GOVERNS)),
        "{messages:#?}"
    );
}

/// The route accepts, but the current checker's rejection is the entry
/// contract's (a bound entry that returns a value, placed at the binding in
/// the Build program), which the current checker judges. Its verdict stands.
#[test]
fn a_rejection_the_build_program_owns_keeps_the_current_verdict_even_when_the_route_accepts() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (messages, verdict) =
        rejected("tests/omega/fail/build/unrequested_target_entry_returns_value");
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
    assert!(
        messages.iter().all(|message| !message.contains(GOVERNS)),
        "{messages:#?}"
    );
}
