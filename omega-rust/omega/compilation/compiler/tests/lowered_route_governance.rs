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
//! A program the route accepts and the current checker rejects outside the
//! Build program and the entry contract fails with an error that opens with
//! `NO_PRODUCT_ROUTE`: no product exists yet for it. No corpus program is
//! known to reach that case now that name resolution refuses a repeated
//! local, so the unit tests of `lowered_route::govern` pin it, with the rest
//! of the decision.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use lowered_discharge::{RouteVerdict, judge_compilation};
use structure_elaboration::Compilation;

/// Kept in step with `compiler`'s `lowered_route::NO_PRODUCT_ROUTE`.
const NO_PRODUCT_ROUTE: &str = "no product route yet";

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
        panic!("{fixture}: the compilation must be rejected after Psi 04");
    };
    let mut verdicts = judge_compilation(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subject: frontier.subject,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
    )
    .into_iter();
    let verdict = verdicts
        .next()
        .expect("the compilation has a primary judgment")
        .expect("the route reaches a verdict");
    assert!(
        verdicts.next().is_none(),
        "these fixtures have no generic check forms"
    );
    (
        diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect(),
        verdict,
    )
}

/// Both checkers reject a loop whose arrival invariant is unproved: the
/// route judges it, so the route's diagnostics are reported.
#[test]
fn a_rejection_both_checkers_make_is_reported_as_the_route_reports_it() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (messages, verdict) =
        rejected("tests/omega/fail/termination/productive_scalar_loop_arrival_unproved");
    let RouteVerdict::Rejected { unproved, .. } = &verdict else {
        panic!("{verdict:?}");
    };
    let route: Vec<String> = unproved
        .iter()
        .filter(|entry| !entry.cause.is_incomplete())
        .map(|entry| entry.message())
        .collect();
    assert!(!route.is_empty(), "{verdict:?}");
    assert!(
        route.iter().all(|message| messages.contains(message)),
        "{messages:#?}"
    );
    assert!(
        messages
            .iter()
            .all(|message| !message.starts_with(NO_PRODUCT_ROUTE)),
        "{messages:#?}"
    );
}

/// A bound entry that returns a value fails both the current entry check and
/// the route's source-owned calling shape. The current rejection stands and
/// the route's independently established diagnostic remains visible.
#[test]
fn an_invalid_bound_entry_reports_both_checkers_rejections() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (messages, verdict) =
        rejected("tests/omega/fail/build/unrequested_target_entry_returns_value");
    let RouteVerdict::Rejected { formations, .. } = verdict else {
        panic!("the entry calling shape must reject: {verdict:?}");
    };
    assert!(!formations.is_empty());
    assert!(
        formations
            .iter()
            .all(|failure| messages.contains(&failure.detail)),
        "{messages:#?}"
    );
    assert!(
        messages
            .iter()
            .all(|message| !message.starts_with(NO_PRODUCT_ROUTE)),
        "{messages:#?}"
    );
}
