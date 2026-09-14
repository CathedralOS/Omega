//! Canonical bound transport through unsigned wrapping definitions.
//!
//! `WrappingIntegerAdd`/`WrappingIntegerDivide` rows are affine in shape but
//! modular in semantics. Unsigned wrapping division maps bounds exactly;
//! wrapping addition maps an upper bound unconditionally while a lower bound
//! is sound only when the same definition supplies the checked no-wrap
//! conjunct `operand <= maximum - addend`. Traversed toward its operand the
//! equation inverts those directions. The proof kernel independently
//! reconstructs every requirement at admission, so this leg only assembles
//! the candidate root-bound conjunction from cited facts.

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_certificate, check_integer_affine_witness,
    integer_affine_wrapping_evidence, map_integer_affine_bound,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody::{self, DefinitionIndex};
use super::super::integer_evidence::{ProjectedFact, projected_facts};
use super::bound;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = definitions.cached_wrapping_proof(goal) {
        return proof;
    }
    definitions.begin_wrapping_proof(goal);
    let proof = prove_uncached(context, goal, assumptions, semantic_axioms, definitions);
    definitions.cache_wrapping_proof(goal, proof.clone());
    proof
}

fn prove_uncached(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let (goal_left, goal_right) = match goal {
        Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => (left, right),
        _ => return None,
    };
    for fact in projected_facts(assumptions, semantic_axioms) {
        let (fact_left, fact_right) = match fact.proposition {
            Proposition::LessThan(left, right) | Proposition::LessOrEqual(left, right) => {
                (left, right)
            }
            _ => continue,
        };
        for root in [fact_left, fact_right] {
            if !matches!(root, ScalarTerm::Value { .. }) {
                continue;
            }
            for target in [goal_left, goal_right] {
                if root == target || !matches!(target, ScalarTerm::Value { .. }) {
                    continue;
                }
                let words = affine_custody::definition_words_to_target(
                    context,
                    semantic_axioms,
                    definitions,
                    root,
                    target,
                );
                for word in words.iter() {
                    if let Some(proof) = prove_word(
                        context,
                        goal,
                        assumptions,
                        semantic_axioms,
                        definitions,
                        &fact,
                        root,
                        target,
                        word,
                    ) {
                        return Some(proof);
                    }
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn prove_word(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    fact: &ProjectedFact<'_>,
    root: &ScalarTerm,
    target: &ScalarTerm,
    definition_axioms: &[usize],
) -> Option<ProofNode> {
    let literal_axioms = affine_custody::literal_axioms(
        context,
        semantic_axioms,
        definitions,
        root,
        definition_axioms,
        target,
    )?;
    let witness = IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        literal_axioms,
        definition_axioms: definition_axioms.to_vec(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let evidence = integer_affine_wrapping_evidence(&form, fact.proposition).ok()?;
    if evidence.is_empty() && !matches!(fact.proposition, Proposition::LessThan(_, _)) {
        // A bare non-strict relation is already the ordinary affine path's
        // root bound. This leg exists for strict roots and for the wrapping
        // evidence conjunction the ordinary path cannot construct.
        return None;
    }
    let mut children = Vec::with_capacity(evidence.len() + 1);
    children.push(fact.proof());
    for required in &evidence {
        children.push(bound::prove_candidate_endpoint(
            context,
            required,
            assumptions,
            semantic_axioms,
            definitions,
        )?);
    }
    let root_bound = if children.len() == 1 {
        children
            .pop()
            .unwrap_or_else(|| unreachable!("one cited relation"))
    } else {
        ProofNode {
            conclusion: Proposition::Conjunction(
                children
                    .iter()
                    .map(|child| child.conclusion.clone())
                    .collect(),
            ),
            rule: ProofRule::ConjunctionIntroduction(children),
        }
    };
    let mapped = map_integer_affine_bound(&form, &root_bound.conclusion).ok()?;
    if mapped != *goal {
        return None;
    }
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness,
        },
    };
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}
