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

/// A selected IEEE comparison guard over parameters, locals or literals
/// already publishes one occurrence (`composed_unit_guarded_jumps`); the same
/// guard over receiver fields stops earlier, at scalar field observation,
/// which admits integer fields only. This is the lowering wall under the
/// `runtime_float_*_comparison` canaries: they reach native physical replay
/// once float field reads lower, not once another physical arm exists.
#[test]
fn selected_float_guards_over_receiver_fields_fail_at_field_observation() {
    let comparison = |guard: &str| {
        format!(
            r#"
            boundary operator != Float::not_equal(left: f64, right: f64) -> bool;
            boundary trait Host {{ machine exit(code: i32); }}
            data Root {{ f: f64; g: f64; }}
            machine Root::enter(&mut self, left: f64, right: f64) reaches Host {{
                transition {guard} {{ true -> good() _ -> bad() }}
                state good(&mut self) {{ Host::exit(70); }}
                state bad(&mut self) {{ Host::exit(71); }}
            }}
        "#
        )
    };
    let lowered = lower_machine(
        &crate::front_end::checked_program(&comparison("left != right")),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("a parameter guard lowers");
    assert_eq!(lowered.selected_ieee_float_comparison_occurrences.len(), 1);
    let error = lower_machine(
        &crate::front_end::checked_program(&comparison("self.f != self.g")),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect_err("a receiver-field float guard is not observed yet");
    assert_eq!(
        format!("{error:?}"),
        "Unsupported(\"runtime scalar field observation requires an integer field\")"
    );
}
