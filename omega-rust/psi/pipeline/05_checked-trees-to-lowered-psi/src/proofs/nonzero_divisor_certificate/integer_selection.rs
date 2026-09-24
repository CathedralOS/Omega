//! Canonical fixed-integer proposition and bound selection.

use std::collections::BTreeSet;

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ValueId};

use super::affine_custody::DefinitionIndex;

mod bound;
mod case_analysis;
mod derived;
mod direct_add;
mod dispatch;
mod exact;
mod forbidden_root;
mod implications;
mod logical;
mod multiply;
mod order;
mod range;
mod shift;
mod substitution;
mod wrapping;

pub(super) use logical::{prove_conjunction, prove_disjunction};

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
    // Whole-selection answers are a pure function of `(goal, parameter
    // roster)` under this scope. Cast completion, value transport, and
    // predicate conversion re-enter this entry for derived goals from inside
    // a running search, so the answer is memoized per scope instead of
    // paying a fresh bounded search for every re-entry. The in-progress
    // marker turns a cyclic re-entry for the same goal into `None` rather
    // than recursion.
    let mut definitions = DefinitionIndex::new(context, assumptions, semantic_axioms);
    if let Some(proof) = definitions.cached_build_proof(goal, machine_parameter_values) {
        return proof;
    }
    definitions.begin_build_proof(goal, machine_parameter_values);
    let ordinary = |goal: &Proposition, assumptions: &[Proposition]| {
        build_without_implications(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
        )
    };
    let proof = implications::prove(goal, assumptions, semantic_axioms, ordinary);
    definitions.cache_build_proof(goal, machine_parameter_values, proof.clone());
    proof
}

/// Relaxed selection for obligations that outgrow the canonical custody
/// envelope: the same guarded search, with the bounded equality/definition
/// closure as an additional leaf producer.
///
/// Canonical producers still run first at every goal, so ordered
/// definition-word custody keeps its contract inside `build`. The closure
/// only consumes facts unconditional in its own scope: a premise cited under
/// an `Implication` still discharges nothing outside that premise's scope.
/// Only the operation-proof fallback calls this; the canonical certificate
/// entry never reaches it.
pub(super) fn prove_relaxed_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    // The relaxed leaf set differs from `build`, so its answers memoize
    // under their own map rather than the canonical `build` answers.
    let mut definitions = DefinitionIndex::new(context, assumptions, semantic_axioms);
    if let Some(proof) = definitions.cached_relaxed_proof(goal, machine_parameter_values) {
        return proof;
    }
    definitions.begin_relaxed_proof(goal, machine_parameter_values);
    let ordinary = |goal: &Proposition, assumptions: &[Proposition]| {
        build_without_implications(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
        )
        .or_else(|| derived::prove(context, goal, assumptions, semantic_axioms))
    };
    let proof = implications::prove(goal, assumptions, semantic_axioms, ordinary);
    definitions.cache_relaxed_proof(goal, machine_parameter_values, proof.clone());
    proof
}

fn build_without_implications(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<ProofNode> {
    // This is the `ordinary` leaf inside every implication search: sibling
    // searches under this exact scope discharge the same premises, so the
    // composed answer is memoized per `(goal, parameter roster)` instead of
    // re-running the cascade for each of them. The in-progress marker turns
    // a cyclic re-entry into `None` rather than recursion.
    let mut definitions = DefinitionIndex::new(context, assumptions, semantic_axioms);
    if let Some(proof) = definitions.cached_selection_proof(goal, machine_parameter_values) {
        return proof;
    }
    definitions.begin_selection_proof(goal, machine_parameter_values);
    let proof = build_without_implications_uncached(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    );
    definitions.cache_selection_proof(goal, machine_parameter_values, proof.clone());
    proof
}

fn build_without_implications_uncached(
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
            build_without_implications(
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
    let mut definitions = DefinitionIndex::new(context, assumptions, semantic_axioms);
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
    .or_else(|| {
        logical::prove_contradiction(
            context,
            goal,
            assumptions,
            semantic_axioms,
            &mut definitions,
        )
    })
}

fn build_with_definitions(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    // One goal carries one answer under this scope: conjunction and
    // disjunction splits re-ask goals the direct path already settled, and
    // rebuilding the cascade per visit is where the producer fan-out grows.
    // `None` is the in-progress marker that turns a hypothetical cycle into
    // the answer `None` rather than recursion.
    if let Some(proof) = definitions.cached_cascade_proof(goal) {
        return proof;
    }
    definitions.begin_cascade_proof(goal);
    let proof =
        build_with_definitions_uncached(context, goal, assumptions, semantic_axioms, definitions);
    definitions.cache_cascade_proof(goal, proof.clone());
    proof
}

fn build_with_definitions_uncached(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = exact::prove(context, goal, assumptions, semantic_axioms) {
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
