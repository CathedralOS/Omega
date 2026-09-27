use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralFieldId};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalExecution, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralBooleanFieldValue, TerminalStructuralScalarFieldValue,
    TerminalStructuralValue,
};
use terminal_psi::{StructuralPathSegment, TerminalModule};

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
                TerminalExecution::start_artifact(
                    &semantic_bytes,
                    &proof_bytes,
                    &profile,
                    &[unsigned(201)],
                    TerminalStructuralInputs {
                        arguments: std::slice::from_ref(&root),
                        scalar_fields: &fields,
                        ..Default::default()
                    }
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
            TerminalExecution::start_artifact(
                &semantic_bytes,
                &proof_bytes,
                &profile,
                &[unsigned(201)],
                TerminalStructuralInputs {
                    arguments: &[root],
                    boolean_fields: &[boolean],
                    ..Default::default()
                }
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
        let start = TerminalExecution::start_artifact(
            &semantic_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[unsigned(201)],
            TerminalStructuralInputs {
                arguments: &[root],
                scalar_fields: &fields[1..],
                ..Default::default()
            },
        );
        let mut execution = start.expect("integer fields need contents only when actually read");
        assert!(
            matches!(execution.resume(&mut TerminalFuelMeter::unbounded(), &mut AcceptTerminalEffects), Err(TerminalInterpretError::StructuralScalarFieldMissing { field, .. }) if field == fields[0].field),
            "{entry} must reject absent contents at its executed read"
        );
    }
}
