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
use proof_admission::accept_certificate;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
    PlaceId, ScalarTerm, ScalarType, StructuralFieldId, StructuralPlaceKind, ValueId,
};

fn context(integer_type: IntegerType, count: u64) -> PropositionContext {
    PropositionContext::from_value_types((1..=count).map(|id| {
        (
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(integer_type),
        )
    }))
    .expect("value context")
}

fn math_value(id: u64, integer_type: IntegerType) -> IntegerMathTerm {
    IntegerMathTerm::MathValue {
        source_type: integer_type,
        value: ValueId::new(id).expect("value id"),
    }
}

#[test]
fn exact_add_goal_relaxes_a_derived_operand_endpoint() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    // v1 = 10, v2 = v1, v3 = 5, v4 = v2 + v3, v5 = v4, v6 = 5.
    // The add operand candidate frontier only cites literal 10 for v5's
    // side: `10 <= v5` must be derived from `15 <= v5` — itself composed
    // through v4's exact-add definition and the cited equalities — then
    // relaxed. The resulting `15 <= v5 + v6` still relaxes to the carrier
    // minimum arm.
    let axioms = vec![
        Proposition::Equal(value(1, integer_type), integer(integer_type, 10)),
        Proposition::Equal(value(2, integer_type), value(1, integer_type)),
        Proposition::Equal(value(3, integer_type), integer(integer_type, 5)),
        Proposition::Equal(
            value(4, integer_type),
            ScalarTerm::exact_integer_add(
                integer_type,
                value(2, integer_type),
                value(3, integer_type),
            )
            .expect("exact add"),
        ),
        Proposition::Equal(value(5, integer_type), value(4, integer_type)),
        Proposition::Equal(value(6, integer_type), integer(integer_type, 5)),
    ];
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(IntegerValue::Signed(-2147483648)),
        IntegerMathTerm::Add(
            Box::new(math_value(5, integer_type)),
            Box::new(math_value(6, integer_type)),
        ),
    );
    let proof = prove_canonical_integer_proposition(&context(integer_type, 6), &goal, &[], &axioms)
        .expect("a relaxed derived operand endpoint closes the carrier lower arm");
    accept_certificate(&context(integer_type, 6), &goal, &[], &axioms, &proof)
        .expect("the relaxed closure certificate checks in the kernel");
}

#[test]
fn exact_subtract_goal_derives_operand_bounds_through_add_definition() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    // v7 = v5 - v6 where v5 aliases v4 = v2 + v3 (10 + 5) and v6 = 3:
    // `12 <= v5 - v6` relaxes to the carrier minimum arm.
    let axioms = vec![
        Proposition::Equal(value(1, integer_type), integer(integer_type, 10)),
        Proposition::Equal(value(2, integer_type), value(1, integer_type)),
        Proposition::Equal(value(3, integer_type), integer(integer_type, 5)),
        Proposition::Equal(
            value(4, integer_type),
            ScalarTerm::exact_integer_add(
                integer_type,
                value(2, integer_type),
                value(3, integer_type),
            )
            .expect("exact add"),
        ),
        Proposition::Equal(value(5, integer_type), value(4, integer_type)),
        Proposition::Equal(value(6, integer_type), integer(integer_type, 3)),
        Proposition::Equal(
            value(7, integer_type),
            ScalarTerm::exact_integer_subtract(
                integer_type,
                value(5, integer_type),
                value(6, integer_type),
            )
            .expect("exact subtract"),
        ),
    ];
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::literal(IntegerValue::Signed(-2147483648)),
        IntegerMathTerm::Subtract(
            Box::new(math_value(5, integer_type)),
            Box::new(math_value(6, integer_type)),
        ),
    );
    let proof = prove_canonical_integer_proposition(&context(integer_type, 7), &goal, &[], &axioms)
        .expect("the derived `v5 <= 15` upper and `15 <= v5` lower close the difference");
    accept_certificate(&context(integer_type, 7), &goal, &[], &axioms, &proof)
        .expect("the subtract certificate checks in the kernel");
}

#[test]
fn exact_multiply_goal_derives_operand_corners_through_equalities() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
    // v8 = v5 aliases the add-defined value and v9 = 2: the multiply's
    // operand corners come from the derived `v5 <= 15`/`15 <= v5` pair.
    let axioms = vec![
        Proposition::Equal(value(1, integer_type), integer(integer_type, 10)),
        Proposition::Equal(value(2, integer_type), value(1, integer_type)),
        Proposition::Equal(value(3, integer_type), integer(integer_type, 5)),
        Proposition::Equal(
            value(4, integer_type),
            ScalarTerm::exact_integer_add(
                integer_type,
                value(2, integer_type),
                value(3, integer_type),
            )
            .expect("exact add"),
        ),
        Proposition::Equal(value(5, integer_type), value(4, integer_type)),
        Proposition::Equal(value(8, integer_type), value(5, integer_type)),
        Proposition::Equal(value(9, integer_type), integer(integer_type, 2)),
        Proposition::Equal(
            value(10, integer_type),
            ScalarTerm::exact_integer_multiply(
                integer_type,
                value(8, integer_type),
                value(9, integer_type),
            )
            .expect("exact multiply"),
        ),
    ];
    let goal = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::Multiply(
            Box::new(math_value(8, integer_type)),
            Box::new(math_value(9, integer_type)),
        ),
        IntegerMathTerm::literal(IntegerValue::Signed(2147483647)),
    );
    let proof =
        prove_canonical_integer_proposition(&context(integer_type, 10), &goal, &[], &axioms)
            .expect("derived operand corners `15 * 2` bound the product");
    accept_certificate(&context(integer_type, 10), &goal, &[], &axioms, &proof)
        .expect("the multiply certificate checks in the kernel");
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
