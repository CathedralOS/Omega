//! Introduce one unresolved case for the shared integer proof search.

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

/// Split the first unresolved connected case. The caller owns recursive search
/// in each branch, including any new cases its proved implications introduce.
pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    mut prove_branch: impl FnMut(&[Proposition]) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let cases = dependencies::connected_cases(goal, assumptions, semantic_axioms);
    // Stable citation order and skipping established alternatives prevent
    // permutations and repeated assumptions. Conditional conclusions are not
    // established until the shared search has proved their premises.
    let (fact, disjuncts) = cases.into_iter().find_map(|fact| {
        let Proposition::Disjunction(disjuncts) = fact.proposition else {
            return None;
        };
        let resolved = disjuncts.iter().any(|disjunct| {
            assumptions
                .iter()
                .chain(semantic_axioms)
                .any(|fact| contains_unconditional(fact, disjunct))
        });
        (!resolved).then_some((fact, disjuncts))
    })?;
    let branches = disjuncts
        .iter()
        .map(|disjunct| {
            let mut branch_assumptions = assumptions.to_vec();
            branch_assumptions.push(disjunct.clone());
            prove_branch(&branch_assumptions)
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
    // The recursive owner has already tried remaining cases in the stronger
    // branch scope. Retrying them here merely changes their order.
    None
}

fn contains_unconditional(fact: &Proposition, wanted: &Proposition) -> bool {
    fact == wanted
        || matches!(fact, Proposition::Conjunction(parts)
            if parts.iter().any(|part| contains_unconditional(part, wanted)))
}
