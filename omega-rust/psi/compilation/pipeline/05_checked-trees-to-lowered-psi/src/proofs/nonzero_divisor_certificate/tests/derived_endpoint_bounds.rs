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
