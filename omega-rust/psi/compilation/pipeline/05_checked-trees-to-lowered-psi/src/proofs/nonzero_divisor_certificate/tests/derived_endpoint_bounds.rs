//! Derived endpoint bounds through cited equalities and single definitions.
//!
//! Corpus obligations bound exact-operation results whose operands reach their
//! evidence only through `Equal` chains — a field aliases a computed value,
//! which aliases a landed literal. These tests pin that producer family: the
//! certificate must compose the cited equality hops and one checked
//! definition step, and must still refuse when no operand evidence exists.

use super::{integer, value};
use crate::proofs::nonzero_divisor_certificate::{
    Proposition, PropositionContext, prove_canonical_integer_proposition,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
    PlaceId, ScalarTerm, ScalarType, StructuralFieldId, StructuralPlaceKind, ValueId,
};

fn math_value(id: u64, integer_type: IntegerType) -> IntegerMathTerm {
    IntegerMathTerm::MathValue {
        source_type: integer_type,
        value: ValueId::new(id).expect("value id"),
    }
}

#[test]
fn a_nonzero_guard_survives_a_second_read_of_the_same_field() {
    use crate::proofs::nonzero_divisor_certificate::produce_relaxed_integer_proof;
    use std::collections::BTreeSet;

    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let root = PlaceId::new(1).unwrap();
    let context = PropositionContext::from_value_types_and_places(
        (1..=3)
            .map(|identity| (ValueId::new(identity).unwrap(), scalar_type))
            .chain([(ValueId::new(6).unwrap(), scalar_type)])
            .chain((4..=5).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Boolean))),
        [(
            root,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        )],
    )
    .unwrap();
    let observed = |identity| value(identity, integer_type);
    let field = ScalarTerm::integer_field_path(
        root,
        vec![CanonicalStructuralPathSegment::Field(
            StructuralFieldId::new(1).unwrap(),
        )],
        integer_type,
    );
    let nonzero = |term: ScalarTerm| {
        Proposition::Disjunction(vec![
            Proposition::LessOrEqual(term.clone(), integer(integer_type, -1)),
            Proposition::LessOrEqual(integer(integer_type, 1), term),
        ])
    };
    let polarity = |identity, truth| {
        Proposition::Equal(
            ScalarTerm::value(ValueId::new(identity).unwrap(), ScalarType::Boolean),
            ScalarTerm::Boolean(truth),
        )
    };
    let implication = |premise, conclusion| Proposition::Implication {
        premise: Box::new(premise),
        conclusion: Box::new(conclusion),
    };
    let mut axioms = vec![
        Proposition::Equal(observed(1), field.clone()),
        Proposition::Equal(observed(3), integer(integer_type, 0)),
        implication(
            Proposition::Equal(observed(1), observed(3)),
            polarity(4, true),
        ),
        implication(
            Proposition::Disjunction(vec![
                Proposition::LessThan(observed(1), observed(3)),
                Proposition::LessThan(observed(3), observed(1)),
            ]),
            polarity(4, false),
        ),
        implication(polarity(4, false), polarity(5, true)),
        implication(polarity(4, true), polarity(5, false)),
        nonzero(observed(1)),
        Proposition::Equal(observed(2), field),
        Proposition::Equal(
            ScalarTerm::value(ValueId::new(4).unwrap(), ScalarType::Boolean),
            ScalarTerm::IntegerEqual {
                scalar_type: integer_type,
                left: Box::new(observed(1)),
                right: Box::new(observed(3)),
            },
        ),
        Proposition::Equal(
            ScalarTerm::value(ValueId::new(5).unwrap(), ScalarType::Boolean),
            ScalarTerm::BooleanNot {
                operand: Box::new(ScalarTerm::value(
                    ValueId::new(4).unwrap(),
                    ScalarType::Boolean,
                )),
            },
        ),
    ];
    axioms.push(Proposition::Equal(
        ScalarTerm::integer_field_path(
            root,
            vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(1).unwrap(),
            )],
            integer_type,
        ),
        observed(6),
    ));
    let goal = nonzero(observed(2));
    let proof = produce_relaxed_integer_proof(&context, &goal, &[], &axioms, &BTreeSet::new())
        .expect("a guarded field read and later unchanged read share the nonzero proof");
    proof_admission::accept_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
    axioms[6] = Proposition::Truth;
    assert!(
        produce_relaxed_integer_proof(&context, &goal, &[], &axioms, &BTreeSet::new()).is_none()
    );
    assert!(proof_admission::accept_certificate(&context, &goal, &[], &axioms, &proof).is_err());
}

#[test]
fn strict_bounds_reuse_the_derived_store_and_read_chain() {
    use crate::proofs::nonzero_divisor_certificate::produce_relaxed_integer_proof;
    use std::collections::BTreeSet;

    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let root = PlaceId::new(1).unwrap();
    let context = PropositionContext::from_value_types_and_places(
        (1..=6).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        [(
            root,
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        )],
    )
    .unwrap();
    let observed = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
    let literal =
        |number| ScalarTerm::integer(integer_type, IntegerValue::Unsigned(number)).unwrap();
    let field = |identity| {
        ScalarTerm::integer_field_path(
            root,
            vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(identity).unwrap(),
            )],
            integer_type,
        )
    };
    let mut axioms = vec![
        Proposition::LessOrEqual(field(1), literal(4)),
        Proposition::Equal(observed(1), field(1)),
        Proposition::LessOrEqual(observed(1), literal(3)),
        Proposition::Equal(observed(2), field(1)),
        Proposition::Equal(observed(3), literal(1)),
        Proposition::Equal(
            observed(4),
            ScalarTerm::exact_integer_add(integer_type, observed(2), observed(3)).unwrap(),
        ),
        Proposition::Equal(field(2), observed(4)),
        Proposition::Equal(observed(5), field(2)),
        Proposition::Equal(observed(6), literal(5)),
    ];
    let parameters = BTreeSet::new();
    let nonstrict = Proposition::LessOrEqual(observed(5), literal(4));
    produce_relaxed_integer_proof(&context, &nonstrict, &[], &axioms, &parameters)
        .expect("the existing closure proves the inclusive endpoint");
    let goals = [
        Proposition::LessThan(observed(5), literal(5)),
        Proposition::LessThan(observed(5), observed(6)),
        Proposition::LessThan(literal(0), observed(5)),
    ];
    let proofs = goals
        .iter()
        .map(|goal| {
            produce_relaxed_integer_proof(&context, goal, &[], &axioms, &parameters)
                .expect("strict bounds retain the same field and definition evidence")
        })
        .collect::<Vec<_>>();
    assert!(
        produce_relaxed_integer_proof(
            &context,
            &Proposition::LessThan(observed(5), literal(4)),
            &[],
            &axioms,
            &parameters,
        )
        .is_none(),
        "discreteness cannot strengthen the proved endpoint"
    );
    axioms[2] = Proposition::Truth;
    for (goal, proof) in goals.iter().zip(&proofs).take(2) {
        assert!(produce_relaxed_integer_proof(&context, goal, &[], &axioms, &parameters).is_none());
        assert!(
            proof_admission::accept_certificate(&context, goal, &[], &axioms, proof).is_err(),
            "the weaker invariant alone cannot replace the removed guard"
        );
    }
}

#[test]
fn bounded_runtime_product_supplies_the_next_multiplication_endpoints() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let context = PropositionContext::from_value_types((1..=3).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type),
        )
    }))
    .unwrap();
    let axioms = vec![
        Proposition::LessOrEqual(integer(integer_type, 0), value(1, integer_type)),
        Proposition::LessOrEqual(value(1, integer_type), integer(integer_type, 100)),
        Proposition::Equal(
            value(2, integer_type),
            ScalarTerm::exact_integer_multiply(
                integer_type,
                value(1, integer_type),
                value(1, integer_type),
            )
            .unwrap(),
        ),
        Proposition::Equal(value(3, integer_type), integer(integer_type, 3)),
    ];
    let product = IntegerMathTerm::Multiply(
        Box::new(math_value(2, integer_type)),
        Box::new(math_value(3, integer_type)),
    );
    let goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(integer_type.minimum_value()),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(
            product,
            IntegerMathTerm::literal(integer_type.maximum_value()),
        ),
    ]);
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &axioms)
        .expect("the prior bounded square supplies endpoints for multiplication by three");
    proof_admission::accept_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
    let mut missing = axioms.clone();
    missing[1] = Proposition::Truth;
    assert!(prove_canonical_integer_proposition(&context, &goal, &[], &missing).is_none());
    assert!(proof_admission::accept_certificate(&context, &goal, &[], &missing, &proof).is_err());
}

#[test]
fn product_endpoints_preserve_signed_corners_and_prior_definition_identity() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let context = PropositionContext::from_value_types((1..=3).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type),
        )
    }))
    .unwrap();
    let assumptions = vec![Proposition::Truth];
    for (left_lower, left_upper, right_lower, right_upper, product_lower, product_upper) in [
        (-4, 5, -3, 2, -15, 12),
        (-8, -2, -7, -3, 6, 56),
        (2, 6, 3, 8, 6, 48),
        (-4, 5, 3, 3, -12, 15),
    ] {
        for reversed in [false, true] {
            let expression = ScalarTerm::exact_integer_multiply(
                integer_type,
                value(1, integer_type),
                if right_lower == right_upper {
                    integer(integer_type, right_lower)
                } else {
                    value(2, integer_type)
                },
            )
            .unwrap();
            let definition = if reversed {
                Proposition::Equal(expression, value(3, integer_type))
            } else {
                Proposition::Equal(value(3, integer_type), expression)
            };
            let axioms = vec![
                Proposition::LessOrEqual(integer(integer_type, left_lower), value(1, integer_type)),
                Proposition::LessOrEqual(value(1, integer_type), integer(integer_type, left_upper)),
                Proposition::LessOrEqual(
                    integer(integer_type, right_lower),
                    value(2, integer_type),
                ),
                Proposition::LessOrEqual(
                    value(2, integer_type),
                    integer(integer_type, right_upper),
                ),
                definition,
            ];
            let goal = Proposition::Conjunction(vec![
                Proposition::LessOrEqual(
                    integer(integer_type, product_lower),
                    value(3, integer_type),
                ),
                Proposition::LessOrEqual(
                    value(3, integer_type),
                    integer(integer_type, product_upper),
                ),
            ]);
            let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
                .or_else(|| super::super::produce_relaxed_integer_proof(
                    &context, &goal, &assumptions, &axioms, &Default::default(),
                ))
                .unwrap_or_else(|| panic!(
                    "both signed endpoints: [{left_lower}, {left_upper}] * [{right_lower}, {right_upper}], reversed={reversed}",
                ));
            proof_admission::accept_certificate(&context, &goal, &assumptions, &axioms, &proof)
                .unwrap();
            let bundle = terminal_psi::ProofBundle {
                evidence: vec![terminal_psi::ObligationEvidence {
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                    route: proof_admission::EvidenceRoute::CertificateDerived(
                        proof_admission::CertificateEnvelope {
                            identity: semantic_vocabulary::EvidenceIdentity::new(1).unwrap(),
                            proof_system_marker: proof_admission::ProofSystemMarker::CURRENT,
                            proof: proof.clone(),
                        },
                    ),
                }],
                ..Default::default()
            };
            let bytes = terminal_codec::encode_proof_bundle(&bundle).unwrap();
            let decoded = terminal_codec::decode_proof_bundle(&bytes).unwrap();
            assert_eq!(decoded, bundle);
            let proof_admission::EvidenceRoute::CertificateDerived(certificate) =
                &decoded.evidence[0].route
            else {
                panic!("certificate route")
            };
            for changed in [
                Proposition::Truth,
                Proposition::Equal(
                    value(3, integer_type),
                    ScalarTerm::exact_integer_multiply(
                        integer_type,
                        value(1, integer_type),
                        value(1, integer_type),
                    )
                    .unwrap(),
                ),
            ] {
                let mut substituted = axioms.clone();
                substituted[4] = changed;
                assert!(
                    proof_admission::accept_certificate(
                        &context,
                        &goal,
                        &assumptions,
                        &substituted,
                        &certificate.proof,
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn derived_bounds_refuse_when_an_operand_has_no_evidence() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    // v1 = 10 but v2 is an unbounded field read: no cited or derived bound
    // reaches `v1 + v2 <= 15`, and no certificate may be invented.
    let context = PropositionContext::from_value_types_and_places(
        (1..=2).map(|identity| {
            (
                ValueId::new(identity).expect("value id"),
                ScalarType::Integer(integer_type),
            )
        }),
        [(
            PlaceId::new(1).expect("place id"),
            StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        )],
    )
    .expect("context with the field's root place");
    let axioms = vec![
        Proposition::Equal(value(1, integer_type), integer(integer_type, 10)),
        Proposition::Equal(
            value(2, integer_type),
            ScalarTerm::integer_field_path(
                PlaceId::new(1).expect("place id"),
                vec![CanonicalStructuralPathSegment::Field(
                    StructuralFieldId::new(2).expect("field id"),
                )],
                integer_type,
            ),
        ),
    ];
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Add(
            Box::new(math_value(1, integer_type)),
            Box::new(math_value(2, integer_type)),
        ),
        IntegerMathTerm::literal(IntegerValue::Signed(15)),
    );
    assert!(
        prove_canonical_integer_proposition(&context, &goal, &[], &axioms).is_none(),
        "an unbounded field operand cannot close a finite sum bound"
    );
}

#[test]
fn closed_wrapping_call_result_supplies_exact_multiply_bounds() {
    let ty = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let context = PropositionContext::from_value_types(
        (1..=17).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(ty))),
    )
    .unwrap();
    // These are the retained call/state aliases at the authored obligation:
    // v5 -> v8 -> v15 = wrapping(v10 * 2), v10 -> v16 = 6, v6 -> v14 = 3.
    // Unrelated receiver aliases form a cycle; they carry no operand evidence.
    let axioms = vec![
        Proposition::Equal(value(17, ty), value(9, ty)),
        Proposition::Equal(value(11, ty), value(17, ty)),
        Proposition::Equal(value(9, ty), value(11, ty)),
        Proposition::Equal(value(16, ty), integer(ty, 6)),
        Proposition::Equal(value(10, ty), value(16, ty)),
        Proposition::Equal(
            value(15, ty),
            ScalarTerm::wrapping_integer_multiply(ty, value(10, ty), integer(ty, 2)).unwrap(),
        ),
        Proposition::Equal(value(8, ty), value(15, ty)),
        Proposition::Equal(value(14, ty), integer(ty, 3)),
        Proposition::Equal(value(5, ty), value(8, ty)),
        Proposition::Equal(value(6, ty), value(14, ty)),
    ];
    let product =
        IntegerMathTerm::Multiply(Box::new(math_value(5, ty)), Box::new(math_value(6, ty)));
    let goal = Proposition::Conjunction(vec![
        Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(ty.minimum_value()),
            product.clone(),
        ),
        Proposition::IntegerMathLessOrEqual(product, IntegerMathTerm::literal(ty.maximum_value())),
    ]);
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &axioms)
        .expect("closed wrapping call result has a certified literal endpoint");
    proof_admission::accept_certificate(&context, &goal, &[], &axioms, &proof).unwrap();
    let mut missing = axioms.clone();
    missing[3] = Proposition::Truth;
    assert!(prove_canonical_integer_proposition(&context, &goal, &[], &missing).is_none());
    assert!(proof_admission::accept_certificate(&context, &goal, &[], &missing, &proof).is_err());
}
