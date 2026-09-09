use super::*;

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
