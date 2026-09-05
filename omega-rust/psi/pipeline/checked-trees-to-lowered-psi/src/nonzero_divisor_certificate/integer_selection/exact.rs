//! Exact retained proposition proof custody.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

use super::super::integer_evidence::cited_facts;

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    cited_facts(assumptions, semantic_axioms)
        .find(|(_, fact)| *fact == goal)
        .map(|(citation, fact)| citation.proof(fact))
        .or_else(|| equality_chain(goal, assumptions, semantic_axioms))
}

/// Follow explicitly cited equalities, proving every reversed edge. A machine
/// result aliases its returned value, whose defining operation supplies the
/// literal equation. No missing equation or implicit symmetry is assumed.
fn equality_chain(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::Equal(left, right) = goal else {
        return None;
    };
    if left == right {
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Primitive(PrimitiveJudgment::ReflexiveEquality),
        });
    }
    let mut facts = Vec::new();
    let mut pending_facts = cited_facts(assumptions, semantic_axioms)
        .map(|(citation, fact)| citation.proof(fact))
        .collect::<Vec<_>>();
    while let Some(proof) = pending_facts.pop() {
        match &proof.conclusion {
            Proposition::Conjunction(conjuncts) => {
                for (conjunct, conclusion) in conjuncts.iter().enumerate() {
                    pending_facts.push(ProofNode {
                        conclusion: conclusion.clone(),
                        rule: ProofRule::ConjunctionElimination {
                            conjunction: Box::new(proof.clone()),
                            conjunct,
                        },
                    });
                }
            }
            Proposition::Equal(_, _) => facts.push(proof),
            _ => {}
        }
    }
    let mut pending = vec![(left.clone(), None::<ProofNode>)];
    let mut index = 0;
    while index < pending.len() {
        let (current, prefix) = pending[index].clone();
        index += 1;
        for next in &facts {
            let Proposition::Equal(source, destination) = &next.conclusion else {
                continue;
            };
            let (destination, next) = if source == &current {
                (destination, next.clone())
            } else if destination == &current {
                (
                    source,
                    ProofNode {
                        conclusion: Proposition::Equal(destination.clone(), source.clone()),
                        rule: ProofRule::EqualitySymmetry {
                            equality: Box::new(next.clone()),
                        },
                    },
                )
            } else {
                continue;
            };
            if pending.iter().any(|(value, _)| value == destination) {
                continue;
            }
            let proof = if let Some(prefix) = prefix.clone() {
                ProofNode {
                    conclusion: Proposition::Equal(left.clone(), destination.clone()),
                    rule: ProofRule::EqualityTransitivity {
                        left_equals_middle: Box::new(prefix),
                        middle_equals_right: Box::new(next.clone()),
                    },
                }
            } else {
                next
            };
            if destination == right {
                return Some(proof);
            }
            pending.push((destination.clone(), Some(proof)));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerSign, IntegerType, PropositionContext, ScalarTerm, ScalarType, ValueId,
    };

    fn context() -> PropositionContext {
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
        PropositionContext::from_value_types(
            (1..=4).map(|identity| (ValueId::new(identity).unwrap(), scalar_type)),
        )
        .unwrap()
    }

    fn equality(left: u64, right: u64) -> Proposition {
        let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 16).unwrap());
        Proposition::Equal(
            ScalarTerm::value(ValueId::new(left).unwrap(), scalar_type),
            ScalarTerm::value(ValueId::new(right).unwrap(), scalar_type),
        )
    }

    #[test]
    fn directed_equality_chain_replays_each_exact_citation() {
        let goal = equality(1, 4);
        let assumptions = [Proposition::Conjunction(vec![
            equality(2, 3),
            equality(3, 4),
        ])];
        let semantic_axioms = [equality(1, 2)];
        let proof = prove(&goal, &assumptions, &semantic_axioms).expect("three cited equalities");
        assert!(matches!(proof.rule, ProofRule::EqualityTransitivity { .. }));
        check_certificate(&context(), &goal, &assumptions, &semantic_axioms, &proof)
            .expect("kernel replays the directed chain");

        assert!(check_certificate(&context(), &goal, &assumptions, &[], &proof).is_err());
        assert!(
            check_certificate(
                &context(),
                &goal,
                &[equality(2, 4)],
                &semantic_axioms,
                &proof
            )
            .is_err(),
            "substituted hypothesis shape cannot satisfy the existing citation"
        );
    }

    #[test]
    fn missing_equality_edges_are_not_invented() {
        let goal = equality(1, 4);
        for semantic_axioms in [
            vec![equality(1, 2), equality(3, 4)],
            vec![equality(2, 1), equality(3, 4)],
            vec![equality(1, 2), equality(3, 2)],
        ] {
            assert!(
                prove(&goal, &[], &semantic_axioms).is_none(),
                "{semantic_axioms:?}"
            );
        }
    }

    #[test]
    fn reversed_edges_have_explicit_symmetry_certificates() {
        let goal = equality(1, 4);
        let axioms = [equality(2, 1), equality(2, 3), equality(4, 3)];
        let proof = prove(&goal, &[], &axioms).expect("two reversed edges");
        check_certificate(&context(), &goal, &[], &axioms, &proof).unwrap();
        let mut changed = axioms.clone();
        changed[0] = equality(1, 2);
        assert!(
            check_certificate(&context(), &goal, &[], &changed, &proof).is_err(),
            "a symmetric proposition does not replace the cited proof node"
        );
    }

    #[test]
    fn equality_cycles_terminate_without_hiding_a_reachable_target() {
        let goal = equality(1, 4);
        let mut semantic_axioms = vec![
            equality(1, 2),
            equality(2, 1),
            equality(2, 3),
            equality(3, 2),
        ];
        assert!(
            prove(&goal, &[], &semantic_axioms).is_none(),
            "cycle is not a missing exit"
        );
        semantic_axioms.push(equality(3, 4));
        let proof = prove(&goal, &[], &semantic_axioms).expect("reachable target after cycle");
        check_certificate(&context(), &goal, &[], &semantic_axioms, &proof).unwrap();
    }

    #[test]
    fn reflexivity_needs_no_citation_and_non_equality_is_not_a_chain() {
        let goal = equality(1, 1);
        let proof = prove(&goal, &[], &[]).expect("reflexivity");
        check_certificate(&context(), &goal, &[], &[], &proof).unwrap();
        let Proposition::Equal(left, right) = equality(1, 4) else {
            unreachable!()
        };
        assert!(
            prove(
                &Proposition::LessOrEqual(left, right),
                &[],
                &[equality(1, 4)]
            )
            .is_none()
        );
    }
}
