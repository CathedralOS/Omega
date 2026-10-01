//! A guard proved on one computation bounds the cast of an identical
//! recomputation.
//!
//! A state that guards `row * 20 + column < 200` and a later state that reads
//! `row` and `column` again to index with `row * 20 + column` produce two
//! distinct values whose only connection is their definition equations over
//! the same observations. The index converts to `u64` through an exact cast,
//! so its bound needs the guard carried through both the recomputation and
//! the cast chain.

use std::collections::BTreeSet;

use proof_admission::check_certificate;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;

fn signed() -> IntegerType {
    IntegerType::new(IntegerSign::Signed, 32).unwrap()
}

fn unsigned() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
}

fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::Integer(integer_type))
}

fn literal(value: i128) -> ScalarTerm {
    ScalarTerm::integer(signed(), IntegerValue::Signed(value)).unwrap()
}

const ROW: u64 = 100;
const COLUMN: u64 = 101;
const CAST: u64 = 40;

/// `row * 20 + column` evaluated into the fresh values `base..=base + 4` from
/// the shared observations `ROW` and `COLUMN`; `base + 4` is the index.
fn index_computation(base: u64) -> Vec<Proposition> {
    let row = value(base, signed());
    let width = value(base + 1, signed());
    let product = value(base + 2, signed());
    let column = value(base + 3, signed());
    vec![
        Proposition::Equal(row.clone(), value(ROW, signed())),
        Proposition::Equal(width.clone(), literal(20)),
        Proposition::Equal(
            product.clone(),
            ScalarTerm::wrapping_integer_multiply(signed(), row, width).unwrap(),
        ),
        Proposition::Equal(column.clone(), value(COLUMN, signed())),
        Proposition::Equal(
            value(base + 4, signed()),
            ScalarTerm::wrapping_integer_add(signed(), product, column).unwrap(),
        ),
    ]
}

/// The guarded computations, then the recomputed index and its cast.
fn roster(upper_guard: bool) -> Vec<Proposition> {
    let mut axioms = index_computation(10);
    axioms.push(Proposition::LessOrEqual(literal(0), value(14, signed())));
    axioms.extend(index_computation(20));
    if upper_guard {
        axioms.push(Proposition::LessThan(value(24, signed()), literal(200)));
        axioms.push(Proposition::LessOrEqual(value(24, signed()), literal(199)));
    }
    axioms.extend(index_computation(30));
    axioms.push(Proposition::Equal(
        value(CAST, unsigned()),
        ScalarTerm::integer_exact_cast(signed(), unsigned(), value(34, signed())).unwrap(),
    ));
    axioms
}

fn context() -> PropositionContext {
    let mut types = vec![
        (ValueId::new(ROW).unwrap(), ScalarType::Integer(signed())),
        (ValueId::new(COLUMN).unwrap(), ScalarType::Integer(signed())),
        (ValueId::new(CAST).unwrap(), ScalarType::Integer(unsigned())),
    ];
    for base in [10, 20, 30] {
        types.extend(
            (base..base + 5).map(|id| (ValueId::new(id).unwrap(), ScalarType::Integer(signed()))),
        );
    }
    PropositionContext::from_value_types(types).unwrap()
}

fn prove(
    context: &PropositionContext,
    axioms: &[Proposition],
) -> Option<proof_admission::ProofNode> {
    produce_checked_canonical_integer_proof(
        context,
        &goal(),
        &[],
        axioms,
        &BTreeSet::new(),
        &mut crate::proofs::ProofSearchDeclines::default(),
    )
}

fn goal() -> Proposition {
    Proposition::LessThan(
        value(CAST, unsigned()),
        ScalarTerm::integer(unsigned(), IntegerValue::Unsigned(200)).unwrap(),
    )
}

#[test]
fn a_recomputed_cast_index_inherits_the_guard_on_the_first_computation() {
    let context = context();
    let axioms = roster(true);
    let proof = prove(&context, &axioms).expect("the recomputed index carries the guard");
    check_certificate(&context, &goal(), &[], &axioms, &proof).unwrap();

    // The certificate cites the guard: replacing its rows with `Truth` at the
    // same positions invalidates the replay, and search without the guard
    // finds no proof.
    let mut withdrawn = axioms.clone();
    for row in &mut withdrawn {
        if matches!(
            row,
            Proposition::LessThan(..) | Proposition::LessOrEqual(_, _)
        ) && row != &Proposition::LessOrEqual(literal(0), value(14, signed()))
        {
            *row = Proposition::Truth;
        }
    }
    assert!(check_certificate(&context, &goal(), &[], &withdrawn, &proof).is_err());
    assert!(prove(&context, &roster(false)).is_none());
}
