//! Compose ordinary integer certificates under each retained alternative.

use super::super::integer_evidence::ProjectedFact;
use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::Proposition;

mod dependencies;

/// The implications a goal can reach through value dependencies, for the
/// implication search that composes them.
pub(super) fn connected_implications<'a>(
    goal: &Proposition,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> Vec<ProjectedFact<'a>> {
    dependencies::connected_implications(goal, assumptions, semantic_axioms)
}

/// Prove a goal the caller has already failed to prove under `assumptions`
/// by eliminating each connected case; only a branch that adds an
/// alternative retries the ordinary builder.
pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut ordinary: impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    // Stable citation order prevents revisiting permutations of the same
    // cases. Each recursive path consumes a strict suffix of this finite set.
    let cases = dependencies::connected_cases(goal, assumptions, semantic_axioms);
    eliminate_cases(goal, assumptions, &cases, &mut ordinary)
}

fn prove_branch(
    goal: &Proposition,
    assumptions: &[Proposition],
    cases: &[ProjectedFact<'_>],
    ordinary: &mut impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    if let Some(proof) = ordinary(assumptions) {
        return Some(proof);
    }
    eliminate_cases(goal, assumptions, cases, ordinary)
}

fn eliminate_cases(
    goal: &Proposition,
    assumptions: &[Proposition],
    cases: &[ProjectedFact<'_>],
    ordinary: &mut impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    for (index, fact) in cases.iter().enumerate() {
        let Proposition::Disjunction(disjuncts) = fact.proposition else {
            unreachable!("only retained disjunctions become cases")
        };
        let branches = disjuncts
            .iter()
            .map(|disjunct| {
                let mut branch_assumptions = assumptions.to_vec();
                branch_assumptions.push(disjunct.clone());
                // The assumed alternative can carry alternatives of its own
                // (a nested computation's negative polarity, for example).
                // They are facts only inside this branch, so they extend this
                // branch's roster rather than the ambient case list; the
                // remaining shared cases still keep their citation order.
                let mut branch_cases = cases[index + 1..].to_vec();
                for nested in super::super::integer_evidence::nested_case_facts(
                    assumptions.len(),
                    branch_assumptions
                        .last()
                        .expect("branch assumption pushed above"),
                ) {
                    if !branch_cases
                        .iter()
                        .any(|case| case.proposition == nested.proposition)
                    {
                        branch_cases.push(nested);
                    }
                }
                prove_branch(goal, &branch_assumptions, &branch_cases, ordinary)
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
