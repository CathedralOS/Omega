//! A guarded field read and a fresh read share only their live cited equality.
use super::{IntegerType, PropositionContext, ScalarTerm, ScalarType, ValueId, integer, value};
use crate::proofs::nonzero_divisor_certificate::prove_canonical_integer_proposition;
use proof_admission::check_certificate;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, PlaceId, StructuralFieldId, StructuralPlaceKind,
};
use semantic_vocabulary::{IntegerSign, Proposition};

fn field(integer_type: IntegerType, root: u64, member: u64) -> ScalarTerm {
    ScalarTerm::integer_field_path(
        PlaceId::new(root).unwrap(),
        vec![CanonicalStructuralPathSegment::Field(
            StructuralFieldId::new(member).unwrap(),
        )],
        integer_type,
    )
}

fn fixture() -> (PropositionContext, Proposition, Vec<Proposition>) {
    let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let context = PropositionContext::from_value_types_and_places(
        (1..=4).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Integer(signed))),
        (1..=2).map(|identity| {
            (
                PlaceId::new(identity).unwrap(),
                StructuralPlaceKind::Parameter {
                    position: (identity - 1) as u32,
                    is_self: false,
                },
            )
        }),
    )
    .unwrap();
    let mut clauses = vec![
        Proposition::LessOrEqual(integer(signed, 0), value(4, signed)),
        Proposition::LessOrEqual(value(4, signed), integer(signed, 16)),
    ];
    clauses.sort();
    let axioms = vec![
        Proposition::Equal(value(1, signed), field(signed, 1, 1)),
        Proposition::LessOrEqual(value(1, signed), integer(signed, 15)),
        Proposition::Equal(value(2, signed), field(signed, 1, 1)),
        Proposition::LessOrEqual(integer(signed, 0), value(2, signed)),
        Proposition::LessOrEqual(value(2, signed), integer(signed, 16)),
        Proposition::Equal(value(3, signed), integer(signed, 1)),
        Proposition::Equal(
            value(4, signed),
            ScalarTerm::exact_integer_add(signed, value(2, signed), value(3, signed)).unwrap(),
        ),
    ];
    (context, Proposition::Conjunction(clauses), axioms)
}

#[test]
fn guarded_increment_range_uses_live_field_equality_bridge() {
    let (context, goal, axioms) = fixture();
    for reverse_first_equality in [false, true] {
        let mut available = axioms.clone();
        if reverse_first_equality {
            let Proposition::Equal(left, right) = &mut available[0] else {
                panic!("field equality");
            };
            std::mem::swap(left, right);
        }
        let proof = prove_canonical_integer_proposition(&context, &goal, &[], &available)
            .expect("the guarded read bound reaches the fresh read through both live equalities");
        check_certificate(&context, &goal, &[], &available, &proof)
            .expect("the kernel checks each cited substitution and affine addition");
    }
}

#[test]
fn guarded_increment_range_rejects_missing_or_redirected_field_equality() {
    let (context, goal, axioms) = fixture();
    let proof = prove_canonical_integer_proposition(&context, &goal, &[], &axioms)
        .expect("original field bridge proves the range");
    let signed = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    for corruption in [
        "old read absent",
        "fresh read absent",
        "root",
        "path",
        "guard absent",
    ] {
        let mut changed = axioms.clone();
        match corruption {
            // Truth keeps citation positions stable. Reconstruction discarding
            // an invalidated equality must remove its authority, not merely
            // make an old serialized certificate refer to another index.
            "old read absent" => changed[0] = Proposition::Truth,
            "fresh read absent" => changed[2] = Proposition::Truth,
            "root" => changed[2] = Proposition::Equal(value(2, signed), field(signed, 2, 1)),
            "path" => changed[2] = Proposition::Equal(value(2, signed), field(signed, 1, 2)),
            "guard absent" => changed[1] = Proposition::Truth,
            _ => unreachable!(),
        }
        assert!(
            prove_canonical_integer_proposition(&context, &goal, &[], &changed).is_none(),
            "{corruption} cannot produce a new range certificate",
        );
        assert!(
            check_certificate(&context, &goal, &[], &changed, &proof).is_err(),
            "{corruption} cannot replay the old certificate",
        );
    }
}
