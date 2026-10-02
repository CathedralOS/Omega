//! Apply conditional premises through the kernel's implication rule.
//!
//! A producer must prove the antecedent in the unchanged input scope. It never
//! installs a proposed consequence as an axiom. Searching only implications
//! whose consequent contains the requested goal bounds irrelevant exploration;
//! recursive antecedents share the enclosing search's depth and step budgets.

use crate::{ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

pub(super) fn prove(
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    for (premises, semantic) in [(requirements, false), (semantic_axioms, true)] {
        for (index, premise) in premises.iter().enumerate() {
            let premise = ProofNode {
                conclusion: premise.clone(),
                rule: if semantic {
                    ProofRule::SemanticAxiom { index }
                } else {
                    ProofRule::Assumption { index }
                },
            };
            if let Some(proof) = from_premise(
                goal,
                premise,
                requirements,
                semantic_axioms,
                remaining,
                depth,
            ) {
                return Some(proof);
            }
        }
    }
    None
}

fn from_premise(
    goal: &Proposition,
    premise: ProofNode,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    remaining: &mut usize,
    depth: usize,
) -> Option<ProofNode> {
    super::step(remaining, depth)?;
    match &premise.conclusion {
        Proposition::Conjunction(children) => {
            for (conjunct, child) in children.iter().enumerate() {
                let child = ProofNode {
                    conclusion: child.clone(),
                    rule: ProofRule::ConjunctionElimination {
                        conjunction: Box::new(premise.clone()),
                        conjunct,
                    },
                };
                if let Some(proof) = from_premise(
                    goal,
                    child,
                    requirements,
                    semantic_axioms,
                    remaining,
                    depth + 1,
                ) {
                    return Some(proof);
                }
            }
            None
        }
        Proposition::Implication {
            premise: antecedent,
            conclusion,
        } => {
            let mut path = Vec::new();
            if antecedent.as_ref() == goal
                || !super::projection(conclusion, goal, &mut path, remaining, depth + 1)?
            {
                return None;
            }
            let antecedent = super::prove(
                antecedent,
                requirements,
                semantic_axioms,
                remaining,
                depth + 1,
            )?;
            let mut proof = ProofNode {
                conclusion: conclusion.as_ref().clone(),
                rule: ProofRule::ImplicationElimination {
                    implication: Box::new(premise.clone()),
                    premise: Box::new(antecedent),
                },
            };
            for conjunct in path {
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
        _ => None,
    }
}

#[cfg(test)]
mod tests;
