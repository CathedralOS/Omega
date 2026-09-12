use super::*;
use semantic_vocabulary::{OperationId, StructuralTypeId};
use terminal_psi::{StructuralMultiplicity, StructuralOperationResult, ValueDeclaration};

fn operation() -> Operation {
    Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Boolean,
            qualifications: Default::default(),
        }),
        kind: OperationKind::StructuralCaseMembership {
            source: PlaceId::new(3).unwrap(),
            case: StructuralCaseId::new(4).unwrap(),
        },
    }
}

#[test]
fn case_membership_retains_exact_observation_without_current_storage_equation() {
    let mut operation = operation();
    let row = structural_effect_semantic_row(&operation.kind)
        .unwrap()
        .unwrap();
    assert_eq!(row.tag(), OperationSemanticTag::StructuralCaseMembership);
    let schema = row.schema();
    assert_eq!(schema.result(), StructuralEffectResultShape::Boolean);
    assert_eq!(
        schema.custody(),
        StructuralEffectCustody::ExactReadableSumRoot
    );
    assert_eq!(
        schema.action(),
        StructuralEffectAction::ObserveCaseMembership
    );
    assert_eq!(
        schema.external_effect(),
        StructuralEffectExternalEffect::None
    );
    assert_eq!(schema.fuel(), StructuralEffectFuelPolicy::ConsumeOne);
    assert_eq!(schema.goal(), StructuralEffectGoalShape::None);
    assert_eq!(
        schema.frontier(),
        StructuralEffectFrontierPolicy::RequiresAndKeepsStructuralPlace
    );
    assert!(!crate::is_unconditionally_total_scalar(&operation.kind));

    for (source, case, result) in [(3, 4, 2), (5, 4, 2), (3, 6, 2), (3, 4, 7)] {
        let source = PlaceId::new(source).unwrap();
        let case = StructuralCaseId::new(case).unwrap();
        let result = ValueId::new(result).unwrap();
        operation.kind = OperationKind::StructuralCaseMembership { source, case };
        operation.result.scalar_mut().unwrap().id = result;
        let observation = structural_effect_leaf_observation(&operation)
            .unwrap()
            .unwrap();
        assert_eq!(
            observation,
            StructuralEffectObservation::CaseMembershipObserved {
                source,
                case,
                result
            }
        );
        assert_eq!(observation.local_equation(), None);
        assert_eq!(observation.canonical_obligation(), None);
    }
}

#[test]
fn case_membership_rejects_non_boolean_results() {
    let mut operation = operation();
    for result in [
        OperationResult::Unit,
        OperationResult::Scalar(ValueDeclaration {
            id: ValueId::new(2).unwrap(),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap()),
            qualifications: Default::default(),
        }),
        OperationResult::Structural(StructuralOperationResult {
            place: PlaceId::new(5).unwrap(),
            structural_type: StructuralTypeId::new(6).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
    ] {
        operation.result = result;
        assert_eq!(
            structural_effect_leaf_observation(&operation),
            Err(OperationSemanticError::StructuralEffectResultShapeMismatch(
                OperationSemanticTag::StructuralCaseMembership
            ))
        );
    }
}

#[test]
fn case_membership_rejects_schema_axis_drift() {
    let operation = operation();
    for axis in 0..6 {
        let mut rows = StructuralEffectSemanticRow::ALL;
        let schema = &mut rows
            .iter_mut()
            .find(|row| row.tag == OperationSemanticTag::StructuralCaseMembership)
            .unwrap()
            .schema;
        match axis {
            0 => schema.result = StructuralEffectResultShape::Scalar,
            1 => schema.custody = StructuralEffectCustody::ExactReadablePrimitiveRoot,
            2 => schema.action = StructuralEffectAction::ReadPrimitive,
            3 => schema.external_effect = StructuralEffectExternalEffect::PortWrite,
            4 => schema.frontier = StructuralEffectFrontierPolicy::KeepsPlaceFrontier,
            _ => schema.goal = StructuralEffectGoalShape::ByteIndexInBounds,
        }
        assert_eq!(
            structural_effect_leaf_observation_in(&operation, &rows),
            Err(OperationSemanticError::StructuralEffectSchemaMismatch(
                OperationSemanticTag::StructuralCaseMembership
            ))
        );
    }
}

#[test]
fn case_membership_requires_exactly_one_semantic_row() {
    let operation = operation();
    let tag = OperationSemanticTag::StructuralCaseMembership;
    let row = *structural_effect_semantic_row(&operation.kind)
        .unwrap()
        .unwrap();
    let mut rows = StructuralEffectSemanticRow::ALL.to_vec();
    rows.retain(|candidate| candidate.tag != tag);
    assert_eq!(
        structural_effect_leaf_observation_in(&operation, &rows),
        Err(OperationSemanticError::MissingStructuralEffectRow(tag))
    );
    rows.extend([row, row]);
    assert_eq!(
        structural_effect_leaf_observation_in(&operation, &rows),
        Err(OperationSemanticError::DuplicateStructuralEffectRow(tag))
    );
}
