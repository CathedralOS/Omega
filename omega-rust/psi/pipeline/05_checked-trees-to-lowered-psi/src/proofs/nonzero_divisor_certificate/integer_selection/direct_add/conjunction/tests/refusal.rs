use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm};

use super::super::SearchBudget;
use super::fixture::{fork_join, literal, value};

#[test]
fn cycles_type_drift_address_carriers_and_overflow_are_refused() {
    let mut cycle = fork_join(IntegerSign::Signed, 8, false, false);
    cycle.axioms[4] = Proposition::Equal(
        cycle.left.clone(),
        ScalarTerm::exact_integer_add(
            cycle.integer_type,
            cycle.left.clone(),
            value(1, cycle.integer_type),
        )
        .expect("cyclic add"),
    );
    assert!(cycle.prove(SearchBudget::default()).proof.is_none());

    let fixture = fork_join(IntegerSign::Unsigned, 8, false, false);
    let wrong_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
    let definitions =
        crate::proofs::nonzero_divisor_certificate::affine_custody::DefinitionIndex::new(
            &fixture.context,
            &[],
            &fixture.axioms,
        );
    assert!(
        super::super::prove_with_budget(
            &fixture.context,
            &fixture.goal,
            wrong_type,
            &fixture.left,
            &fixture.right,
            &fixture.target,
            false,
            &[],
            &fixture.axioms,
            &definitions,
            SearchBudget::default(),
        )
        .proof
        .is_none()
    );

    let address = IntegerType::address(64).expect("64-bit address");
    assert!(
        super::super::prove_with_budget(
            &fixture.context,
            &fixture.goal,
            address,
            &fixture.left,
            &fixture.right,
            &fixture.target,
            false,
            &[],
            &fixture.axioms,
            &definitions,
            SearchBudget::default(),
        )
        .proof
        .is_none()
    );

    let mut overflow = fork_join(IntegerSign::Unsigned, 8, false, false);
    overflow.axioms[0] = Proposition::Equal(
        value(1, overflow.integer_type),
        literal(overflow.integer_type, IntegerValue::Unsigned(250)),
    );
    overflow.axioms[2] = Proposition::Equal(
        value(3, overflow.integer_type),
        literal(overflow.integer_type, IntegerValue::Unsigned(10)),
    );
    assert!(overflow.prove(SearchBudget::default()).proof.is_none());
}
