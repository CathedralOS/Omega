use super::super::super::super::affine_custody::DefinitionIndex;
use super::{Proposition, ScalarTerm};
use proof_admission::{AcceptedProofRule, ProofNode, accept_certificate, check_certificate};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, PropositionContext, ScalarType, ValueId,
};

fn prove(assumptions: &[Proposition], axioms: &[Proposition]) -> Option<ProofNode> {
    super::prove(
        &context(),
        assumptions,
        axioms,
        &mut DefinitionIndex::new(&context(), assumptions, axioms),
    )
}

fn integer_type() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 32).unwrap()
}

fn literal(value: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type(), IntegerValue::Unsigned(value)).unwrap()
}

fn value(identity: u64) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer_type()),
    )
}

fn context() -> PropositionContext {
    PropositionContext::from_value_types((1..=4).map(|identity| {
        (
            ValueId::new(identity).unwrap(),
            ScalarType::Integer(integer_type()),
        )
    }))
    .unwrap()
}

/// A closed arithmetic endpoint evaluates through the checked closed-relation
/// primitive, so `value == 2 + 1` contradicts `value < 3` by literal `3 < 3`.
#[test]
fn closed_arithmetic_endpoints_reach_literal_contradictions() {
    let sum = ScalarTerm::wrapping_integer_add(integer_type(), literal(2), literal(1)).unwrap();
    let assumptions = [Proposition::Equal(value(1), sum)];
    let axioms = [Proposition::LessThan(value(1), literal(3))];
    let proof = prove(&assumptions, &axioms).expect("evaluated bound leg");
    let acceptance = accept_certificate(
        &context(),
        &Proposition::Falsehood,
        &assumptions,
        &axioms,
        &proof,
    )
    .unwrap();
    assert!(
        acceptance
            .rules
            .contains(&AcceptedProofRule::IntegerOrderSubstitution)
    );
    assert!(
        acceptance
            .rules
            .contains(&AcceptedProofRule::PredicateDenotation)
    );
    // Open operands keep their exact identity: no closed denotation exists for
    // `value == v2 + 1` even alongside a strict bound on the same value.
    let open = ScalarTerm::wrapping_integer_add(integer_type(), value(2), literal(1)).unwrap();
    assert!(
        prove(
            &[Proposition::Equal(value(1), open)],
            std::slice::from_ref(&axioms[0])
        )
        .is_none(),
        "an open sum is not a literal bound"
    );
}

#[test]
fn integer_contradiction_search_rejects_consistent_or_unmatched_bounds() {
    let lower = Proposition::LessOrEqual(literal(3), value(1));
    let other_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    for upper in [
        Proposition::LessThan(value(1), literal(4)),
        Proposition::LessThan(value(2), literal(3)),
        Proposition::LessThan(
            value(1),
            ScalarTerm::integer(other_type, IntegerValue::Unsigned(3)).unwrap(),
        ),
        Proposition::LessThan(
            ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(other_type)),
            literal(3),
        ),
    ] {
        assert!(prove(std::slice::from_ref(&lower), &[upper]).is_none());
    }
    assert!(prove(&[], &[]).is_none());
}

#[test]
fn directly_cited_closed_integer_falsehood_is_not_an_unconditional_primitive() {
    for proposition in [
        Proposition::Equal(literal(3), literal(4)),
        Proposition::LessThan(literal(3), literal(3)),
        Proposition::LessOrEqual(literal(5), literal(4)),
    ] {
        let premises = [proposition];
        let proof = prove(&premises, &[]).unwrap();
        let acceptance =
            accept_certificate(&context(), &Proposition::Falsehood, &premises, &[], &proof)
                .unwrap();
        assert_eq!(acceptance.assumptions.len(), 1);
        assert!(check_certificate(&context(), &Proposition::Falsehood, &[], &[], &proof).is_err());
    }
}
