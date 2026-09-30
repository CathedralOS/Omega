//! A receiverless callee whose later state calls a top-level
//! `boundary requirement` with a published service ceiling.
//!
//! The call site matches its exact target reach; the closure then retains the
//! admitted target by identity. A bodyless requirement's own checked-inferred
//! reach is the empty row while its published ceiling names its authored
//! services, so that inferred row cannot stand in for a call's reach.

use super::{CheckedTrees, lower_machine};
use crate::TerminalMachineSelection;
use terminal_psi::OperationKind;

fn source(result: &str, call: &str, completion: &str) -> String {
    format!(
        "boundary trait Waits {{ machine idle() reaches Waits; }}
         data TimedWait {{}}
         boundary requirement TimedWait::poll(timeout: i32){result}
         reaches
             Waits;
         machine TimedWait::wait(milliseconds: u32){result}
         reaches
             Waits
         {{
             transition milliseconds > 100 {{
                 true -> refused()
                 _ -> at_once()
             }}
             state refused(){result} {{ {completion} }}
             state at_once(){result} {{
                 {call}
             }}
         }}
         machine run(){result} reaches Waits {{
             {call_run}
         }}",
        call_run = if result.is_empty() {
            "TimedWait::wait(5);".to_owned()
        } else {
            "let ready: i32 = TimedWait::wait(5);\n             ready".to_owned()
        },
    )
}

fn scalar_source() -> String {
    source(
        " -> i32",
        "let ready: i32 = TimedWait::poll(1);\n                 ready",
        "0",
    )
}

fn unit_source() -> String {
    source("", "TimedWait::poll(1);", "")
}

fn requirement_calls(lowered: &lowered_psi::LoweredPsi) -> usize {
    lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::BoundaryCall { .. }))
        .count()
}

#[test]
fn later_state_requirement_calls_lower_with_scalar_and_unit_results() {
    for source in [scalar_source(), unit_source()] {
        let checked = crate::front_end::checked_program(&source);
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
            .expect("later-state requirement call lowers");
        assert_eq!(requirement_calls(&lowered), 1, "{source}");
    }
}

#[test]
fn later_state_requirement_call_keeps_its_exact_call_reach() {
    let checked = crate::front_end::checked_program(&scalar_source());
    let mut changed: CheckedTrees = checked.clone();
    let call = changed
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter_mut()
        .flat_map(|plan| &mut plan.states)
        .flat_map(|state| &mut state.operations)
        .find_map(|operation| operation.call_mut())
        .expect("later-state requirement call");
    assert_ne!(
        call.target.service_reach.transitive,
        language_semantics::ServiceReachRowTable::EMPTY_ROW,
        "the call carries the requirement's published reach"
    );
    call.target.service_reach = language_semantics::ServiceReachSummary {
        direct: language_semantics::ServiceReachRowTable::EMPTY_ROW,
        transitive: language_semantics::ServiceReachRowTable::EMPTY_ROW,
    };
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("run")).is_err(),
        "a call reach below the requirement's published ceiling is refused"
    );
}
