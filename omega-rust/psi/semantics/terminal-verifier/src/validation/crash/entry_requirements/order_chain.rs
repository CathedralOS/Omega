//! Search exact integer order paths and emit independently checked certificates.
//!
//! Short-circuit guards can establish an order through several comparisons.
//! Those comparisons remain separate ledger facts: this search neither adds a
//! transitive axiom nor changes the crash question. Only unconditional premises
//! and their conjuncts enter the graph; alternatives must never be combined.
//! Reachability distinguishes strict from nonstrict paths to the same endpoint.
//! Borrowed facts and predecessor coordinates keep search from cloning a proof
//! tree at every branch. Only the selected path becomes a certificate.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

use super::step;

struct OrderFact<'input> {
    root: &'input Proposition,
    relation: &'input Proposition,
    semantic: bool,
    premise_index: usize,
    conjuncts: Vec<usize>,
}

struct Reached<'input> {
    endpoint: &'input ScalarTerm,
    strict: bool,
    previous: Option<(usize, usize)>,
    length: usize,
}

pub(super) fn prove(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    let (start, end, strict_goal) = relation(goal)?;
    let mut facts = Vec::new();
    for (premises, semantic) in [(requirements, false), (semantic_axioms, true)] {
        for (premise_index, root) in premises.iter().enumerate() {
            collect(
                OrderFact {
                    root,
                    relation: root,
                    semantic,
                    premise_index,
                    conjuncts: Vec::new(),
                },
                &mut facts,
                remaining,
                depth,
            )?;
        }
    }
    let mut reached = vec![Reached {
        endpoint: start,
        strict: false,
        previous: None,
        length: 0,
    }];
    let mut cursor = 0;
    while cursor < reached.len() {
        let length = reached[cursor].length + 1;
        for (fact_index, fact) in facts.iter().enumerate() {
            step(remaining, depth + length)?;
            let (left, right, strict) = relation(fact.relation)?;
            if left != reached[cursor].endpoint {
                continue;
            }
            let strict = strict || reached[cursor].strict;
            let mut visited = false;
            for prior in &reached {
                step(remaining, depth + length)?;
                if prior.endpoint == right && prior.strict == strict {
                    visited = true;
                    break;
                }
            }
            if visited {
                continue;
            }
            reached.push(Reached {
                endpoint: right,
                strict,
                previous: Some((cursor, fact_index)),
                length,
            });
            if right == end && (!strict_goal || strict) {
                return certificate(goal, &facts, &reached, remaining, depth);
            }
        }
        cursor += 1;
    }
    None
}

fn collect<'input>(
    fact: OrderFact<'input>,
    facts: &mut Vec<OrderFact<'input>>,
    remaining: &mut usize,
    depth: usize,
) -> Option<()> {
    step(remaining, depth)?;
    if relation(fact.relation).is_some() {
        facts.push(fact);
    } else if let Proposition::Conjunction(children) = fact.relation {
        for (conjunct, child) in children.iter().enumerate() {
            let mut conjuncts = fact.conjuncts.clone();
            conjuncts.push(conjunct);
            collect(
                OrderFact {
                    relation: child,
                    conjuncts,
                    ..fact
                },
                facts,
                remaining,
                depth + 1,
            )?;
        }
    }
    Some(())
}

fn relation(proposition: &Proposition) -> Option<(&ScalarTerm, &ScalarTerm, bool)> {
    let (left, right, strict) = match proposition {
        Proposition::LessThan(left, right) => (left, right, true),
        Proposition::LessOrEqual(left, right) => (left, right, false),
        _ => return None,
    };
    (matches!(left.scalar_type(), ScalarType::Integer(_))
        && left.scalar_type() == right.scalar_type())
    .then_some((left, right, strict))
}

fn citation(fact: &OrderFact<'_>, remaining: &mut usize, depth: usize) -> Option<ProofNode> {
    step(remaining, depth)?;
    let mut proof = ProofNode {
        conclusion: fact.root.clone(),
        rule: if fact.semantic {
            ProofRule::SemanticAxiom {
                index: fact.premise_index,
            }
        } else {
            ProofRule::Assumption {
                index: fact.premise_index,
            }
        },
    };
    for (nesting, &conjunct) in fact.conjuncts.iter().enumerate() {
        step(remaining, depth + nesting + 1)?;
        let Proposition::Conjunction(children) = &proof.conclusion else {
            return None;
        };
        proof = ProofNode {
            conclusion: children.get(conjunct)?.clone(),
            rule: ProofRule::ConjunctionElimination {
                conjunction: Box::new(proof),
                conjunct,
            },
        };
    }
    Some(proof)
}

fn certificate(
    goal: &Proposition,
    facts: &[OrderFact<'_>],
    reached: &[Reached<'_>],
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    let mut path = Vec::new();
    let mut arrival = reached.last()?;
    while let Some((previous, fact)) = arrival.previous {
        step(remaining, depth + path.len())?;
        path.push(fact);
        arrival = reached.get(previous)?;
    }
    let mut path = path.into_iter().rev();
    let mut proof = citation(facts.get(path.next()?)?, remaining, depth)?;
    for (length, fact) in path.enumerate() {
        step(remaining, depth + length + 1)?;
        let next = citation(facts.get(fact)?, remaining, depth + length + 1)?;
        let (left, _, left_strict) = relation(&proof.conclusion)?;
        let (_, right, right_strict) = relation(&next.conclusion)?;
        let strict = left_strict || right_strict;
        proof = ProofNode {
            conclusion: if strict {
                Proposition::LessThan(left.clone(), right.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), right.clone())
            },
            rule: if strict {
                ProofRule::IntegerStrictOrderTransitivity {
                    left_to_middle: Box::new(proof),
                    middle_to_right: Box::new(next),
                }
            } else {
                ProofRule::IntegerLessOrEqualTransitivity {
                    left_less_or_equal_middle: Box::new(proof),
                    middle_less_or_equal_right: Box::new(next),
                }
            },
        };
    }
    if matches!(goal, Proposition::LessOrEqual(..)) && relation(&proof.conclusion)?.2 {
        step(remaining, depth)?;
        proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerOrderWeakening {
                relation: Box::new(proof),
            },
        };
    }
    (proof.conclusion == *goal).then_some(proof)
}

#[cfg(test)]
mod tests;
