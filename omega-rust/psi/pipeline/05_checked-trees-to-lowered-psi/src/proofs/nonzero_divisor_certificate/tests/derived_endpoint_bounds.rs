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
fn computed_index_bound_survives_integer_widening() {
    let source = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let target = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let literal =
        |carrier, number| ScalarTerm::integer(carrier, IntegerValue::Unsigned(number)).unwrap();
    let context = PropositionContext::from_value_types((1..=6).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(if identity == 6 { target } else { source }),
        )
    }))
    .unwrap();
    let assumptions = vec![
        Proposition::LessOrEqual(value(1, source), literal(source, 2)),
        Proposition::LessOrEqual(value(2, source), literal(source, 3)),
    ];
    let axioms = vec![
        Proposition::Equal(value(3, source), literal(source, 4)),
        Proposition::Equal(
            value(4, source),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type: source,
                left: Box::new(value(1, source)),
                right: Box::new(value(3, source)),
            },
        ),
        Proposition::Equal(
            value(5, source),
            ScalarTerm::ExactIntegerAdd {
                scalar_type: source,
                left: Box::new(value(4, source)),
                right: Box::new(value(2, source)),
            },
        ),
        Proposition::Equal(
            value(6, target),
            ScalarTerm::IntegerWiden {
                source_type: source,
                target_type: target,
                operand: Box::new(value(5, source)),
            },
        ),
    ];
    let goal = Proposition::LessThan(value(6, target), literal(target, 12));
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &axioms)
        .expect("widen(y * 4 + x) < 12 follows from y <= 2 and x <= 3");
    proof_admission::check_certificate(&context, &goal, &assumptions, &axioms, &proof)
        .expect("the kernel independently checks the computed source bound and cast");
    let weaker_goal = Proposition::LessThan(value(6, target), literal(target, 13));
    let weaker_proof =
        prove_canonical_integer_proposition(&context, &weaker_goal, &assumptions, &axioms)
            .expect("a derived endpoint also establishes a weaker requested bound");
    proof_admission::check_certificate(
        &context,
        &weaker_goal,
        &assumptions,
        &axioms,
        &weaker_proof,
    )
    .unwrap();
    let mut wider = assumptions.clone();
    wider[1] = Proposition::LessOrEqual(value(2, source), literal(source, 4));
    assert!(proof_admission::check_certificate(&context, &goal, &wider, &axioms, &proof).is_err());
    assert!(prove_canonical_integer_proposition(&context, &goal, &wider, &axioms).is_none());
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
