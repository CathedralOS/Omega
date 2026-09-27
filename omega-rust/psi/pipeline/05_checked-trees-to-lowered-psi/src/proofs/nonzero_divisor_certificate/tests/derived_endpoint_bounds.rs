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
