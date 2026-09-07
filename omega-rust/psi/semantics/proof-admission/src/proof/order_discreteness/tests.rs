use super::*;
use crate::{ProofNode, ProofRule, check_certificate};
use semantic_vocabulary::{IntegerSign, IntegerType, PropositionContext, ScalarType, ValueId};

#[test]
fn both_literal_endpoints_require_the_exact_adjacent_integer_and_cited_bound() {
    for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
        let integer_type = IntegerType::new(sign, 8).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let identity = ValueId::new(1).unwrap();
        let value = ScalarTerm::value(identity, scalar_type);
        let context = PropositionContext::from_value_types([(identity, scalar_type)]).unwrap();
        let literal = |number| {
            ScalarTerm::integer(
                integer_type,
                match sign {
                    IntegerSign::Signed => IntegerValue::Signed(number),
                    IntegerSign::Unsigned => IntegerValue::Unsigned(number as u128),
                },
            )
            .unwrap()
        };
        for lower in [false, true] {
            let premise = if lower {
                Proposition::LessOrEqual(literal(1), value.clone())
            } else {
                Proposition::LessOrEqual(value.clone(), literal(1))
            };
            let goal = if lower {
                Proposition::LessThan(literal(0), value.clone())
            } else {
                Proposition::LessThan(value.clone(), literal(2))
            };
            let proof = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(ProofNode {
                        conclusion: premise.clone(),
                        rule: ProofRule::SemanticAxiom { index: 0 },
                    }),
                },
            };
            check_certificate(&context, &goal, &[], std::slice::from_ref(&premise), &proof)
                .unwrap();
            assert!(check_certificate(&context, &goal, &[], &[], &proof).is_err());
            for wrong in [
                Proposition::LessThan(value.clone(), value.clone()),
                Proposition::LessThan(literal(1), value.clone()),
                Proposition::LessThan(value.clone(), literal(1)),
                premise.clone(),
            ] {
                let mut altered = proof.clone();
                altered.conclusion = wrong.clone();
                assert!(
                    check_certificate(
                        &context,
                        &wrong,
                        &[],
                        std::slice::from_ref(&premise),
                        &altered
                    )
                    .is_err()
                );
            }
        }
    }
}

#[test]
fn adjacency_never_wraps_at_carrier_or_host_limits() {
    for bits in [8, 128] {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let integer_type = IntegerType::new(sign, bits).unwrap();
            let minimum = ScalarTerm::integer(integer_type, integer_type.minimum_value()).unwrap();
            let maximum = ScalarTerm::integer(integer_type, integer_type.maximum_value()).unwrap();
            assert!(adjacent(&minimum, false).is_none());
            assert!(adjacent(&maximum, true).is_none());
            assert!(adjacent(&minimum, true).is_some());
            assert!(adjacent(&maximum, false).is_some());
        }
    }
    let address =
        ScalarTerm::integer(IntegerType::address(64).unwrap(), IntegerValue::Unsigned(1)).unwrap();
    assert!(adjacent(&address, false).is_none());
    assert!(adjacent(&address, true).is_none());
}

#[test]
fn signed_negative_bounds_preserve_sign_and_other_endpoint() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let literal = |number| ScalarTerm::integer(integer_type, IntegerValue::Signed(number)).unwrap();
    let value = ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(integer_type));
    let premise = Proposition::LessOrEqual(literal(-1), value.clone());
    check(&premise, &Proposition::LessThan(literal(-2), value.clone())).unwrap();
    assert!(check(&premise, &Proposition::LessThan(literal(0), value)).is_err());
}
