use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralTypeId};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalArtifactInterpretError, TerminalEffectHandler, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError, TerminalScalarValue,
    TerminalStructuralScalarFieldValue, TerminalStructuralValue,
};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment, StructuralTypeShape,
    TerminalModule,
};

fn record_fields(
    module: &TerminalModule,
    identity: StructuralTypeId,
) -> &[StructuralFieldDeclaration] {
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == identity)
        .expect("fixture record type");
    let StructuralTypeShape::Record { fields } = &declaration.shape else {
        panic!("fixture contains records")
    };
    fields
}

fn child_type(module: &TerminalModule, parent: StructuralTypeId, name: &str) -> StructuralTypeId {
    let field = record_fields(module, parent)
        .iter()
        .find(|field| field.identity == name)
        .expect("fixture structural field");
    let StructuralFieldType::Structural(child) = field.field_type else {
        panic!("fixture child is structural")
    };
    child
}

pub(super) fn execute(
    module: &TerminalModule,
    semantics: &[u8],
    proof: &[u8],
    argument: TerminalStructuralValue,
    values: [(&str, u128); 3],
    handler: &mut impl TerminalEffectHandler,
) -> u64 {
    let scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let mut fields = Vec::new();
    // Supply every Metrics instance, including both untouched sibling subtrees.
    for batch in ["batch", "spare"] {
        let batch_type = child_type(module, argument.structural_type, batch);
        for metrics in ["metrics", "shadow"] {
            let metrics_type = child_type(module, batch_type, metrics);
            for (name, value) in values {
                let field = record_fields(module, metrics_type)
                    .iter()
                    .find(|field| field.identity == name)
                    .expect("explicit fixture integer field");
                fields.push(TerminalStructuralScalarFieldValue {
                    argument_index: 0,
                    path: [batch, metrics]
                        .map(|name| StructuralPathSegment::Field(name.into()))
                        .to_vec(),
                    field: field.id,
                    value: TerminalScalarValue::Integer {
                        scalar_type,
                        value: IntegerValue::Unsigned(value),
                    },
                });
            }
        }
    }
    let profile = AdmissionProfile::default();
    let start = |fields: &[TerminalStructuralScalarFieldValue]| {
        TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(
            semantics,
            proof,
            &profile,
            &[],
            std::slice::from_ref(&argument),
            fields,
        )
    };
    // Entry validation must also reject invalid contents in the unread spare.shadow.
    let unread_current = fields
        .iter()
        .position(|field| {
            field.field == fields[0].field
                && field.path
                    == ["spare", "shadow"].map(|name| StructuralPathSegment::Field(name.into()))
        })
        .expect("explicit unread current field");
    let rejected_field = fields[unread_current].field;
    let mut missing = fields.clone();
    missing.remove(unread_current);
    let mut outside_range = fields.clone();
    outside_range[unread_current].value = TerminalScalarValue::Integer {
        scalar_type,
        value: IntegerValue::Unsigned(201),
    };
    for invalid in [&missing, &outside_range] {
        assert!(matches!(
            start(invalid),
            Err(TerminalArtifactInterpretError::Execution(
                TerminalInterpretError::StructuralScalarFieldArgumentInvalid {
                    argument_index: 0,
                    field,
                }
            )) if field == rejected_field
        ));
    }
    let mut execution = start(&fields).expect("bind every explicit bounded fixture field");
    let mut meter = TerminalFuelMeter::unbounded();
    assert_eq!(
        execution
            .resume_with_effect_handler(&mut meter, handler)
            .expect("member arithmetic remains verified metadata at interpretation"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    meter.usage().total_units()
}
