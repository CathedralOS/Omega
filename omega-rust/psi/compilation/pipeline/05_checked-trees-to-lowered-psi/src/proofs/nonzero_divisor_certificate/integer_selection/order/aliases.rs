//! Explicit conjunction projection and equality transport for integer bounds.

use crate::proofs::nonzero_divisor_certificate::integer_evidence::literal_value;
use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts, closed_integer_relation};
use super::super::exact;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    // One roster session serves every endpoint substitution below: the loop
    // asks hundreds of equality questions against this unchanged scope, so
    // the cited-fact index is resolved once rather than per question.
    let session = exact::session(context, assumptions, semantic_axioms);
    // Walk cited facts and their conjunction leaves in the order a stack of
    // their proofs pops them, but build a leaf's proof only once its
    // relation completes: completion depends on the relation alone.
    let mut pending = cited_facts(assumptions, semantic_axioms)
        .map(|(citation, fact)| (citation, fact, Vec::new(), fact))
        .collect::<Vec<_>>();
    while let Some((citation, root, projection, fact)) = pending.pop() {
        let leaf = || projected_proof(citation, root, &projection);
        match fact {
            Proposition::Conjunction(conjuncts) => {
                for (conjunct, conclusion) in conjuncts.iter().enumerate() {
                    let mut child = projection.clone();
                    child.push(conjunct);
                    pending.push((citation, root, child, conclusion));
                }
            }
            Proposition::LessOrEqual(_, _) => {
                if let Some(proof) = complete(&session, goal, fact, leaf) {
                    return Some(proof);
                }
            }
            Proposition::LessThan(left, right) => {
                let weakened = Proposition::LessOrEqual(left.clone(), right.clone());
                if let Some(proof) = complete(&session, goal, &weakened, || ProofNode {
                    conclusion: weakened.clone(),
                    rule: ProofRule::IntegerOrderWeakening {
                        relation: Box::new(leaf()),
                    },
                }) {
                    return Some(proof);
                }
            }
            Proposition::Equal(left, right) => {
                // A closed endpoint is useful only after a separately cited
                // equality chain connects it to this obligation's subject.
                for literal in [left, right]
                    .into_iter()
                    .filter(|term| literal_value(term).is_some())
                {
                    for relation in [
                        Proposition::LessOrEqual(literal.clone(), goal_right.clone()),
                        Proposition::LessOrEqual(goal_left.clone(), literal.clone()),
                    ] {
                        if let Some(closed) = closed_integer_relation(relation) {
                            let conclusion = closed.conclusion.clone();
                            if let Some(proof) = complete(&session, goal, &conclusion, || closed) {
                                return Some(proof);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// The cited proof of a conjunction leaf: the citation, then one
/// elimination per projection step.
fn projected_proof(citation: Citation, root: &Proposition, projection: &[usize]) -> ProofNode {
    let mut proof = citation.proof(root);
    for &conjunct in projection {
        let Proposition::Conjunction(conjuncts) = &proof.conclusion else {
            unreachable!("projection follows cited conjunction children")
        };
        proof = ProofNode {
            conclusion: conjuncts[conjunct].clone(),
            rule: ProofRule::ConjunctionElimination {
                conjunction: Box::new(proof),
                conjunct,
            },
        };
    }
    proof
}

/// How one relation endpoint reaches the goal's endpoint.
enum EndpointStep {
    Unchanged,
    Equality(ProofNode),
    Bridge(ProofNode),
}

/// Complete `relation` into `goal` by replacing each endpoint in turn. The
/// steps are decided on the relation's proposition first; its proof is built
/// only when both endpoints reach the goal.
fn complete(
    session: &exact::Session,
    goal: &Proposition,
    relation: &Proposition,
    relation_proof: impl FnOnce() -> ProofNode,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let Proposition::LessOrEqual(left, right) = relation else {
        return None;
    };
    let left_step = endpoint_step(session, left, 0, goal_left)?;
    let right_step = endpoint_step(session, right, 1, goal_right)?;
    let mut proof = relation_proof();
    for (endpoint, target, step) in [(0, goal_left, left_step), (1, goal_right, right_step)] {
        proof = replace_endpoint(proof, endpoint, target, step);
    }
    (proof.conclusion == *goal).then_some(proof)
}

fn endpoint_step(
    session: &exact::Session,
    old: &ScalarTerm,
    endpoint: usize,
    target: &ScalarTerm,
) -> Option<EndpointStep> {
    if old == target {
        return Some(EndpointStep::Unchanged);
    }
    if let Some(equality) = session.prove_equality(target, old) {
        return Some(EndpointStep::Equality(equality));
    }
    let bridge = if endpoint == 0 {
        Proposition::LessOrEqual(target.clone(), old.clone())
    } else {
        Proposition::LessOrEqual(old.clone(), target.clone())
    };
    closed_integer_relation(bridge).map(EndpointStep::Bridge)
}

fn replace_endpoint(
    relation: ProofNode,
    endpoint: usize,
    target: &ScalarTerm,
    step: EndpointStep,
) -> ProofNode {
    let Proposition::LessOrEqual(left, right) = &relation.conclusion else {
        unreachable!("endpoint steps are planned on an order relation")
    };
    let conclusion = if endpoint == 0 {
        Proposition::LessOrEqual(target.clone(), right.clone())
    } else {
        Proposition::LessOrEqual(left.clone(), target.clone())
    };
    match step {
        EndpointStep::Unchanged => relation,
        EndpointStep::Equality(equality) => ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(relation),
                equality: Box::new(equality),
                endpoint,
            },
        },
        EndpointStep::Bridge(bridge) => {
            let (left, right) = if endpoint == 0 {
                (bridge, relation)
            } else {
                (relation, bridge)
            };
            ProofNode {
                conclusion,
                rule: ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(left),
                    middle_less_or_equal_right: Box::new(right),
                },
            }
        }
    }
}
