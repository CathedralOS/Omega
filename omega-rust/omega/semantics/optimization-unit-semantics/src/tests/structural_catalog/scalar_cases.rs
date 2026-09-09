//! Constructor payload identity, scalar use and declaration corruption controls.

use super::super::*;

fn scalar_case_unit() -> PsiOptimizationUnit {
    let mut candidate = unit();
    let structural_type = id(801, StructuralTypeId::new);
    let case = id(802, semantic_vocabulary::StructuralCaseId::new);
    let field = id(803, semantic_vocabulary::StructuralFieldId::new);
    let place = id(804, PlaceId::new);
    let operation = id(805, OperationId::new);
    let scalar_type = candidate.functions[0].blocks[0].nodes[0].definitions[0].scalar_type;
    candidate.structural_types = vec![terminal_psi::StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::ScalarOutcome".into(),
        shape: terminal_psi::StructuralTypeShape::Sum { cases: vec![terminal_psi::StructuralCaseDeclaration {
            id: case, identity: "Value".into(), fields: vec![terminal_psi::StructuralFieldDeclaration {
                id: field, identity: "value".into(), relevance: terminal_psi::BindingRelevance::Relevant,
                field_type: terminal_psi::StructuralFieldType::Scalar(scalar_type),
            }],
        }] },
    }];
    candidate.functions[0].structural_places.push(terminal_psi::StructuralPlaceDeclaration {
        id: place, kind: StructuralPlaceKind::OperationResult { producer: operation, structural_type },
    });
    let mut node = candidate.functions[0].blocks[0].nodes[0].clone();
    node.operation = AbstractOperation::EstablishScalarCase {
        psi_operation: operation,
        result: terminal_psi::StructuralOperationResult { place, structural_type,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(), projected_qualifications: Vec::new(), claims: Vec::new() },
        result_case: case,
        fields: vec![terminal_psi::ScalarCaseField { field, value: id(3, ValueId::new), range_obligation: None }],
    };
    candidate.functions[0].blocks[0].nodes.insert(1, node);
    refresh_node_derivatives(&mut candidate, 0, 0, 1);
    refresh_node_derivatives(&mut candidate, 0, 0, 2);
    refresh_identity(&mut candidate);
    candidate
}

#[test]
fn scalar_case_initializer_is_a_live_typed_scalar_use() {
    let candidate = scalar_case_unit();
    let function = &candidate.functions[0];
    let node = &function.blocks[0].nodes[1];
    assert_eq!(node.uses.len(), 1);
    assert_eq!(node.uses[0].value, id(3, ValueId::new));
    let types = candidate.structural_types.iter().map(|declaration| (declaration.id, declaration)).collect();
    assert!(crate::unit_validation::scalar_case_establishment_matches(function, &node.operation, &types));
    let original = candidate.identity;
    for mutation in 0..5 {
        let mut changed = candidate.clone();
        let AbstractOperation::EstablishScalarCase { fields, result_case, result, .. } = &mut changed.functions[0].blocks[0].nodes[1].operation else { unreachable!() };
        match mutation {
            0 => fields.clear(),
            1 => fields[0].field = id(899, semantic_vocabulary::StructuralFieldId::new),
            2 => fields[0].value = id(898, ValueId::new),
            3 => *result_case = id(897, semantic_vocabulary::StructuralCaseId::new),
            4 => result.multiplicity = terminal_psi::StructuralMultiplicity::Linear,
            _ => unreachable!(),
        }
        refresh_identity(&mut changed);
        assert_ne!(original, changed.identity);
        assert!(!crate::unit_validation::scalar_case_establishment_matches(&changed.functions[0], &changed.functions[0].blocks[0].nodes[1].operation, &types));
    }
}
