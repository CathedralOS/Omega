//! Bounded composition of independently cited implications and exact equalities.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

use super::super::integer_evidence::projected_facts;
use super::{case_analysis, exact, logical};

#[cfg(test)]
mod tests;

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
    }
    .goal(goal, assumptions, 0, true)
}

struct Search<'input, Ordinary> {
    semantic_axioms: &'input [Proposition],
    ordinary: Ordinary,
    remaining: usize,
    active: Vec<Proposition>,
}

impl<Ordinary: Fn(&Proposition, &[Proposition]) -> Option<ProofNode>> Search<'_, Ordinary> {
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
        if let Some(proof) = (self.ordinary)(goal, assumptions) {
            return Some(proof);
        }
        let logical = match goal {
            Proposition::Conjunction(parts) => logical::prove_conjunction(goal, parts, |part| {
                self.goal(part, assumptions, depth + 1, allow_cases)
            }),
            Proposition::Disjunction(parts) => logical::prove_disjunction(goal, parts, |part| {
                self.goal(part, assumptions, depth + 1, allow_cases)
            }),
            _ => None,
        };
        if logical.is_some() {
            return logical;
        }
        for fact in projected_facts(assumptions, self.semantic_axioms) {
            let Proposition::Implication {
                premise,
                conclusion,
            } = fact.proposition
            else {
                continue;
            };
            if self.active.contains(fact.proposition) {
                continue;
            }
            self.remaining = self.remaining.checked_sub(1)?;
            let Some(bridge) =
                EqualityBridge::select(goal, conclusion, assumptions, self.semantic_axioms)
            else {
                continue;
            };
            self.active.push(fact.proposition.clone());
            let premise = self.goal(premise, assumptions, depth + 1, allow_cases);
            self.active.pop();
            if let Some(premise) = premise {
                let proof = ProofNode {
                    conclusion: *conclusion.clone(),
                    rule: ProofRule::ImplicationElimination {
                        implication: Box::new(fact.proof()),
                        premise: Box::new(premise),
                    },
                };
                return Some(bridge.apply(proof));
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

enum EqualityBridge {
    Exact,
    Through {
        prefix: ProofNode,
        suffix: ProofNode,
        reverse: bool,
    },
}

impl EqualityBridge {
    fn select(
        goal: &Proposition,
        conclusion: &Proposition,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
    ) -> Option<Self> {
        if goal == conclusion {
            return Some(Self::Exact);
        }
        let (Proposition::Equal(goal_left, goal_right), Proposition::Equal(left, right)) =
            (goal, conclusion)
        else {
            return None;
        };
        for (left, right, reverse) in [(left, right, false), (right, left, true)] {
            if let Some(prefix) = exact::prove(
                &Proposition::Equal(goal_left.clone(), left.clone()),
                assumptions,
                semantic_axioms,
            ) && let Some(suffix) = exact::prove(
                &Proposition::Equal(right.clone(), goal_right.clone()),
                assumptions,
                semantic_axioms,
            ) {
                return Some(Self::Through {
                    prefix,
                    suffix,
                    reverse,
                });
            }
        }
        None
    }

    fn apply(self, mut proof: ProofNode) -> ProofNode {
        let Self::Through {
            prefix,
            suffix,
            reverse,
        } = self
        else {
            return proof;
        };
        if reverse {
            let Proposition::Equal(left, right) = &proof.conclusion else {
                unreachable!("only equalities need orientation")
            };
            proof = ProofNode {
                conclusion: Proposition::Equal(right.clone(), left.clone()),
                rule: ProofRule::EqualitySymmetry {
                    equality: Box::new(proof),
                },
            };
        }
        join(join(prefix, proof), suffix)
    }
}

fn join(left: ProofNode, right: ProofNode) -> ProofNode {
    let (Proposition::Equal(start, middle), Proposition::Equal(other_middle, end)) =
        (&left.conclusion, &right.conclusion)
    else {
        unreachable!("equality bridges retain their exact endpoints")
    };
    debug_assert_eq!(middle, other_middle);
    if start == middle {
        return right;
    }
    if other_middle == end {
        return left;
    }
    ProofNode {
        conclusion: Proposition::Equal(start.clone(), end.clone()),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(left),
            middle_equals_right: Box::new(right),
        },
    }
}
