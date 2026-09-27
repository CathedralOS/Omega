//! Canonical compound integer proposition proof construction.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody::DefinitionIndex;

mod integer_contradiction;

/// A selected edge can be impossible under a literal guard or declared guarantees.
/// Keep that edge in the arrival roster and prove its contradiction using exact
/// citations. `false and goal` has the same false denotation; the existing
/// conversion and conjunction elimination close the goal without a new rule
/// or a trusted prune. Every contradictory citation remains in the proof.
pub(super) fn prove_contradiction(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    // Reconstruction closes a literal guard directly to Falsehood, whereas a
    // call-produced guard can retain contradictory value equations. Both need
    // exact citations: neither permits pruning the impossible arrival itself.
    let contradiction =
        super::super::integer_evidence::projected_facts(assumptions, semantic_axioms)
            .into_iter()
            .find(|fact| fact.proposition == &Proposition::Falsehood)
            .map(|fact| fact.proof())
            .or_else(|| {
                super::exact::prove(
                    context,
                    &Proposition::Equal(ScalarTerm::Boolean(false), ScalarTerm::Boolean(true)),
                    assumptions,
                    semantic_axioms,
                )
            })
            .or_else(|| {
                // The derived legs depend on the scope alone, so the session
                // memo computes them once and every goal visited under this
                // scope shares the answer.
                super::exact::session(context, assumptions, semantic_axioms).contradiction(|| {
                    integer_contradiction::prove(context, assumptions, semantic_axioms, definitions)
                })
            })?;
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::ConjunctionElimination {
            conjunction: Box::new(ProofNode {
                conclusion: Proposition::Conjunction(vec![Proposition::Falsehood, goal.clone()]),
                rule: ProofRule::PredicateDenotation {
                    premise: Box::new(contradiction),
                },
            }),
            conjunct: 1,
        },
    })
}

pub(in super::super) fn prove_conjunction(
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

pub(in super::super) fn prove_disjunction(
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

#[cfg(test)]
mod tests {}
