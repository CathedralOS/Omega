//! Compose ordinary integer certificates under each retained alternative.

use super::super::integer_evidence::ProjectedFact;
use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

mod dependencies;

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut ordinary: impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    // Stable citation order prevents revisiting permutations of the same
    // cases. Each recursive path consumes a strict suffix of this finite set.
    let cases = dependencies::connected_cases(goal, assumptions, semantic_axioms);
    prove_with_cases(goal, assumptions, &cases, &mut ordinary)
}

fn prove_with_cases(
    goal: &Proposition,
    assumptions: &[Proposition],
    cases: &[ProjectedFact<'_>],
    ordinary: &mut impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    if let Some(proof) = ordinary(assumptions) {
        return Some(proof);
    }
    for (index, fact) in cases.iter().enumerate() {
        let Proposition::Disjunction(disjuncts) = fact.proposition else {
            unreachable!("only retained disjunctions become cases")
        };
        let branches = disjuncts
            .iter()
            .map(|disjunct| {
                let mut branch_assumptions = assumptions.to_vec();
                branch_assumptions.push(disjunct.clone());
                prove_with_cases(goal, &branch_assumptions, &cases[index + 1..], ordinary)
            })
            .collect::<Option<Vec<_>>>();
        if let Some(branches) = branches {
            return Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::DisjunctionElimination {
                    disjunction: Box::new(fact.proof()),
                    branches,
                },
            });
        }
    }
    None
}
