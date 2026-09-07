use super::*;
use semantic_vocabulary::{OperationId, StructuralTypeId};
use terminal_psi::{StructuralMultiplicity, StructuralOperationResult};

fn operation() -> Operation {
    Operation {
        id: OperationId::new(1).unwrap(),
        result: OperationResult::Structural(StructuralOperationResult {
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
fn subslice_observation_retains_exact_operands_and_both_ordered_bounds() {
    let operation = operation();
    let observation = structural_effect_leaf_observation(&operation)
        .unwrap()
        .unwrap();
    assert_eq!(
        observation,
        StructuralEffectObservation::ByteSequenceSubslice {
            source: PlaceId::new(1).unwrap(),
            start: ValueId::new(3).unwrap(),
            end: ValueId::new(4).unwrap(),
            length: ValueId::new(5).unwrap(),
            destination: PlaceId::new(2).unwrap(),
            obligation: ObligationId::new(6).unwrap(),
        }
    );
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let (obligation, goal) = observation.canonical_obligation().unwrap();
    assert_eq!(obligation, ObligationId::new(6).unwrap());
    assert_eq!(
        goal,
        Proposition::Conjunction(vec![
            Proposition::LessOrEqual(value(3), value(4)),
            Proposition::LessOrEqual(value(4), value(5)),
        ])
    );
    goal.validate().unwrap();
    assert_eq!(observation.local_equation(), None);
    assert!(!crate::is_unconditionally_total_scalar(&operation.kind));
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
