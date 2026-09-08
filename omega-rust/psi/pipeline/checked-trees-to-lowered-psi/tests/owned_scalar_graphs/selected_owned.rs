use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralScalarFieldValue, TerminalStructuralValue,
};

use super::{LIMITS, support};

#[test]
fn skipped_and_taken_owned_calls_each_settle_their_selected_affine_frontier() {
    for body in [
        "let answer: bool = take && positive(limits); answer",
        // A value destination exercises selected computation calls. A bare
        // named destination is a state transfer, owned by cyclic/control lowering.
        "transition take { true -> (positive(limits) == true) false -> false }",
    ] {
        let source = format!(
            "{LIMITS}\nmachine positive(limits: Limits) -> bool {{ limits.limit > 0 }}\nmachine select(take: bool, limits: Limits) -> bool {{ {body} }}"
        );
        let (_, module, semantic, proof) = support::publish(&source, "select");
        let root = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let parameter = &root.structural_parameters[0];
        for take in [false, true] {
            for limit in [0, 42] {
                let fields = [("limit", limit), ("divisor", 3)].map(|(identity, value)| {
                    TerminalStructuralScalarFieldValue {
                        argument_index: 0,
                        path: Vec::new(),
                        field: support::field(&module, parameter.structural_type, identity),
                        value: TerminalScalarValue::Integer {
                            scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                            value: IntegerValue::Unsigned(value),
                        },
                    }
                });
                let mut execution =
                    TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(
                        &semantic,
                        &proof,
                        &proof_admission::AdmissionProfile::default(),
                        &[TerminalScalarValue::Boolean(take)],
                        &[TerminalStructuralValue {
                            opaque_identity: 71,
                            structural_type: parameter.structural_type,
                            qualifications: Vec::new(),
                            path: Vec::new(),
                        }],
                        &fields,
                    )
                    .unwrap();
                let mut meter = TerminalFuelMeter::with_allowance(0);
                let mut complete = false;
                for _ in 0..128 {
                    match execution.resume(&mut meter).unwrap() {
                        TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                        TerminalExecutionStatus::Complete(result) => {
                            assert_eq!(
                                result,
                                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(
                                    take && limit > 0
                                ))
                            );
                            assert!(execution.live_affine_frontier().next().is_none());
                            complete = true;
                            break;
                        }
                        other => panic!("unexpected outcome {other:?}"),
                    }
                }
                assert!(complete, "selected call finishes under one-unit fuel");
            }
        }
    }
}
