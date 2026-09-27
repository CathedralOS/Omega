use super::{
    ObligationId, Operation, OperationKind, OperationResult, OperationSemanticTag, PlaceId,
    Proposition, StructuralEffectCustody, StructuralEffectExternalEffect,
    StructuralEffectFrontierPolicy, StructuralEffectFuelPolicy, StructuralEffectGoalShape,
    StructuralEffectResultShape, StructuralEffectSemanticRow, ValueId,
    structural_effect_leaf_observation, structural_effect_leaf_observation_in,
    structural_effect_semantic_row,
};
use semantic_vocabulary::{OperationId, StructuralTypeId};
use terminal_psi::{StructuralMultiplicity, StructuralOperationResult};

fn operation() -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Structural(StructuralOperationResult {
            qualification_establishments: Vec::new(),
            place: PlaceId::new(2).unwrap(),
            structural_type: StructuralTypeId::new(1).unwrap(),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::ByteSequenceSubslice {
            source: PlaceId::new(1).unwrap(),
            start: ValueId::new(3).unwrap(),
            end: ValueId::new(4).unwrap(),
            length: ValueId::new(5).unwrap(),
            obligation: ObligationId::new(6).unwrap(),
        },
    }
}

#[test]
fn empty_at_end_keeps_two_canonical_legs_instead_of_a_guessed_true_goal() {
    let mut operation = operation();
    let OperationKind::ByteSequenceSubslice {
        start, end, length, ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    *start = *length;
    *end = *length;
    let (_, goal) = structural_effect_leaf_observation(&operation)
        .unwrap()
        .unwrap()
        .canonical_obligation()
        .unwrap();
    let Proposition::Conjunction(legs) = &goal else {
        panic!("two ordered bounds")
    };
    assert_eq!(legs.len(), 2);
    assert_eq!(legs[0], legs[1]);
    assert!(matches!(&legs[0], Proposition::LessOrEqual(left, right) if left == right));
    goal.validate().unwrap();
}

#[test]
fn subslice_schema_rejects_goal_custody_result_and_frontier_drift() {
    let operation = operation();
    let schema = structural_effect_semantic_row(&operation.kind)
        .unwrap()
        .unwrap()
        .schema();
    assert_eq!(schema.goal(), StructuralEffectGoalShape::ByteRangeInBounds);
    assert_eq!(schema.fuel(), StructuralEffectFuelPolicy::ConsumeOne);
    assert_eq!(
        schema.external_effect(),
        StructuralEffectExternalEffect::None
    );
    for mutation in 0..4 {
        let mut rows = StructuralEffectSemanticRow::ALL;
        let row = rows
            .iter_mut()
            .find(|row| row.tag == OperationSemanticTag::ByteSequenceSubslice)
            .unwrap();
        match mutation {
            0 => row.schema.goal = StructuralEffectGoalShape::None,
            1 => row.schema.custody = StructuralEffectCustody::ExactByteSequenceLiteral,
            2 => row.schema.result = StructuralEffectResultShape::Unit,
            _ => row.schema.frontier = StructuralEffectFrontierPolicy::AddsOwnedPlace,
        }
        assert!(
            structural_effect_leaf_observation_in(&operation, &rows).is_err(),
            "mutation {mutation}"
        );
    }
    let mut wrong_result = operation;
    wrong_result.result = OperationResult::Unit;
    assert!(structural_effect_leaf_observation(&wrong_result).is_err());
}
