use super::{integer, value};
use crate::proofs::nonzero_divisor_certificate::{
    Proposition, PropositionContext, prove_canonical_integer_proposition,
};
use proof_admission::ProofRule;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};

#[test]
fn i1_exact_division_goal_nests_two_citation_transport_for_both_endpoints() {
    let i1 = IntegerType::new(IntegerSign::Signed, 1).expect("i1");
    let context = PropositionContext::from_value_types([
        (ValueId::new(1).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(2).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(3).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(4).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(5).unwrap(), ScalarType::Integer(i1)),
        (ValueId::new(6).unwrap(), ScalarType::Integer(i1)),
    ])
    .expect("six i1 values");
    let goal = Proposition::Conjunction(vec![
        Proposition::LessOrEqual(value(2, i1), integer(i1, -1)),
        Proposition::LessOrEqual(integer(i1, 0), value(1, i1)),
    ]);
    let assumptions = [
        Proposition::LessOrEqual(value(3, i1), value(4, i1)),
        Proposition::LessOrEqual(value(4, i1), integer(i1, -1)),
        Proposition::Equal(value(3, i1), value(2, i1)),
        Proposition::LessOrEqual(integer(i1, 0), value(6, i1)),
        Proposition::LessOrEqual(value(6, i1), value(5, i1)),
        Proposition::Equal(value(5, i1), value(1, i1)),
    ];
    let proof = prove_canonical_integer_proposition(&context, &goal, &assumptions, &[])
        .expect("two cited bounds transport to both i1 operands");
    let ProofRule::ConjunctionIntroduction(conjuncts) = proof.rule else {
        panic!("i1 two-citation transport proves both canonical conjuncts")
    };
    for (conjunct, endpoint, first, second, equality) in
        [(&conjuncts[0], 0, 0, 1, 2), (&conjuncts[1], 1, 3, 4, 5)]
    {
        let ProofRule::IntegerOrderSubstitution {
            relation,
            equality: equality_proof,
            endpoint: actual_endpoint,
        } = &conjunct.rule
        else {
            panic!("i1 bound uses endpoint substitution")
        };
        assert_eq!(*actual_endpoint, endpoint);
        let ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } = &relation.rule
        else {
            panic!("transported i1 bound uses two-citation transitivity")
        };
        assert!(matches!(
            left_less_or_equal_middle.rule,
            ProofRule::Assumption { index } if index == first
        ));
        assert!(matches!(
            middle_less_or_equal_right.rule,
            ProofRule::Assumption { index } if index == second
        ));
        assert!(matches!(
            equality_proof.rule,
            ProofRule::Assumption { index } if index == equality
        ));
    }

    assert!(
        prove_canonical_integer_proposition(
            &context,
            &goal,
            &[
                assumptions[0].clone(),
                assumptions[1].clone(),
                assumptions[2].clone(),
                assumptions[3].clone(),
                assumptions[5].clone(),
            ],
            &[],
        )
        .is_none(),
        "missing one middle bound cannot prove the complete i1 conjunction",
    );
}
