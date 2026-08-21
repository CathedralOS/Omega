//! Focused certificates for the canonical fixed-integer nonzero-divisor goal.
//!
//! This producer deliberately consumes only machine requirements and facts
//! reconstructed before the operation site. It never sees the operation's own
//! result equation, so the certificate cannot justify the operation with a
//! fact produced by that same operation.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule, check_certificate};

#[derive(Clone, Copy)]
enum Citation {
    Assumption(usize),
    SemanticAxiom(usize),
}

pub(super) fn prove_nonzero_divisor(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let proof = match goal {
        Proposition::LessOrEqual(_, _) => prove_integer_bound(goal, assumptions, semantic_axioms)?,
        Proposition::Disjunction(disjuncts) if disjuncts.len() == 2 => {
            let (index, disjunct) =
                disjuncts.iter().enumerate().find_map(|(index, disjunct)| {
                    prove_integer_bound(disjunct, assumptions, semantic_axioms)
                        .map(|proof| (index, proof))
                })?;
            ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::DisjunctionIntroduction {
                    disjunct: Box::new(disjunct),
                    index,
                },
            }
        }
        _ => return None,
    };
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}

fn prove_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };

    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        if fact == goal {
            return Some(citation.proof(fact));
        }
    }

    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(fact_left, fact_right) = fact else {
            continue;
        };
        if fact_left == goal_left {
            let tail = Proposition::LessOrEqual(fact_right.clone(), goal_right.clone());
            if let Some(tail) = closed_integer_relation(tail) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(citation.proof(fact)),
                        middle_less_or_equal_right: Box::new(tail),
                    },
                });
            }
        }
        if fact_right == goal_right {
            let head = Proposition::LessOrEqual(goal_left.clone(), fact_left.clone());
            if let Some(head) = closed_integer_relation(head) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(head),
                        middle_less_or_equal_right: Box::new(citation.proof(fact)),
                    },
                });
            }
        }
    }

    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = fact else {
            continue;
        };
        for (old, replacement) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            if replacement.integer_value().is_none() {
                continue;
            }
            let (endpoint, relation) = if old == goal_left {
                (
                    0,
                    Proposition::LessOrEqual(replacement.clone(), goal_right.clone()),
                )
            } else if old == goal_right {
                (
                    1,
                    Proposition::LessOrEqual(goal_left.clone(), replacement.clone()),
                )
            } else {
                continue;
            };
            if let Some(relation) = closed_integer_relation(relation) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(relation),
                        equality: Box::new(citation.proof(fact)),
                        endpoint,
                    },
                });
            }
        }
    }
    None
}

fn cited_facts<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition)> {
    assumptions
        .iter()
        .enumerate()
        .map(|(index, fact)| (Citation::Assumption(index), fact))
        .chain(
            semantic_axioms
                .iter()
                .enumerate()
                .map(|(index, fact)| (Citation::SemanticAxiom(index), fact)),
        )
}

fn closed_integer_relation(conclusion: Proposition) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = &conclusion else {
        return None;
    };
    let (left_type, left) = left.integer_value()?;
    let (right_type, right) = right.integer_value()?;
    (left_type == right_type
        && left_type
            .compare(left, right)
            .is_some_and(|order| !order.is_gt()))
    .then_some(ProofNode {
        conclusion,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    })
}

impl Citation {
    fn proof(self, conclusion: &Proposition) -> ProofNode {
        ProofNode {
            conclusion: conclusion.clone(),
            rule: match self {
                Self::Assumption(index) => ProofRule::Assumption { index },
                Self::SemanticAxiom(index) => ProofRule::SemanticAxiom { index },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::{IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId};

    fn value(id: u64, integer_type: IntegerType) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(id).expect("value id"),
            ScalarType::Integer(integer_type),
        )
    }

    fn integer(integer_type: IntegerType, value: i128) -> ScalarTerm {
        ScalarTerm::integer(integer_type, IntegerValue::Signed(value)).expect("integer")
    }

    #[test]
    fn signed_goal_prefers_negative_arm_and_tightens_requirement() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = value(1, integer_type);
        let negative_one = integer(integer_type, -1);
        let goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), negative_one),
            Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone()),
        ]);
        let requirements = [Proposition::LessOrEqual(divisor, integer(integer_type, -2))];
        let proof = prove_nonzero_divisor(
            &PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type),
            )])
            .unwrap(),
            &goal,
            &requirements,
            &[],
        )
        .expect("negative bound proves nonzero");
        assert!(matches!(
            proof.rule,
            ProofRule::DisjunctionIntroduction { index: 0, .. }
        ));
    }

    #[test]
    fn signed_goal_selects_positive_arm_from_exact_requirement() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = value(1, integer_type);
        let positive = Proposition::LessOrEqual(integer(integer_type, 1), divisor.clone());
        let goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(divisor, integer(integer_type, -1)),
            positive.clone(),
        ]);
        let proof = prove_nonzero_divisor(
            &PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type),
            )])
            .unwrap(),
            &goal,
            &[positive],
            &[],
        )
        .expect("positive requirement proves nonzero");
        let ProofRule::DisjunctionIntroduction { disjunct, index } = proof.rule else {
            panic!("signed goal uses disjunction introduction")
        };
        assert_eq!(index, 1);
        assert!(matches!(disjunct.rule, ProofRule::Assumption { index: 0 }));
    }

    #[test]
    fn literal_equality_substitution_uses_only_prior_fact() {
        let integer_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let divisor = value(1, integer_type);
        let goal = Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1)).unwrap(),
            divisor.clone(),
        );
        let facts = [Proposition::Equal(
            divisor,
            ScalarTerm::integer(integer_type, IntegerValue::Unsigned(5)).unwrap(),
        )];
        let proof = prove_nonzero_divisor(
            &PropositionContext::from_value_types([(
                ValueId::new(1).unwrap(),
                ScalarType::Integer(integer_type),
            )])
            .unwrap(),
            &goal,
            &[],
            &facts,
        )
        .expect("literal equality proves nonzero");
        assert!(matches!(
            proof.rule,
            ProofRule::IntegerLessOrEqualSubstitution { endpoint: 1, .. }
        ));
        assert!(
            prove_nonzero_divisor(
                &PropositionContext::from_value_types([(
                    ValueId::new(1).unwrap(),
                    ScalarType::Integer(integer_type),
                )])
                .unwrap(),
                &goal,
                &[],
                &[],
            )
            .is_none()
        );
    }
}
