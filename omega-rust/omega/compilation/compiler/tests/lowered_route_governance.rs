//! The lowered-form checker's verdict is the product compilation's verdict.
//!
//! The owner ruled on 2026-11-12 that the old checker is removed now
//! (`wiki/drafts/designs/check_on_lowered_form.md`, "The gate"): the checker
//! on the lowered form decides every product compilation before Psi 05 is
//! given the trees, and a program it cannot judge is refused. These run the
//! product check itself, so they pin what the default compile reports, not
//! what a harness computes beside it. The unit tests of `lowered_route` pin
//! the refusal and the "no product route yet" case.
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use fixture_package_inputs::{
    FixtureProductCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use terminal_psi_to_terminal_psi::RouteVerdict;
use typed_trees_to_terminal_psi::Compilation;

/// Kept in step with `compiler`'s `lowered_route::NO_PRODUCT_ROUTE`.
const NO_PRODUCT_ROUTE: &str = "no product route yet";

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The rejection of `fixture` as the product check reports it, and the
/// route's own verdict on what that check claims.
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
    let FixtureProductCheck::Rejected {
        frontier,
        diagnostics,
    } = checked
    else {
        panic!("{fixture}: the compilation must be rejected after Psi 04");
    };
    let mut verdicts = terminal_psi_to_terminal_psi::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &frontier.typed,
            Compilation {
                declaration_metadata: &[],
                entries: &frontier.entries,
                subjects: &[frontier.subject],
                purpose: source::DependencyScope::Product,
                selected: &frontier.selected,
                bindings: &frontier.bindings,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Portable,
        )
        .program,
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

/// A loop whose arrival invariant is unproved is rejected with the route's
/// diagnostics.
#[test]
fn a_rejection_is_reported_as_the_route_reports_it() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (messages, verdict) =
        rejected("tests/fixtures/fail/termination/productive_scalar_loop_arrival_unproved");
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

/// Bound entries owe both their source-owned calling shape and every
/// continuation precondition, including bindings outside the requested target.
/// The product check reports the route's actual rejection in each case.
#[test]
fn an_invalid_bound_entry_reports_the_routes_rejection() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (messages, verdict) =
        rejected("tests/fixtures/fail/build/unrequested_target_entry_returns_value");
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

    for (fixture, callee, clause, expected_calls) in [
        (
            "tests/fixtures/fail/build/program_entry_unestablished_requires",
            "launch",
            "false",
            3,
        ),
        (
            "tests/fixtures/fail/build/unrequested_target_entry_unestablished_requires",
            "other",
            "0 == 1",
            1,
        ),
        (
            "tests/fixtures/fail/build/program_entry_named_predicate_requires",
            "launch",
            "is_zero(1)",
            4,
        ),
    ] {
        let (messages, verdict) = rejected(fixture);
        let RouteVerdict::Rejected {
            formations,
            unproved,
            ..
        } = verdict
        else {
            panic!("the installed call must reject: {verdict:?}");
        };
        assert!(formations.is_empty(), "the calling shape is valid");
        assert_eq!(
            unproved.len(),
            expected_calls,
            "each binding owes its own call"
        );
        for failure in unproved {
            assert!(!failure.cause.is_incomplete(), "{failure:?}");
            let message = failure.message();
            assert!(
                message.contains(&format!("requires contract for call {callee}")),
                "{message}"
            );
            assert!(message.contains(clause), "{message}");
            assert!(messages.contains(&message), "{messages:#?}");
        }
        assert!(
            messages
                .iter()
                .all(|message| !message.starts_with(NO_PRODUCT_ROUTE)),
            "{messages:#?}"
        );
    }
}

/// A named predicate owes total arithmetic under its own formals even when no
/// machine uses it. `value + 1u32` can overflow, so the checker refuses the
/// declaration and places the refusal there. The positive twin differs only
/// in `+%`, whose wrapping addition is total.
#[test]
fn a_named_predicate_is_formed_at_its_declaration() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let (messages, verdict) =
        rejected("tests/fixtures/fail/proofs/unused_partial_mathematical_predicate");
    let RouteVerdict::Rejected { violations, .. } = verdict else {
        panic!("the partial predicate must be refused: {verdict:?}");
    };
    let [violation] = violations.as_slice() else {
        panic!("one declaration owes one refusal: {violations:?}");
    };
    assert!(
        violation.statement.starts_with(
            "named predicate `reflexive`: contract arithmetic is not proved total before its condition"
        ),
        "{violation:?}"
    );
    let site = violation
        .site
        .expect("the refusal is placed at the declaration");
    let main = repo_root()
        .join("tests/fixtures/fail/proofs/unused_partial_mathematical_predicate/main.omg");
    let source = std::fs::read_to_string(main).unwrap();
    assert!(
        source[site.span.start..].starts_with("reflexive("),
        "{site:?} is not the `reflexive` declaration"
    );
    assert!(messages.contains(&violation.message()), "{messages:#?}");

    let main =
        repo_root().join("tests/fixtures/pass/proofs/wrapping_mathematical_predicate/main.omg");
    let checked = std::thread::scope(|scope| {
        compiler::checked::spawn_compile_worker(scope, || {
            check_fixture_keeping_typed_frontier(&main, "linux_x86_64")
        })
        .expect("compiler worker")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    });
    assert!(
        matches!(checked, FixtureProductCheck::Accepted { .. }),
        "the wrapping predicate is total and compiles"
    );
}

#[test]
fn named_predicate_terms_form_under_their_own_inputs() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let source = std::fs::read_to_string(
        repo_root().join("tests/fixtures/pass/proofs/wrapping_mathematical_predicate/main.omg"),
    )
    .unwrap();
    for (name, program, accepted, dependency) in [
        ("unused-wrapping", source.clone(), true, false),
        (
            "called-wrapping",
            source
                .replace("value +% 1u32 == 1u32", "value +% 1u32 == 0u32")
                .replace(
                    "requires is_zero(0u32)",
                    "requires reflexive(4294967295u32)",
                ),
            true,
            false,
        ),
        // A safe actual argument cannot repair a declaration over arbitrary u32.
        (
            "called-exact",
            source
                .replace("+%", "+")
                .replace("requires is_zero(0u32)", "requires reflexive(0u32)"),
            false,
            false,
        ),
        (
            "nonzero-divisor",
            source.replace("value +% 1u32", "value /% 1u32"),
            true,
            false,
        ),
        (
            "possibly-zero-divisor",
            source.replace("value +% 1u32", "1u32 /% value"),
            false,
            false,
        ),
        (
            "dependency-wrapping",
            source.replace("requires is_zero(0u32)", "requires reflexive(0u32)"),
            true,
            true,
        ),
        (
            "dependency-exact",
            source
                .replace("+%", "+")
                .replace("requires is_zero(0u32)", "requires reflexive(0u32)"),
            false,
            true,
        ),
    ] {
        let frontier = unbuilt_frontier::staged_frontier(name, &program);
        let entry = unbuilt_frontier::program_machine(&frontier.typed, "launch").unwrap();
        // As in lowered_route_subjects, check the entry on behalf of another
        // package: only declarations it reaches belong to this closure.
        let subject = if dependency {
            let another = semantic_vocabulary::PackageKeyIdentity::from_digest([91; 32]).unwrap();
            assert_ne!(frontier.subject, Some(another));
            Some(another)
        } else {
            frontier.subject
        };
        let verdicts = terminal_psi_to_terminal_psi::judge_compilation(
            typed_trees_to_terminal_psi::prepare_compilation(
                &frontier.typed,
                Compilation {
                    declaration_metadata: &[],
                    entries: &[entry],
                    subjects: &[subject],
                    purpose: source::DependencyScope::Product,
                    selected: &frontier.selected,
                    bindings: &frontier.bindings,
                    supplies: &[],
                },
                terminal_codec::TerminalPublication::Invocation,
            )
            .program,
        );
        let [verdict] = verdicts.as_slice() else {
            panic!("{name}: {verdicts:?}")
        };
        if accepted {
            let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
                panic!("{name}: {verdict:?}")
            };
            assert_eq!(
                accepted
                    .elaboration
                    .module
                    .scalar_predicate_definitions
                    .len(),
                if dependency { 1 } else { 2 },
                "{name}: each reached declaration has one authoritative definition"
            );
        } else {
            let Ok(RouteVerdict::Rejected { violations, .. }) = verdict else {
                panic!("{name}: {verdict:?}")
            };
            assert!(violations.iter().any(|violation| violation.statement.contains("named predicate `reflexive`: contract arithmetic is not proved total before its condition")), "{name}: {violations:?}");
        }
    }
}
