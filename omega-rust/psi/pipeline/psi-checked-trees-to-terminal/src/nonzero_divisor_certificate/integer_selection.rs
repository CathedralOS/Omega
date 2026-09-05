//! Canonical fixed-integer proposition and bound selection.

use std::collections::BTreeSet;

use psi_core::{Proposition, PropositionContext, ValueId};
use psi_proof_admission::ProofNode;

use super::affine_custody::DefinitionIndex;

mod bound;
mod case_analysis;
mod direct_add;
mod dispatch;
mod exact;
mod forbidden_root;
mod logical;
mod multiply;
mod order;
mod range;
mod shift;
mod substitution;

pub(super) fn build(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    build_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
    )
}

pub(super) fn build_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    if let Some(proof) = build_without_cases(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    ) {
        return Some(proof);
    }
    // Keep whole-goal correlated proofs above this split. Independent
    // conjuncts need their own cases, not a Cartesian product of all cases.
    if let Proposition::Conjunction(conjuncts) = goal {
        return logical::prove_conjunction(goal, conjuncts, |part| {
            build_with_machine_parameters(
                context,
                part,
                assumptions,
                semantic_axioms,
                machine_parameter_values,
            )
        });
    }
    case_analysis::prove(goal, assumptions, semantic_axioms, |assumptions| {
        build_without_cases(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
        )
    })
}

fn build_without_cases(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    let mut definitions = DefinitionIndex::new(semantic_axioms);
    if let Some(proof) = forbidden_root::prove(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    ) {
        return Some(proof);
    }
    build_with_definitions(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &mut definitions,
    )
}

fn build_with_definitions(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = exact::prove(goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    if let Some(proof) =
        dispatch::prove_atomic(context, goal, assumptions, semantic_axioms, definitions)
    {
        return proof;
    }
    match goal {
        Proposition::Conjunction(conjuncts) => {
            logical::prove_conjunction(goal, conjuncts, |part| {
                build_with_definitions(context, part, assumptions, semantic_axioms, definitions)
            })
        }
        Proposition::Disjunction(disjuncts) => {
            logical::prove_disjunction(goal, disjuncts, |part| {
                build_with_definitions(context, part, assumptions, semantic_axioms, definitions)
            })
        }
        _ => None,
    }
}
