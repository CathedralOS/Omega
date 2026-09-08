use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralFieldId};
use terminal_fuel::{FuelChargeSite, TerminalFuelMeter};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalExecution, TerminalExecutionResult,
    TerminalExecutionStatus, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralBooleanFieldValue, TerminalStructuralScalarFieldValue,
    TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralPathSegment, TerminalModule};

use super::{LIMITS, support};

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn inputs(
    module: &TerminalModule,
    limit: u128,
    divisor: u128,
) -> (
    TerminalStructuralValue,
    [TerminalStructuralScalarFieldValue; 2],
) {
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &root.structural_parameters[0];
    (
        TerminalStructuralValue {
            opaque_identity: 71,
            structural_type: parameter.structural_type,
            qualifications: Vec::new(),
            path: Vec::new(),
        },
        [("limit", limit), ("divisor", divisor)].map(|(identity, value)| {
            TerminalStructuralScalarFieldValue {
                argument_index: 0,
                path: Vec::new(),
                field: support::field(module, parameter.structural_type, identity),
                value: unsigned(value),
            }
        }),
    )
}

#[test]
fn affine_limits_integer_fields_execute_directly_and_through_owned_forwarding() {
    for entry in ["inspect", "enter"] {
        let (_, module, semantic_bytes, proof_bytes) = support::publish(LIMITS, entry);
        let profile = proof_admission::AdmissionProfile::default();
        for (limit, divisor) in [(0, 3), (42, 4), (99, 5), (u128::from(u64::MAX), 3)] {
            let (root, fields) = inputs(&module, limit, divisor);
            let mut execution =
                TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(
                    &semantic_bytes,
                    &proof_bytes,
                    &profile,
                    &[unsigned(201)],
                    &[root],
                    &fields,
                )
                .expect("bind explicit integer Limits fields");
            let mut meter = TerminalFuelMeter::with_allowance(0);
            let mut complete = false;
            for _ in 0..128 {
                match execution
                    .resume(&mut meter)
                    .expect("resume real owned scalar graph")
                {
                    TerminalExecutionStatus::Complete(result) => {
                        assert_eq!(
                            result,
                            TerminalExecutionResult::Scalar(unsigned(limit)),
                            "{entry}, divisor {divisor}"
                        );
                        complete = true;
                        break;
                    }
                    TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
                    other => panic!("unexpected Limits outcome: {other:?}"),
                }
            }
            assert!(complete);
            assert!(
                execution.live_affine_frontier().next().is_none(),
                "callee disposes the owned Limits"
            );
            let mut reads = 0;
            let mut stores = 0;
            let mut calls = 0;
            for operation in module
                .machines
                .iter()
                .flat_map(|machine| &machine.blocks)
                .flat_map(|block| &block.operations)
            {
                let usage = meter
                    .usage()
                    .at(FuelChargeSite::Operation(operation.id))
                    .expect("authored operation executes rather than folding the helper");
                assert_eq!(usage.executions(), 1, "no replay across one-unit pauses");
                match operation.kind {
                    OperationKind::IntegerStructuralField { .. } => reads += 1,
                    OperationKind::WriteOnlyPrimitiveStore { .. } => stores += 1,
                    OperationKind::CallStructuralScalar { .. } => calls += 1,
                    _ => {}
                }
            }
            assert_eq!(reads, 1);
            assert_eq!(stores, 1, "reset mutates its caller's primitive local");
            assert_eq!(calls, if entry == "inspect" { 1 } else { 2 });
        }
    }
}

#[test]
fn canonical_limits_entry_rejects_invalid_integer_field_inputs() {
    let (_, module, semantic_bytes, proof_bytes) = support::publish(LIMITS, "inspect");
    let profile = proof_admission::AdmissionProfile::default();
    let (root, original) = inputs(&module, 42, 4);
    for mutation in [
        "type",
        "width",
        "range",
        "path",
        "field",
        "position",
        "duplicate",
    ] {
        let mut fields = original.to_vec();
        match mutation {
            "type" => fields[0].value = TerminalScalarValue::Boolean(true),
            "width" => {
                fields[0].value = TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                    value: IntegerValue::Unsigned(42),
                }
            }
            "range" => fields[0].value = unsigned(u128::from(u64::MAX) + 1),
            "path" => fields[0]
                .path
                .push(StructuralPathSegment::Field("limit".into())),
            "field" => fields[0].field = StructuralFieldId::new(u64::MAX).unwrap(),
            "position" => fields[0].argument_index = 1,
            "duplicate" => fields.push(fields[0].clone()),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(
                    &semantic_bytes,
                    &proof_bytes,
                    &profile,
                    &[unsigned(201)],
                    std::slice::from_ref(&root),
                    &fields
                ),
                Err(TerminalArtifactInterpretError::Execution(
                    TerminalInterpretError::StructuralScalarFieldArgumentInvalid { .. }
                ))
            ),
            "{mutation}"
        );
    }
    let boolean = TerminalStructuralBooleanFieldValue {
        argument_index: 0,
        path: Vec::new(),
        field: original[0].field,
        value: true,
    };
    assert!(
        matches!(
            TerminalExecution::start_artifact_with_structural_arguments_and_boolean_fields(
                &semantic_bytes,
                &proof_bytes,
                &profile,
                &[unsigned(201)],
                &[root],
                &[boolean]
            ),
            Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralBooleanFieldArgumentInvalid { .. }
            ))
        ),
        "Boolean adapter preserves its checked field-type error"
    );
}

#[test]
fn omitted_limit_never_acquires_default_contents_at_entry_or_after_forwarding() {
    for entry in ["inspect", "enter"] {
        let (_, module, semantic_bytes, proof_bytes) = support::publish(LIMITS, entry);
        let (root, fields) = inputs(&module, 42, 4);
        let start = TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(
            &semantic_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[unsigned(201)],
            &[root],
            &fields[1..],
        );
        let mut execution = start.expect("integer fields need contents only when actually read");
        assert!(
            matches!(execution.resume(&mut TerminalFuelMeter::unbounded()), Err(TerminalInterpretError::StructuralScalarFieldMissing { field, .. }) if field == fields[0].field),
            "{entry} must reject absent contents at its executed read"
        );
    }
}
