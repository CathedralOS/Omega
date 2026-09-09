use super::*;
use terminal_interpreter::{TerminalArtifactInterpretError, TerminalStructuralScalarFieldValue};

fn restrict(module: &mut TerminalModule, nested: bool) {
    let integer = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let field = StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "value".into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::BoundedInteger(
            semantic_vocabulary::BoundedIntegerType::new(
                integer,
                IntegerValue::Signed(0),
                IntegerValue::Signed(255),
            )
            .unwrap(),
        ),
    };
    let shape = StructuralTypeShape::Record {
        fields: vec![field],
    };
    if nested {
        module.structural_types.push(StructuralTypeDeclaration {
            id: structural_type_id(2),
            identity: "Nested".into(),
            shape,
        });
        module.structural_types[0].shape = StructuralTypeShape::FixedArray {
            element: structural_type_id(2),
            length: 2,
        };
    } else {
        module.structural_types[0].shape = shape;
    }
}

#[test]
fn bounded_integer_host_result_rejects_before_handler_or_custody() {
    for nested in [false, true] {
        let mut module = structural_boundary_effect_module();
        restrict(&mut module, nested);
        let semantic = encode_module(&module).unwrap();
        let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
        let mut execution =
            TerminalExecution::start_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
                .unwrap();
        let mut handler = BoundaryResultHandler {
            result: Ok(TerminalEffectResult::Structural(TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: structural_type_id(1),
                qualifications: Vec::new(),
                path: Vec::new(),
            })),
            requests: 0,
            effects: Vec::new(),
        };
        assert!(matches!(
            execution.resume_with_effect_handler(&mut TerminalFuelMeter::unbounded(), &mut handler),
            Err(TerminalInterpretError::VerifiedOperationMalformed)
        ));
        assert_eq!(handler.requests, 0);
        assert!(execution.effects().is_empty());
    }
}

#[test]
fn bounded_integer_opaque_entry_rejects_even_without_a_field_read() {
    for nested in [false, true] {
        let mut module = boundary_borrows::borrowed_boundary_module(StructuralAccess::SharedBorrow);
        restrict(&mut module, nested);
        let semantic = encode_module(&module).unwrap();
        let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
        assert!(
            TerminalExecution::start_artifact_with_structural_arguments(
                &semantic,
                &proof,
                &AdmissionProfile::default(),
                &[],
                &[TerminalStructuralValue {
                    opaque_identity: 71,
                    structural_type: structural_type_id(1),
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
            )
            .is_err()
        );
    }
}

#[test]
fn explicit_bounded_entry_fields_validate_all_contents_before_startup() {
    for nested in [false, true] {
        let mut module = boundary_borrows::borrowed_boundary_module(StructuralAccess::SharedBorrow);
        restrict(&mut module, nested);
        let semantic = encode_module(&module).unwrap();
        let proof = encode_proof_bundle(&ProofBundle::default()).unwrap();
        let roots = [TerminalStructuralValue {
            opaque_identity: 71,
            structural_type: structural_type_id(1),
            qualifications: Vec::new(),
            path: Vec::new(),
        }];
        let fields = (0..if nested { 2 } else { 1 })
            .map(|position| TerminalStructuralScalarFieldValue {
                argument_index: 0,
                path: if nested {
                    vec![StructuralPathSegment::FixedIndex(position)]
                } else {
                    Vec::new()
                },
                field: StructuralFieldId::new(1).unwrap(),
                value: TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    value: IntegerValue::Signed(if position == 0 { 0 } else { 255 }),
                },
            })
            .collect::<Vec<_>>();
        for mutation in [
            "none",
            "missing",
            "below",
            "above",
            "width",
            "sign",
            "duplicate",
        ] {
            let mut supplied = fields.clone();
            match mutation {
                "none" => {}
                "missing" => {
                    supplied.pop();
                }
                "below" | "above" => {
                    let TerminalScalarValue::Integer { value, .. } = &mut supplied[0].value else {
                        panic!("integer field")
                    };
                    *value = IntegerValue::Signed(if mutation == "below" { -1 } else { 256 });
                }
                "width" => {
                    supplied[0].value = TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Signed, 64).unwrap(),
                        value: IntegerValue::Signed(0),
                    }
                }
                "sign" => {
                    supplied[0].value = TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                        value: IntegerValue::Unsigned(0),
                    }
                }
                "duplicate" => supplied.push(supplied[0].clone()),
                _ => unreachable!(),
            }
            let result =
                TerminalExecution::start_artifact_with_structural_arguments_and_scalar_fields(
                    &semantic,
                    &proof,
                    &AdmissionProfile::default(),
                    &[],
                    &roots,
                    &supplied,
                );
            if mutation == "none" {
                assert!(
                    result.is_ok(),
                    "nested={nested}: {:?}",
                    result.as_ref().err()
                );
            } else {
                assert!(
                    matches!(
                        &result,
                        Err(TerminalArtifactInterpretError::Execution(
                            TerminalInterpretError::StructuralScalarFieldArgumentInvalid { .. }
                        ))
                    ),
                    "nested={nested}, mutation={mutation}: {:?}",
                    result.as_ref().err()
                );
            }
        }
    }
}
