//! Canonical compound integer proposition proof construction.

use psi_core::Proposition;
use psi_proof_kernel::{ProofNode, ProofRule};

pub(super) fn prove_conjunction(
    goal: &Proposition,
    conjuncts: &[Proposition],
    mut prove: impl FnMut(&Proposition) -> Option<ProofNode>,
) -> Option<ProofNode> {
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionIntroduction(
            conjuncts
                .iter()
                .map(&mut prove)
                .collect::<Option<Vec<_>>>()?,
        ),
    })
}

pub(super) fn prove_disjunction(
    goal: &Proposition,
    disjuncts: &[Proposition],
    mut prove: impl FnMut(&Proposition) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let (index, disjunct) = disjuncts
        .iter()
        .enumerate()
        .find_map(|(index, disjunct)| prove(disjunct).map(|proof| (index, proof)))?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(disjunct),
            index,
        },
    })
}
