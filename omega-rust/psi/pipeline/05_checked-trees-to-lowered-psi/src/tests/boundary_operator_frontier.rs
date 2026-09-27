//! The boundary-operator coverage frontier, witnessed from source.
//!
//! `checked_boundary_operator_scope` publishes occurrences only for the
//! intrinsic comparison families; every other admitted boundary-operator
//! application either emits a `BoundaryCall` operation — covered by the
//! boundary-settlement lane, not the operator lane — or fails earlier, at
//! statement-sequence planning. These tests pin both ends of that frontier so
//! an upstream admission change arrives as a failing expectation here rather
//! than a silent coverage gap downstream (`TRANSLATION-VALIDATION`).

use crate::TerminalMachineSelection;
use crate::lower_machine;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

/// A `boundary machine` requirement with a checked-body provider is a spelled
/// operator use whose emitted operation is `BoundaryCall`. The physical-child
/// lane for it is the boundary-settlement roster, so the operator-occurrence
/// roster intentionally stays empty: the lowered module must carry the call,
/// its source-custody join, and the checked demand, but no D29 occurrence.
#[test]
fn spelled_boundary_machine_calls_ride_the_boundary_lane() {
    let source = r#"
        data CheckedMath {}
        boundary machine - CheckedMath::select_left(left: i32, right: i32) -> i32;
        data CheckedMathProvider {}
        machine CheckedMathProvider::select_left_impl(left: i32, right: i32) -> i32
            satisfies CheckedMath::select_left
        {
            transition { _ -> left }
        }
        machine run() {
            let result: i32 = (70 as i32) - 0;
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let [application] = checked.facts.operators.boundary_applications.as_slice() else {
        panic!("one checked boundary application")
    };
    let checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression { expression, .. } =
        application.site
    else {
        panic!("the spelled use names an expression site")
    };
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect("a boundary-machine call lowers");
    let operations = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    let calls = operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::BoundaryCall { .. }
            )
        })
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("the selected operator emits exactly one BoundaryCall: {operations:?}")
    };
    let [source_call] = lowered.source_call_occurrences.as_slice() else {
        panic!("the emitted call retains one source join")
    };
    assert_eq!(
        source_call.source_site,
        Some(checked_trees::NominalMachineUseSite::Expression(expression))
    );
    assert_eq!(source_call.terminal_operation, call.id);
    assert_eq!(source_call.source_target, application.requirement_symbol);

    // Publishing re-derives the scope: the demand survives beside an empty
    // operator-occurrence roster; the call itself is a boundary occurrence.
    let mut timings = TerminalProductionTimings::default();
    let produced = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("run"),
    )
    .produce(TerminalProductionCustody::artifact_only(&mut timings))
    .expect("a boundary-machine call publishes");
    let scope = produced.boundary_operator_scope();
    assert!(scope.occurrences().is_empty());
    assert_eq!(
        scope.applications(),
        checked.facts.operators.boundary_applications.as_slice()
    );
}

/// A named `boundary operator` call inside a local initializer stops at
/// statement-sequence planning: the call is not a flow call, so the Unit
/// closure has no admitted body. This is the upstream wall the
/// `derive_operator_physical_span` fall-through never sees — extend occurrence
/// replay only after this admission lands.
#[test]
fn named_boundary_operator_initializers_fail_at_flow_call_planning() {
    let source = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32
            requires value == value
            ensures result == value + 0 && value == value;
        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
            satisfies CheckedMath::offset_zero
            requires input == input
            ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }
        boundary trait Host {
            machine mark(value: u64) reaches Host;
        }
        machine Marker::mark(value: u64) satisfies Host::mark {}
        machine run() reaches Host {
            Host::mark(1);
            let result: i32 = CheckedMath::offset_zero(70);
            Host::mark(2);
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    assert_eq!(checked.facts.operators.boundary_applications.len(), 1);
    let error = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect_err("the operator call is not an admitted flow call");
    assert_eq!(
        error.to_string(),
        "InvalidUnitMachinePlan { machine: \"run\", reason: \"attached Unit closure is missing a checked transitive machine plan\", omission: Some(\"`run` has no admitted body (local construction stopped at statement sequence: call: flow call, statement 1)\") }"
    );
}

/// A spelled `boundary operator` use in a local initializer stops earlier
/// still: the operator call is not a pure initializer, so the scalar local
/// never reaches statement-sequence call planning. Same upstream wall,
/// different clause.
#[test]
fn spelled_boundary_operator_initializers_fail_at_pure_initializer_admission() {
    let source = r#"
        data CheckedMath {}
        boundary operator - CheckedMath::select_left(left: i32, right: i32) -> i32;
        data CheckedMathProvider {}
        machine CheckedMathProvider::select_left_impl(left: i32, right: i32) -> i32
            satisfies CheckedMath::select_left
        {
            transition { _ -> left }
        }
        boundary trait Host {
            machine mark(value: u64) reaches Host;
        }
        machine Marker::mark(value: u64) satisfies Host::mark {}
        machine run() reaches Host {
            Host::mark(1);
            let result: i32 = (70 as i32) - 0;
            Host::mark(2);
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    assert_eq!(checked.facts.operators.boundary_applications.len(), 1);
    let error = lower_machine(&checked, TerminalMachineSelection::Name("run"))
        .expect_err("the operator use is not a pure initializer");
    assert_eq!(
        error.to_string(),
        "InvalidUnitMachinePlan { machine: \"run\", reason: \"attached Unit closure is missing a checked transitive machine plan\", omission: Some(\"`run` has no admitted body (local construction stopped at statement sequence: local data: scalar local: pure initializer, statement 1)\") }"
    );
}
