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
