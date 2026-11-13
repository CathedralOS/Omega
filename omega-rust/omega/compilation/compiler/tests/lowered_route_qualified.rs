//! The checker on the lowered form judges a machine declared under a data
//! type's name, `machine Counter::clear(counter: &mut Counter)`, and the calls
//! that name it, instead of refusing every program that has one.
//!
//! `wiki/spec/language/machines.md`, "Static declarations and explicit
//! state": "All machines are static declarations independent of data", and
//! "Every argument, including context and capabilities, is ordinary and
//! explicit." So such a machine is read as any other, and `Counter::clear(x)`
//! is a direct call that passes exactly `x`. The data type in the name
//! supplies nothing, and a type that declares only that name (`data Main {}`)
//! has nothing to judge.
//!
//! Each program below was refused as `Unsupported: machine attached data`
//! before. The call forms differ: a call in an expression, a call statement
//! and a transition target each record the qualifier their own way.
#[path = "support/unbuilt_frontier.rs"]
mod unbuilt_frontier;

use lowered_discharge::{RouteVerdict, judge};
use structure_elaboration::{Compilation, elaborate};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The route's verdict on everything the check of `fixture` claims, with
/// `entry` bound as its `build.omg` binds it.
fn route_verdict(fixture: &str, entry: &str) -> RouteVerdict {
    let frontier = unbuilt_frontier::bound_fixture_frontier(fixture, Some(entry), &[]);
    judge(elaborate(
        &frontier.typed,
        Compilation {
            declaration_metadata: &[],
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
    ))
    .unwrap_or_else(|defect| panic!("{fixture}: the route must reach a verdict: {defect:?}"))
}

/// Programs the route accepts with the entry each one's Build binds, by the
/// call form that names the qualified machine.
const ACCEPTED: &[(&str, &str)] = &[
    // An expression call and a transition target, `Main::countdown(..)`,
    // the second from inside the machine itself.
    (
        "tests/fixtures/pass/calls/machine_self_call_recursion_compile",
        "Main::main",
    ),
    // Call statements and expression calls of five machines under one name,
    // two of them lending a scalar local.
    (
        "tests/fixtures/run/calls/qualified_atomic_operation_names",
        "main",
    ),
    // The qualified machine owes an `ensures` it meets through its callee's.
    (
        "tests/fixtures/pass/calls/attached_terminal_machine_call_exit",
        "start",
    ),
    // The qualified machine declares a nested machine and calls a free one
    // of the same name.
    (
        "tests/fixtures/pass/calls/state_and_machine_names_exit",
        "main",
    ),
    // The qualified machine is the bound entry and ends in a call of a
    // private helper that returns nothing, whose input conditions are
    // inferred from this one arrival.
    (
        "tests/fixtures/pass/calls/attached_context_terminal_machine_call_exit",
        "Main::main",
    ),
];

#[test]
fn a_program_with_data_qualified_machines_is_judged_and_accepted() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut wrong = Vec::new();
    for (fixture, entry) in ACCEPTED {
        let verdict = route_verdict(fixture, entry);
        if !matches!(verdict, RouteVerdict::Accepted(_)) {
            wrong.push(format!("{fixture}: {verdict:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The twin of `attached_terminal_machine_call_exit`: `Main::main` passes 40
/// to a callee that requires 41. The route rejects it at that call, so the qualified machine's body is really judged.
#[test]
fn a_data_qualified_machine_that_breaks_its_callees_requires_is_rejected() {
    let _compilation = FIXTURE_COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fixture = "tests/fixtures/fail/calls/free_machine_named_transition_rejected";
    let verdict = route_verdict(fixture, "start");
    let RouteVerdict::Rejected { unproved, .. } = &verdict else {
        panic!("{fixture}: {verdict:?}");
    };
    let messages: Vec<String> = unproved.iter().map(|entry| entry.message()).collect();
    assert!(
        messages.iter().any(|message| message
            .contains("cannot prove requires contract for call scan from Main::main: 40 == 41")),
        "{messages:#?}"
    );
}
