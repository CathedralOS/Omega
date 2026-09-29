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

/// Receiver-field reads feed the same selected IEEE comparison as parameters.
/// The serialized operation must retain the declared field's exact format;
/// successful source admission alone does not establish that agreement.
#[test]
fn selected_float_guards_replay_receiver_field_formats() {
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
    let lowered = lower_machine(
        &crate::front_end::checked_program(&comparison("self.f != self.g")),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("receiver-field observations compose with the selected comparison");
    assert_eq!(lowered.selected_ieee_float_comparison_occurrences.len(), 1);
    let mut module = terminal_codec::decode_module(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
    )
    .unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &lowered.proof_bundle, &profile).unwrap();
    let reads = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::PrimitiveScalarRead { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(reads.len(), 2, "both operands remain storage observations");
    for read in reads {
        assert_eq!(
            read.result.scalar_ref().unwrap().scalar_type,
            semantic_vocabulary::ScalarType::IeeeFloat(
                semantic_vocabulary::IeeeFloatFormat::Binary64,
            ),
        );
    }
    let read = module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::PrimitiveScalarRead { .. }
            )
        })
        .unwrap();
    let terminal_psi::OperationResult::Scalar(result) = &mut read.result else {
        panic!("the field read has a scalar result");
    };
    result.scalar_type =
        semantic_vocabulary::ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32);
    assert!(
        terminal_verifier::verify_module(&module, &lowered.proof_bundle, &profile).is_err(),
        "an f64 field cannot be observed as f32 by altering the result declaration",
    );
}
