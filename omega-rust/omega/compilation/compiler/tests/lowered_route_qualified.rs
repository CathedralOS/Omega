//! The checker on the lowered form judges a machine declared under a data
//! type's name, `machine Counter::clear(counter: &mut Counter)`, and the calls
//! that name it, instead of handing every program that has one back to the
//! current checker.
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
#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::{
    FixtureFrontierCheck, check_fixture_keeping_typed_frontier, repo_root,
};
use lowered_discharge::{RouteVerdict, judge};
use structure_elaboration::{Compilation, elaborate};

// Fixture compilation shares process-wide counters; run one at a time.
static FIXTURE_COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The route's verdict on everything the check of `fixture` claims, and
/// whether the current checker accepted the program.
fn route_verdict(fixture: &str) -> (RouteVerdict, bool) {
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
    let (frontier, accepted) = match checked {
        FixtureFrontierCheck::Accepted { frontier, .. } => (frontier, true),
        FixtureFrontierCheck::Rejected { frontier, .. } => (frontier, false),
        _ => panic!("{fixture}: the compilation must reach the current checker"),
    };
    let verdict = judge(elaborate(
        &frontier.typed,
        Compilation {
            entries: &frontier.entries,
            subject: frontier.subject,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
    ))
    .unwrap_or_else(|defect| panic!("{fixture}: the route must reach a verdict: {defect:?}"));
    (verdict, accepted)
}

/// Programs both checkers accept, by the call form that names the qualified
/// machine.
const ACCEPTED: &[&str] = &[
    // An expression call and a transition target, `Main::countdown(..)`,
    // the second from inside the machine itself.
    "tests/omega/pass/calls/machine_self_call_recursion_compile",
    // Call statements and expression calls of five machines under one name,
    // two of them lending a scalar local.
    "tests/omega/run/calls/qualified_atomic_operation_names",
    // The qualified machine owes an `ensures` it meets through its callee's.
    "tests/omega/pass/calls/attached_terminal_machine_call_exit",
    // The qualified machine declares a nested machine and calls a free one
    // of the same name.
    "tests/omega/pass/calls/state_and_machine_names_exit",
    // The qualified machine is the bound entry and ends in a call of a
    // private helper that returns nothing, whose input conditions are
    // inferred from this one arrival.
    "tests/omega/pass/calls/attached_context_terminal_machine_call_exit",
];

#[test]
fn a_program_with_data_qualified_machines_is_judged_and_accepted() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let mut wrong = Vec::new();
    for fixture in ACCEPTED {
        let (verdict, accepted) = route_verdict(fixture);
        assert!(accepted, "{fixture}: the current checker accepts it");
        if !matches!(verdict, RouteVerdict::Accepted(_)) {
            wrong.push(format!("{fixture}: {verdict:?}"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The twin of `attached_terminal_machine_call_exit`: `Main::main` passes 40
/// to a callee that requires 41. The route rejects it at that call, as the
/// current checker does, so the qualified machine's body is really judged.
#[test]
fn a_data_qualified_machine_that_breaks_its_callees_requires_is_rejected() {
    let _compilation = FIXTURE_COMPILATIONS.lock().unwrap();
    let fixture = "tests/omega/fail/calls/free_machine_named_transition_rejected";
    let (verdict, accepted) = route_verdict(fixture);
    assert!(!accepted, "{fixture}: the current checker rejects it");
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
