use super::{literal, unsigned_literal, value};
use crate::integer_rules::integer_affine::{
    IntegerAffineBoundConversionError, IntegerAffineWitness, IntegerAffineWitnessError,
    check_integer_affine_witness, integer_affine_wrapping_evidence, map_integer_affine_bound,
};
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::Proposition;
use semantic_vocabulary::PropositionContext;
use semantic_vocabulary::ScalarTerm;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::{IntegerSign, ValueId};

#[test]
fn wrapping_add_chain_maps_lower_bound_only_with_no_wrap_evidence() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let axioms = vec![Proposition::Equal(
        target.clone(),
        ScalarTerm::wrapping_integer_add(
            integer_type,
            root.clone(),
            unsigned_literal(integer_type, 1),
        )
        .unwrap(),
    )];
    let checked = check_integer_affine_witness(
        &context,
        &axioms,
        &IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            definition_axioms: vec![0],
            literal_axioms: vec![None],
        },
    )
    .expect("unsigned wrapping add traverses forward");

    // An upper bound on the operand is unconditional: the reduced sum can
    // only be lower than the exact sum.
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(root.clone(), unsigned_literal(integer_type, 9)),
        ),
        Ok(Proposition::LessOrEqual(
            target.clone(),
            unsigned_literal(integer_type, 10),
        )),
    );

    // A lower bound requires the checked `operand <= maximum - literal`
    // conjunct; the helper reports exactly that proposition.
    let lower = Proposition::LessOrEqual(unsigned_literal(integer_type, 2), root.clone());
    let evidence = Proposition::LessOrEqual(root.clone(), unsigned_literal(integer_type, 254));
    assert_eq!(
        integer_affine_wrapping_evidence(&checked, &lower),
        Ok(vec![evidence.clone()]),
    );
    assert_eq!(
        map_integer_affine_bound(&checked, &lower),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![lower.clone(), evidence.clone()]),
        ),
        Ok(Proposition::LessOrEqual(
            unsigned_literal(integer_type, 3),
            target.clone(),
        )),
    );
    // A wrong headroom, a duplicated conjunct, or a stray member all
    // reject: the evidence set must match the checked requirement exactly.
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                lower.clone(),
                Proposition::LessOrEqual(root.clone(), unsigned_literal(integer_type, 253)),
            ]),
        ),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![
                lower.clone(),
                evidence.clone(),
                Proposition::LessOrEqual(root.clone(), unsigned_literal(integer_type, 200)),
            ]),
        ),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
    );
    assert_eq!(
        map_integer_affine_bound(&checked, &Proposition::Conjunction(vec![lower, evidence]),),
        Ok(Proposition::LessOrEqual(
            unsigned_literal(integer_type, 3),
            target,
        )),
    );
}

#[test]
fn wrapping_add_backward_maps_strict_bounds_with_headroom_evidence() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let operand = value(1, integer_type);
    let defined = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let axioms = vec![Proposition::Equal(
        defined.clone(),
        ScalarTerm::wrapping_integer_add(
            integer_type,
            operand.clone(),
            unsigned_literal(integer_type, 1),
        )
        .unwrap(),
    )];
    // Traverse the same equation from the defined value toward the operand.
    let checked = check_integer_affine_witness(
        &context,
        &axioms,
        &IntegerAffineWitness {
            root: defined.clone(),
            target: operand.clone(),
            definition_axioms: vec![0],
            literal_axioms: vec![None],
        },
    )
    .expect("wrapping add traverses toward its operand");

    // `defined < 2` yields `operand < 1` only under the no-wrap headroom
    // `operand <= 65534`.
    let strict = Proposition::LessThan(defined.clone(), unsigned_literal(integer_type, 2));
    let headroom =
        Proposition::LessOrEqual(operand.clone(), unsigned_literal(integer_type, 65_534));
    assert_eq!(
        integer_affine_wrapping_evidence(&checked, &strict),
        Ok(vec![headroom.clone()]),
    );
    assert_eq!(
        map_integer_affine_bound(&checked, &strict),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![strict, headroom.clone()]),
        ),
        Ok(Proposition::LessThan(
            operand.clone(),
            unsigned_literal(integer_type, 1),
        )),
    );

    // The non-strict twin maps through the same evidence.
    let upper = Proposition::LessOrEqual(defined.clone(), unsigned_literal(integer_type, 2));
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::Conjunction(vec![upper, headroom.clone()]),
        ),
        Ok(Proposition::LessOrEqual(
            operand.clone(),
            unsigned_literal(integer_type, 1),
        )),
    );

    // A lower bound on the defined value is unconditional backward:
    // `operand = defined - 1 (mod 2^w)` stays at or above `bound - 1`.
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(unsigned_literal(integer_type, 1), defined.clone()),
        ),
        Ok(Proposition::LessOrEqual(
            unsigned_literal(integer_type, 0),
            operand.clone(),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessThan(unsigned_literal(integer_type, 1), defined),
        ),
        Ok(Proposition::LessThan(
            unsigned_literal(integer_type, 0),
            operand,
        )),
    );
}

#[test]
fn wrapping_add_backward_lands_a_sibling_literal_and_rejects_ambiguity() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let operand = value(1, integer_type);
    let defined = value(2, integer_type);
    let sibling = value(3, integer_type);
    let context = PropositionContext::from_value_types(
        (1..=4).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    // `sibling == 3` lands before `defined == operand + sibling`, so the
    // backward traversal resolves `operand` uniquely.
    let landing = Proposition::Equal(sibling.clone(), unsigned_literal(integer_type, 3));
    let definition = Proposition::Equal(
        defined.clone(),
        ScalarTerm::wrapping_integer_add(integer_type, operand.clone(), sibling.clone()).unwrap(),
    );
    let checked = check_integer_affine_witness(
        &context,
        &[landing.clone(), definition],
        &IntegerAffineWitness {
            root: defined.clone(),
            target: operand.clone(),
            definition_axioms: vec![1],
            literal_axioms: vec![Some(0)],
        },
    )
    .expect("landed sibling resolves the wrapping operand");
    let upper = Proposition::LessOrEqual(defined.clone(), unsigned_literal(integer_type, 10));
    let headroom = Proposition::LessOrEqual(operand.clone(), unsigned_literal(integer_type, 252));
    assert_eq!(
        integer_affine_wrapping_evidence(&checked, &upper),
        Ok(vec![headroom.clone()]),
    );
    assert_eq!(
        map_integer_affine_bound(&checked, &Proposition::Conjunction(vec![upper, headroom]),),
        Ok(Proposition::LessOrEqual(
            operand.clone(),
            unsigned_literal(integer_type, 7),
        )),
    );

    // With both addends landed the backward direction cannot tell which
    // one is the operand, so the definition rejects as ambiguous.
    let ambiguous = Proposition::Equal(
        defined.clone(),
        ScalarTerm::wrapping_integer_add(integer_type, operand.clone(), operand.clone()).unwrap(),
    );
    let landed_operand = Proposition::Equal(operand.clone(), unsigned_literal(integer_type, 4));
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &[landed_operand, ambiguous],
            &IntegerAffineWitness {
                root: defined,
                target: operand,
                definition_axioms: vec![1],
                literal_axioms: vec![Some(0)],
            },
        ),
        Err(IntegerAffineWitnessError::AmbiguousDefinition(1)),
    );
}

#[test]
fn wrapping_divide_maps_both_directions_and_rejects_zero_or_signed() {
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let axioms = vec![Proposition::Equal(
        target.clone(),
        ScalarTerm::wrapping_integer_divide(
            integer_type,
            root.clone(),
            unsigned_literal(integer_type, 3),
        )
        .unwrap(),
    )];
    let witness = IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms: vec![0],
        literal_axioms: vec![None],
    };
    let checked = check_integer_affine_witness(&context, &axioms, &witness)
        .expect("unsigned wrapping divide traverses forward");
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(root.clone(), unsigned_literal(integer_type, 9)),
        ),
        Ok(Proposition::LessOrEqual(
            target.clone(),
            unsigned_literal(integer_type, 3),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(unsigned_literal(integer_type, 4), root.clone()),
        ),
        Ok(Proposition::LessOrEqual(
            unsigned_literal(integer_type, 1),
            target.clone(),
        )),
    );
    // Division is not a translation: strict endpoints cannot traverse it.
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessThan(root.clone(), unsigned_literal(integer_type, 9)),
        ),
        Err(IntegerAffineBoundConversionError::StrictBoundNotTranslation),
    );

    let zero_divisor = vec![Proposition::Equal(
        target.clone(),
        ScalarTerm::wrapping_integer_divide(
            integer_type,
            root.clone(),
            unsigned_literal(integer_type, 0),
        )
        .unwrap(),
    )];
    assert_eq!(
        check_integer_affine_witness(&context, &zero_divisor, &witness),
        Err(IntegerAffineWitnessError::ZeroDivisionLiteral),
    );
}

/// One signed wrapping sum `target = operand + addend (mod 2^8)` checked as a
/// forward witness from the operand, and the same equation checked backward.
fn signed_wrapping_witnesses(
    addend: i128,
) -> (
    ScalarTerm,
    ScalarTerm,
    crate::integer_rules::integer_affine::CheckedIntegerAffineForm,
    crate::integer_rules::integer_affine::CheckedIntegerAffineForm,
) {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let operand = value(1, integer_type);
    let target = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let axioms = vec![Proposition::Equal(
        target.clone(),
        ScalarTerm::wrapping_integer_add(
            integer_type,
            operand.clone(),
            literal(integer_type, addend),
        )
        .unwrap(),
    )];
    let witness = |root: &ScalarTerm, destination: &ScalarTerm| {
        check_integer_affine_witness(
            &context,
            &axioms,
            &IntegerAffineWitness {
                root: root.clone(),
                target: destination.clone(),
                definition_axioms: vec![0],
                literal_axioms: vec![None],
            },
        )
        .expect("signed wrapping add traverses")
    };
    let forward = witness(&operand, &target);
    let backward = witness(&target, &operand);
    (operand, target, forward, backward)
}

#[test]
fn signed_wrapping_add_demands_evidence_on_the_side_its_addend_can_cross() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let bound = |value| literal(integer_type, value);

    // A positive addend can only cross the maximum: `x <= 9` maps to
    // `x + 1 <= 10` unconditionally, while `0 <= x` needs `x <= 126`.
    let (operand, target, forward, backward) = signed_wrapping_witnesses(1);
    assert_eq!(
        map_integer_affine_bound(
            &forward,
            &Proposition::LessOrEqual(operand.clone(), bound(9))
        ),
        Ok(Proposition::LessOrEqual(target.clone(), bound(10))),
    );
    let lower = Proposition::LessOrEqual(bound(0), operand.clone());
    let headroom = Proposition::LessOrEqual(operand.clone(), bound(126));
    assert_eq!(
        integer_affine_wrapping_evidence(&forward, &lower),
        Ok(vec![headroom.clone()]),
    );
    assert_eq!(
        map_integer_affine_bound(&forward, &lower),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
    );
    assert_eq!(
        map_integer_affine_bound(
            &forward,
            &Proposition::Conjunction(vec![lower, headroom.clone()]),
        ),
        Ok(Proposition::LessOrEqual(bound(1), target.clone())),
    );
    // Backward, the lower side is the unconditional one.
    assert_eq!(
        map_integer_affine_bound(
            &backward,
            &Proposition::LessOrEqual(bound(5), target.clone())
        ),
        Ok(Proposition::LessOrEqual(bound(4), operand.clone())),
    );
    let upper = Proposition::LessOrEqual(target.clone(), bound(5));
    assert_eq!(
        integer_affine_wrapping_evidence(&backward, &upper),
        Ok(vec![headroom]),
    );

    // A negative addend can only cross the minimum: `2 <= x` maps to
    // `1 <= x - 1` unconditionally, while `x <= 9` needs `-127 <= x`.
    let (operand, target, forward, backward) = signed_wrapping_witnesses(-1);
    assert_eq!(
        map_integer_affine_bound(
            &forward,
            &Proposition::LessOrEqual(bound(2), operand.clone())
        ),
        Ok(Proposition::LessOrEqual(bound(1), target.clone())),
    );
    let upper = Proposition::LessOrEqual(operand.clone(), bound(9));
    let floor = Proposition::LessOrEqual(bound(-127), operand.clone());
    assert_eq!(
        integer_affine_wrapping_evidence(&forward, &upper),
        Ok(vec![floor.clone()]),
    );
    assert_eq!(
        map_integer_affine_bound(&forward, &upper),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceMissing),
    );
    assert_eq!(
        map_integer_affine_bound(
            &forward,
            &Proposition::Conjunction(vec![upper, floor.clone()]),
        ),
        Ok(Proposition::LessOrEqual(target.clone(), bound(8))),
    );
    // A positive addend's headroom is not evidence for a negative addend.
    assert_eq!(
        map_integer_affine_bound(
            &forward,
            &Proposition::Conjunction(vec![
                Proposition::LessOrEqual(operand.clone(), bound(9)),
                Proposition::LessOrEqual(operand.clone(), bound(126)),
            ]),
        ),
        Err(IntegerAffineBoundConversionError::WrappingEvidenceUnexpected),
    );
    // Backward, the upper side is the unconditional one.
    assert_eq!(
        map_integer_affine_bound(
            &backward,
            &Proposition::LessOrEqual(target.clone(), bound(5))
        ),
        Ok(Proposition::LessOrEqual(operand.clone(), bound(6))),
    );
    assert_eq!(
        integer_affine_wrapping_evidence(&backward, &Proposition::LessOrEqual(bound(5), target)),
        Ok(vec![floor]),
    );
}
