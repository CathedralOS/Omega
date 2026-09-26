use super::{literal, value};
use crate::integer_rules::integer_affine::witness_checking::CheckedIntegerEndpointStep;
use crate::integer_rules::integer_affine::{
    IntegerAffineWitness, IntegerAffineWitnessError, check_integer_affine_witness,
    integer_affine_wrapping_evidence, map_integer_affine_bound,
};
use semantic_vocabulary::IntegerType;
use semantic_vocabulary::Proposition;
use semantic_vocabulary::PropositionContext;
use semantic_vocabulary::ScalarTerm;
use semantic_vocabulary::ScalarType;
use semantic_vocabulary::{IntegerSign, ValueId};

#[test]
fn checks_ordered_add_subtract_multiply_normalization() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let root = value(1, integer_type);
    let added = value(2, integer_type);
    let subtracted = value(3, integer_type);
    let target = value(4, integer_type);
    let axioms = vec![
        Proposition::Equal(
            added.clone(),
            ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(integer_type, 7))
                .unwrap(),
        ),
        Proposition::Equal(
            ScalarTerm::exact_integer_subtract(
                integer_type,
                added.clone(),
                literal(integer_type, 2),
            )
            .unwrap(),
            subtracted.clone(),
        ),
        Proposition::Equal(
            target.clone(),
            ScalarTerm::exact_integer_multiply(integer_type, literal(integer_type, -3), subtracted)
                .unwrap(),
        ),
    ];
    let context = PropositionContext::from_value_types(
        (1..=4).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let checked = check_integer_affine_witness(
        &context,
        &axioms,
        &IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            definition_axioms: vec![0, 1, 2],
            literal_axioms: vec![None, None, None],
        },
    )
    .expect("ordered affine witness");
    assert_eq!(checked.root(), &root);
    assert_eq!(checked.target(), &target);
    assert_eq!(checked.coefficient(), -3);
    assert_eq!(checked.offset(), -15);
}

#[test]
fn checks_landed_literal_sibling_and_rejects_stale_or_unused_custody() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let root = value(1, integer_type);
    let sibling = value(2, integer_type);
    let target = value(3, integer_type);
    let context = PropositionContext::from_value_types(
        (1..=5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let landing = Proposition::Equal(sibling.clone(), literal(integer_type, 7));
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::exact_integer_add(integer_type, root.clone(), sibling.clone()).unwrap(),
    );
    let witness = IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms: vec![1],
        literal_axioms: vec![Some(0)],
    };

    let checked =
        check_integer_affine_witness(&context, &[landing.clone(), definition.clone()], &witness)
            .expect("an earlier exact landing supplies the affine sibling literal");
    assert_eq!(checked.coefficient(), 1);
    assert_eq!(checked.offset(), 7);

    let missing_alignment = IntegerAffineWitness {
        literal_axioms: Vec::new(),
        ..witness.clone()
    };
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &[landing.clone(), definition.clone()],
            &missing_alignment,
        ),
        Err(IntegerAffineWitnessError::LiteralAxiomCountMismatch),
    );

    let late_landing = IntegerAffineWitness {
        definition_axioms: vec![0],
        literal_axioms: vec![Some(1)],
        ..witness.clone()
    };
    assert_eq!(
        check_integer_affine_witness(&context, &[definition, landing], &late_landing),
        Err(IntegerAffineWitnessError::LiteralAxiomNotPrior {
            definition: 0,
            literal: 1,
        }),
    );

    let inline_definition = Proposition::Equal(
        target,
        ScalarTerm::exact_integer_add(integer_type, root, literal(integer_type, 7)).unwrap(),
    );
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &[
                Proposition::Equal(sibling, literal(integer_type, 7)),
                inline_definition
            ],
            &witness,
        ),
        Err(IntegerAffineWitnessError::UnusedLiteralAxiom(1)),
    );
}

#[test]
fn exact_add_backward_maps_bounds_and_rejects_ambiguity() {
    // A signed carrier traverses the exact-add backward step: the checked
    // no-overflow obligation makes `operand = defined - literal` a true
    // integer equation, so no wrap headroom evidence is needed.
    let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
    let operand = value(1, integer_type);
    let defined = value(2, integer_type);
    let sibling = value(3, integer_type);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let landing = Proposition::Equal(sibling.clone(), literal(integer_type, 1));
    let definition = Proposition::Equal(
        defined.clone(),
        ScalarTerm::exact_integer_add(integer_type, operand.clone(), sibling.clone()).unwrap(),
    );
    let checked = check_integer_affine_witness(
        &context,
        &[landing, definition],
        &IntegerAffineWitness {
            root: defined.clone(),
            target: operand.clone(),
            definition_axioms: vec![1],
            literal_axioms: vec![Some(0)],
        },
    )
    .expect("exact add traverses toward its operand");
    assert_eq!(checked.coefficient(), 1);
    assert_eq!(checked.offset(), -1);
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessThan(defined.clone(), literal(integer_type, 3)),
        ),
        Ok(Proposition::LessThan(
            operand.clone(),
            literal(integer_type, 2),
        )),
    );
    assert_eq!(
        map_integer_affine_bound(
            &checked,
            &Proposition::LessOrEqual(literal(integer_type, 1), defined.clone()),
        ),
        Ok(Proposition::LessOrEqual(
            literal(integer_type, 0),
            operand.clone(),
        )),
    );
    // Backward traversal is a translation, so a strict bound needs no
    // wrapping evidence at all.
    assert_eq!(
        integer_affine_wrapping_evidence(
            &checked,
            &Proposition::LessThan(defined.clone(), literal(integer_type, 3)),
        ),
        Ok(Vec::new()),
    );

    // Both addends landing makes the operand choice ambiguous and rejects.
    let ambiguous = Proposition::Equal(
        defined.clone(),
        ScalarTerm::exact_integer_add(integer_type, operand.clone(), operand.clone()).unwrap(),
    );
    let landed_operand = Proposition::Equal(operand.clone(), literal(integer_type, 4));
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
fn rejects_reordered_stale_and_non_affine_definitions() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let root = value(1, integer_type);
    let target = value(2, integer_type);
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(integer_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(integer_type)),
    ])
    .unwrap();
    let axiom = Proposition::Equal(
        target.clone(),
        ScalarTerm::exact_integer_add(integer_type, root.clone(), literal(integer_type, 1))
            .unwrap(),
    );
    let witness = |definition_axioms: Vec<usize>| IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        literal_axioms: vec![None; definition_axioms.len()],
        definition_axioms,
    };
    assert_eq!(
        check_integer_affine_witness(&context, std::slice::from_ref(&axiom), &witness(vec![1])),
        Err(IntegerAffineWitnessError::UnknownSemanticAxiom(1)),
    );
    assert_eq!(
        check_integer_affine_witness(&context, &[axiom.clone(), axiom], &witness(vec![1, 0])),
        Err(IntegerAffineWitnessError::NonCanonicalDefinitionOrder),
    );
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &[Proposition::Equal(target.clone(), root.clone())],
            &witness(vec![0]),
        ),
        Err(IntegerAffineWitnessError::DefinitionShapeMismatch(0)),
    );
}

#[test]
fn rejects_non_value_roots_stale_unsigned_words_and_checked_overflow() {
    let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
    let i8_target = value(1, i8_type);
    let i8_context = PropositionContext::from_value_types([(
        ValueId::new(1).unwrap(),
        ScalarType::Integer(i8_type),
    )])
    .unwrap();
    assert_eq!(
        check_integer_affine_witness(
            &i8_context,
            &[Proposition::Equal(
                i8_target.clone(),
                ScalarTerm::exact_integer_add(i8_type, literal(i8_type, 0), literal(i8_type, 1),)
                    .unwrap(),
            )],
            &IntegerAffineWitness {
                root: literal(i8_type, 0),
                target: i8_target,
                definition_axioms: vec![0],
                literal_axioms: vec![None],
            },
        ),
        Err(IntegerAffineWitnessError::RootNotValue),
    );

    let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
    let u8_root = value(1, u8_type);
    let u8_target = value(2, u8_type);
    let u8_context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(u8_type)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(u8_type)),
    ])
    .unwrap();
    assert_eq!(
        check_integer_affine_witness(
            &u8_context,
            &[],
            &IntegerAffineWitness {
                root: u8_root,
                target: u8_target,
                definition_axioms: vec![0],
                literal_axioms: vec![None],
            },
        ),
        Err(IntegerAffineWitnessError::UnknownSemanticAxiom(0)),
    );

    let i128_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
    let root = value(1, i128_type);
    let intermediate = value(2, i128_type);
    let target = value(3, i128_type);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(i128_type))),
    )
    .unwrap();
    let axioms = [
        Proposition::Equal(
            intermediate.clone(),
            ScalarTerm::exact_integer_multiply(
                i128_type,
                root.clone(),
                literal(i128_type, i128::MAX),
            )
            .unwrap(),
        ),
        Proposition::Equal(
            target.clone(),
            ScalarTerm::exact_integer_multiply(i128_type, intermediate, literal(i128_type, 2))
                .unwrap(),
        ),
    ];
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &axioms,
            &IntegerAffineWitness {
                root,
                target,
                definition_axioms: vec![0, 1],
                literal_axioms: vec![None, None],
            },
        ),
        Err(IntegerAffineWitnessError::CoefficientOverflow),
    );
}

#[test]
fn bitwise_and_mask_witness_checks_both_orders_and_lands_a_value_mask() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root = value(1, integer_type);
    let sibling = value(2, integer_type);
    let target = value(3, integer_type);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let witness = || IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms: vec![0],
        literal_axioms: vec![None],
    };

    // A non-negative literal mask admits in either operand order.
    for expression in [
        ScalarTerm::integer_bitwise_and(integer_type, root.clone(), literal(integer_type, 15))
            .unwrap(),
        ScalarTerm::integer_bitwise_and(integer_type, literal(integer_type, 15), root.clone())
            .unwrap(),
    ] {
        let checked = check_integer_affine_witness(
            &context,
            &[Proposition::Equal(target.clone(), expression)],
            &witness(),
        )
        .expect("non-negative mask admits in either operand order");
        assert_eq!(
            checked.endpoint_steps.as_slice(),
            &[CheckedIntegerEndpointStep::BitwiseAndMask(15)],
        );
    }

    // A value mask resolves through a prior literal axiom.
    let landing = Proposition::Equal(sibling.clone(), literal(integer_type, 15));
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_bitwise_and(integer_type, root.clone(), sibling.clone()).unwrap(),
    );
    let checked = check_integer_affine_witness(
        &context,
        &[landing, definition],
        &IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            definition_axioms: vec![1],
            literal_axioms: vec![Some(0)],
        },
    )
    .expect("a landed value mask resolves");
    assert_eq!(
        checked.endpoint_steps.as_slice(),
        &[CheckedIntegerEndpointStep::BitwiseAndMask(15)],
    );
}

#[test]
fn bitwise_and_mask_witness_rejects_negative_and_unlanded_masks() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    let root = value(1, integer_type);
    let sibling = value(2, integer_type);
    let target = value(3, integer_type);
    let context = PropositionContext::from_value_types(
        (1..=3).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))),
    )
    .unwrap();
    let witness = || IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms: vec![0],
        literal_axioms: vec![None],
    };

    // A negative signed mask leaves the sign bit reachable: no `[0, mask]`
    // image exists, so the step fails closed in either operand order.
    for mask in [literal(integer_type, -1), literal(integer_type, -256)] {
        for expression in [
            ScalarTerm::integer_bitwise_and(integer_type, root.clone(), mask.clone()).unwrap(),
            ScalarTerm::integer_bitwise_and(integer_type, mask.clone(), root.clone()).unwrap(),
        ] {
            assert_eq!(
                check_integer_affine_witness(
                    &context,
                    &[Proposition::Equal(target.clone(), expression)],
                    &witness(),
                ),
                Err(IntegerAffineWitnessError::NegativeBitwiseAndMask),
            );
        }
    }

    // A value mask with no literal axiom fails closed as a shape mismatch.
    let unlanded = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_bitwise_and(integer_type, root.clone(), sibling.clone()).unwrap(),
    );
    assert_eq!(
        check_integer_affine_witness(&context, &[unlanded], &witness()),
        Err(IntegerAffineWitnessError::DefinitionShapeMismatch(0)),
    );

    // A cited literal axiom the mask does not need cannot ride along unused.
    let landing = Proposition::Equal(sibling, literal(integer_type, 7));
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_bitwise_and(integer_type, root, literal(integer_type, 15)).unwrap(),
    );
    assert_eq!(
        check_integer_affine_witness(
            &context,
            &[landing, definition],
            &IntegerAffineWitness {
                root: value(1, integer_type),
                target,
                definition_axioms: vec![1],
                literal_axioms: vec![Some(0)],
            },
        ),
        Err(IntegerAffineWitnessError::UnusedLiteralAxiom(1)),
    );
}
