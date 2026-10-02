//! Search an equivalent predicate view, then make each conversion explicit.
//! Published goals and premise slots never change: normalized citations are
//! backed by original citations under a checked PredicateDenotation step.

use proof_admission::{
    ProofNode, ProofRule, check_predicate_denotations, check_value_equality_denotation,
    retain_original_predicate_citations,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm, ValueId};
use std::collections::BTreeSet;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameters: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let converted =
        check_predicate_denotations(context, goal, assumptions, semantic_axioms).ok()?;
    let equations = semantic_axioms
        .iter()
        .enumerate()
        .filter_map(|(index, proposition)| {
            matches!(proposition, Proposition::Equal(ScalarTerm::Value { .. }, _)).then_some(
                ProofNode {
                    conclusion: proposition.clone(),
                    rule: ProofRule::SemanticAxiom { index },
                },
            )
        })
        .collect::<Vec<_>>();
    let contextualize = |proposition: &Proposition| {
        if !equations.is_empty()
            && matches!(
                proposition,
                Proposition::IntegerMathEqual(..)
                    | Proposition::IntegerMathLessThan(..)
                    | Proposition::IntegerMathLessOrEqual(..)
            )
        {
            check_value_equality_denotation(
                context,
                proposition,
                equations.iter().map(|proof| &proof.conclusion),
            )
            .ok()
            .unwrap_or_else(|| proposition.clone())
        } else {
            proposition.clone()
        }
    };
    let requirements = converted
        .requirements()
        .iter()
        .map(contextualize)
        .collect::<Vec<_>>();
    let axioms = converted
        .semantic_axioms()
        .iter()
        .map(contextualize)
        .collect::<Vec<_>>();
    if converted.goal() == goal && requirements == assumptions && axioms == semantic_axioms {
        return None;
    }
    let mut proof = super::integer_selection::build_with_machine_parameters(
        context,
        converted.goal(),
        &requirements,
        &axioms,
        machine_parameters,
    )?;
    retain_original_predicate_citations(
        &mut proof,
        context,
        assumptions,
        semantic_axioms,
        &equations,
    )?;
    Some(if &proof.conclusion == goal {
        proof
    } else {
        ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::PredicateDenotation {
                premise: Box::new(proof),
            },
        }
    })
}

#[cfg(test)]
mod tests {}
