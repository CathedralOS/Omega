use super::*;
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralFieldId};

#[test]
fn outcome_specific_guarantee_checks_its_structural_field_metadata() {
    let (mut module, success, _, _) = payloadless_guard_module();
    let root = PlaceId::new(930).unwrap();
    let structural_type = StructuralTypeId::new(930).unwrap();
    let field_id = StructuralFieldId::new(1).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "Input".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                id: field_id,
                identity: "value".into(),
                field_type: StructuralFieldType::Scalar(ScalarType::Integer(integer_type)),
                relevance: terminal_psi::BindingRelevance::Relevant,
            }],
        },
    });
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: root,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: root,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    let field = ScalarTerm::integer_field_path(
        root,
        vec![CanonicalStructuralPathSegment::Field(field_id)],
        integer_type,
    );
    machine
        .contract
        .outcome_specific_ensures
        .push(OutcomeSpecificEnsure {
            guard: OutcomeSpecificGuard {
                result_type: StructuralTypeId::new(920).unwrap(),
                result_case: success,
            },
            position: 0,
            obligation: ObligationId::new(930).unwrap(),
            proposition: Proposition::Equal(field.clone(), field),
            evidence: None,
        });
    validate_module(&module).expect("valid guarded guarantee on a readable integer field");
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
        let mut invalid = module.clone();
        invalid.machines[0].structural_parameters[0].access = access;
        validate_module(&invalid).expect("guarded contract formation does not read the field");
        let StructuralTypeShape::Record { fields } = &mut invalid.structural_types[1].shape else {
            panic!("input record");
        };
        fields[0].id = StructuralFieldId::new(2).unwrap();
        assert!(matches!(
            validate_module(&invalid),
            Err(ModuleError::InvalidIntegerFieldTerm { .. })
        ));
    }
}
