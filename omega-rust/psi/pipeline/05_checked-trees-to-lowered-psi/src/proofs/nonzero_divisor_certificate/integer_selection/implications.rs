//! Bounded composition of independently cited implications.
//!
//! A proved consequence is a lemma for ordinary arithmetic and logical proof
//! construction, not only a fact whose syntax already matches the final goal.
//! Introduce it as one local assumption and discharge that assumption through
//! implication introduction/elimination. This preserves exact premise custody
//! without adding a kernel rule or teaching every arithmetic builder to search
//! conditional facts. The shared search budget and active implication roster
//! prevent cyclic laws from manufacturing their own premises.
//! Try already-provable premises before recursively deriving other premises.
//! Otherwise a stronger unavailable guard can spend the whole search budget on
//! permutations of weaker implications before a usable bound is considered.
//! This changes search order only; every selected law keeps its original citation.

use std::collections::BTreeMap;

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

use super::{case_analysis, logical};

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    ordinary: impl Fn(&Proposition, &[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    Search {
        semantic_axioms,
        ordinary,
        remaining: 4096,
        active: Vec::new(),
        proved: BTreeMap::new(),
    }
    .goal(goal, assumptions, 0, true)
}

struct Search<'input, Ordinary> {
    semantic_axioms: &'input [Proposition],
    ordinary: Ordinary,
    remaining: usize,
    active: Vec<Proposition>,
    /// `ordinary` answers one `(goal, assumptions)` question under this
    /// search's fixed context, axioms, and machine parameters, and every
    /// cached proof cites that same scope for the kernel to replay. The memo
    /// only skips re-derivation when the search revisits a scope — through
    /// implication elimination, a conjunction's own split, or a case branch.
    proved: BTreeMap<(Proposition, Vec<Proposition>), Option<ProofNode>>,
}

impl<Ordinary: Fn(&Proposition, &[Proposition]) -> Option<ProofNode>> Search<'_, Ordinary> {
    fn ordinary_proof(
        &mut self,
        goal: &Proposition,
        assumptions: &[Proposition],
    ) -> Option<ProofNode> {
        let key = (goal.clone(), assumptions.to_vec());
        match self.proved.get(&key) {
            Some(proof) => proof.clone(),
            None => {
                let proof = (self.ordinary)(goal, assumptions);
                self.proved.insert(key, proof.clone());
                proof
            }
        }
    }

    fn goal(
        &mut self,
        goal: &Proposition,
        assumptions: &[Proposition],
        depth: usize,
        allow_cases: bool,
    ) -> Option<ProofNode> {
        if depth >= 64 {
            return None;
        }
        self.remaining = self.remaining.checked_sub(1)?;
        if let Some(proof) = self.ordinary_proof(goal, assumptions) {
            return Some(proof);
        }
        let logical = match goal {
            Proposition::Conjunction(parts) => logical::prove_conjunction(goal, parts, |part| {
                self.goal(part, assumptions, depth + 1, allow_cases)
            }),
            Proposition::Disjunction(parts) => logical::prove_disjunction(goal, parts, |part| {
                self.goal(part, assumptions, depth + 1, allow_cases)
            }),
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                // The kernel appends this premise only while checking the body.
                // Preserve that exact assumption order so nested introductions
                // and case analysis cannot leak a conditional fact outward.
                let mut scoped = assumptions.to_vec();
                scoped.push(*premise.clone());
                self.goal(conclusion, &scoped, depth + 1, allow_cases)
                    .map(|body| ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::ImplicationIntroduction {
                            body: Box::new(body),
                        },
                    })
            }
            _ => None,
        };
        if logical.is_some() {
            return logical;
        }
        // Only implications connected to the goal's values can shorten its
        // proof; an unrelated guard would otherwise be tried, and its premise
        // proved, at every depth of the search.
        let mut implications =
            case_analysis::connected_implications(goal, assumptions, self.semantic_axioms);
        // Premise probes spend the same search budget as recursive questions;
        // their ordinary proofs are memoized for the subsequent selected goal.
        self.remaining = self.remaining.checked_sub(implications.len())?;
        implications.sort_by_cached_key(|fact| match fact.proposition {
            Proposition::Implication { premise, .. } => {
                self.ordinary_proof(premise, assumptions).is_none()
            }
            _ => true,
        });
        for fact in implications {
            let Proposition::Implication {
                premise,
                conclusion,
            } = fact.proposition
            else {
                continue;
            };
            if self.active.contains(fact.proposition) || assumptions.contains(conclusion) {
                continue;
            }
            self.remaining = self.remaining.checked_sub(1)?;
            self.active.push(fact.proposition.clone());
            let premise = self.goal(premise, assumptions, depth + 1, allow_cases);
            if let Some(premise) = premise {
                let consequence = ProofNode {
                    conclusion: *conclusion.clone(),
                    rule: ProofRule::ImplicationElimination {
                        implication: Box::new(fact.proof()),
                        premise: Box::new(premise),
                    },
                };
                let mut scoped = assumptions.to_vec();
                scoped.push(*conclusion.clone());
                let body = self.goal(goal, &scoped, depth + 1, allow_cases);
                self.active.pop();
                if let Some(body) = body {
                    return Some(ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::ImplicationElimination {
                            implication: Box::new(ProofNode {
                                conclusion: Proposition::Implication {
                                    premise: conclusion.clone(),
                                    conclusion: Box::new(goal.clone()),
                                },
                                rule: ProofRule::ImplicationIntroduction {
                                    body: Box::new(body),
                                },
                            }),
                            premise: Box::new(consequence),
                        },
                    });
                }
            } else {
                self.active.pop();
            }
        }
        // Case analysis appends only the currently selected disjunct. Reuse
        // its finite case roster rather than recursively selecting it again.
        allow_cases.then(|| {
            case_analysis::prove(goal, assumptions, self.semantic_axioms, |branch| {
                self.goal(goal, branch, depth + 1, false)
            })
        })?
    }
}
