use super::super::Elaboration;
use crate::{
    Budget, IntegerCastChainWitness, PrimitiveJudgment, ProofNode, ProofRule,
    verify_bounded_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};
use std::collections::BTreeSet;

fn integer(sign: IntegerSign, bits: u16) -> IntegerType {
    IntegerType::new(sign, bits).expect("fixed integer type")
}

fn value(id: u64, integer: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(id).expect("value id"),
        ScalarType::Integer(integer),
    )
}

fn context_for(roots: &[(u64, IntegerType)]) -> PropositionContext {
    PropositionContext::from_value_types(
        roots
            .iter()
            .map(|(id, ty)| (ValueId::new(*id).unwrap(), ScalarType::Integer(*ty))),
    )
    .expect("context")
}

fn denotation_counts(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    axioms: &[Proposition],
    proof: &ProofNode,
) -> (usize, usize) {
    let parameters = BTreeSet::new();
    let mut elaboration =
        Elaboration::new(context, goal, assumptions, axioms, &parameters).unwrap();
    elaboration.node(proof).unwrap();
    (
        elaboration.denotation.rule_axioms.len(),
        elaboration.denotation.cast_identities.len(),
    )
}

#[test]
fn cast_bound_transports_through_one_exact_cast_edge() {
    let i16 = integer(IntegerSign::Signed, 16);
    let i8 = integer(IntegerSign::Signed, 8);
    let root = value(1, i16);
    let target = value(2, i8);
    let context = context_for(&[(1, i16), (2, i8)]);
    let root_bound = Proposition::LessOrEqual(
        ScalarTerm::integer(i16, IntegerValue::Signed(1)).unwrap(),
        root.clone(),
    );
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_exact_cast(i16, i8, root.clone()).unwrap(),
    );
    let conclusion = Proposition::LessOrEqual(
        ScalarTerm::integer(i8, IntegerValue::Signed(1)).unwrap(),
        target.clone(),
    );
    let proof = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::IntegerCastBound {
            root_bound: Box::new(ProofNode {
                conclusion: root_bound.clone(),
                rule: ProofRule::Assumption { index: 0 },
            }),
            witness: IntegerCastChainWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms: vec![0],
            },
        },
    };
    verify_bounded_certificate(
        &context,
        &conclusion,
        std::slice::from_ref(&root_bound),
        std::slice::from_ref(&definition),
        &proof,
        &mut Budget::default(),
    )
    .expect("the transported bound verifies in the kernel");
    let (rule_axioms, cast_identities) = denotation_counts(
        &context,
        &conclusion,
        std::slice::from_ref(&root_bound),
        std::slice::from_ref(&definition),
        &proof,
    );
    assert_eq!(rule_axioms, 0);
    assert_eq!(cast_identities, 1);
}

#[test]
fn cast_bound_walks_a_multi_edge_word_in_either_direction() {
    let i16 = integer(IntegerSign::Signed, 16);
    let u16 = integer(IntegerSign::Unsigned, 16);
    let i8 = integer(IntegerSign::Signed, 8);
    let root = value(1, i16);
    let middle = value(3, u16);
    let target = value(2, i8);
    let context = context_for(&[(1, i16), (2, i8), (3, u16)]);
    let first_definition = Proposition::Equal(
        middle.clone(),
        ScalarTerm::integer_exact_cast(i16, u16, root.clone()).unwrap(),
    );
    let second_definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_exact_cast(u16, i8, middle.clone()).unwrap(),
    );
    let axioms = [first_definition, second_definition];
    // Both orientations: `7 ≤ root` transports to `7 ≤ target`, and
    // `root ≤ 42` transports to `target ≤ 42`.
    for root_is_left in [false, true] {
        let literal = |ty, v| ScalarTerm::integer(ty, IntegerValue::Signed(v)).unwrap();
        let root_bound = if root_is_left {
            Proposition::LessOrEqual(root.clone(), literal(i16, 42))
        } else {
            Proposition::LessOrEqual(literal(i16, 7), root.clone())
        };
        let conclusion = if root_is_left {
            Proposition::LessOrEqual(target.clone(), literal(i8, 42))
        } else {
            Proposition::LessOrEqual(literal(i8, 7), target.clone())
        };
        let proof = ProofNode {
            conclusion: conclusion.clone(),
            rule: ProofRule::IntegerCastBound {
                root_bound: Box::new(ProofNode {
                    conclusion: root_bound.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                witness: IntegerCastChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms: vec![0, 1],
                },
            },
        };
        verify_bounded_certificate(
            &context,
            &conclusion,
            std::slice::from_ref(&root_bound),
            &axioms,
            &proof,
            &mut Budget::default(),
        )
        .expect("the chained transport verifies in the kernel");
        let (rule_axioms, cast_identities) = denotation_counts(
            &context,
            &conclusion,
            std::slice::from_ref(&root_bound),
            &axioms,
            &proof,
        );
        assert_eq!(rule_axioms, 0);
        assert_eq!(cast_identities, 2);
    }
}

#[test]
fn truth_root_cast_bound_uses_the_tightest_carrier_membership() {
    let u8 = integer(IntegerSign::Unsigned, 8);
    let u64 = integer(IntegerSign::Unsigned, 64);
    let root = value(1, u8);
    let target = value(2, u64);
    let context = context_for(&[(1, u8), (2, u64)]);
    let definition = Proposition::Equal(
        target.clone(),
        ScalarTerm::integer_widen(u8, u64, root.clone()).unwrap(),
    );
    // u8 → u64 widening: the surviving interval is [0, 255] — both
    // conclusions originate at the root's carrier membership bounds and
    // transport through the exact-cast identity.
    for (bound, maximum) in [(true, 0u128), (false, 255)] {
        let endpoint = ScalarTerm::integer(u64, IntegerValue::Unsigned(maximum)).unwrap();
        let conclusion = if bound {
            Proposition::LessOrEqual(endpoint, target.clone())
        } else {
            Proposition::LessOrEqual(target.clone(), endpoint)
        };
        let proof = ProofNode {
            conclusion: conclusion.clone(),
            rule: ProofRule::IntegerCastBound {
                root_bound: Box::new(ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                }),
                witness: IntegerCastChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms: vec![0],
                },
            },
        };
        verify_bounded_certificate(
            &context,
            &conclusion,
            &[],
            std::slice::from_ref(&definition),
            &proof,
            &mut Budget::default(),
        )
        .expect("the carrier-membership bound transports to the target");
        let (rule_axioms, cast_identities) = denotation_counts(
            &context,
            &conclusion,
            &[],
            std::slice::from_ref(&definition),
            &proof,
        );
        assert_eq!(rule_axioms, 0);
        assert_eq!(cast_identities, 1);
    }
}
