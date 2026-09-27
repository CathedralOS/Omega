//! Ordered atomic integer-bound proof selection.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarType};

use super::super::{
    affine_custody::DefinitionIndex, affine_selection, cast_custody, cast_selection,
};
use super::{derived, order, range, shift, substitution, wrapping};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    // The cascade below is a pure function of `(goal, scope)`; the index's
    // memo replays it per goal instead of rescanning the cited roster for
    // every endpoint substitution the strict/discrete producers attempt.
    if let Some(proof) = definitions.cached_bound_proof(goal) {
        return proof;
    }
    definitions.begin_bound_proof(goal);
    let proof = prove_uncached(context, goal, assumptions, semantic_axioms, definitions);
    definitions.cache_bound_proof(goal, proof.clone());
    proof
}

fn prove_uncached(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = super::super::integer_evidence::integer_carrier_bound(context, goal) {
        return Some(proof);
    }
    if let Some(proof) =
        order::prove_exact_or_closed_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) =
        order::prove_two_fact_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) =
        substitution::prove(context, goal, assumptions, semantic_axioms, definitions)
    {
        return Some(proof);
    }

    order::prove_aliased_integer_bound(context, goal, assumptions, semantic_axioms)
        .or_else(|| cast_selection::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| prove_derived_cast_bound(context, goal, assumptions, semantic_axioms))
        .or_else(|| {
            affine_selection::prove_with_definitions(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
            )
        })
        .or_else(|| wrapping::prove(context, goal, assumptions, semantic_axioms, definitions))
        .or_else(|| shift::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| range::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| order::prove_equal_integer_bound(context, goal, assumptions, semantic_axioms))
}

pub(super) fn prove_candidate_endpoint(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    // Same one-question-per-goal memo as `prove`: wrapping evidence and shift
    // endpoints ask this cascade for the same `operand <= bound` shapes many
    // times within one scope.
    if let Some(proof) = definitions.cached_endpoint_proof(goal) {
        return proof;
    }
    definitions.begin_endpoint_proof(goal);
    let proof =
        prove_candidate_endpoint_uncached(context, goal, assumptions, semantic_axioms, definitions);
    definitions.cache_endpoint_proof(goal, proof.clone());
    proof
}

fn prove_candidate_endpoint_uncached(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) =
        order::prove_exact_or_closed_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) =
        order::prove_two_fact_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) =
        substitution::prove_without_cast(context, goal, assumptions, semantic_axioms, definitions)
    {
        return Some(proof);
    }

    order::prove_aliased_integer_bound(context, goal, assumptions, semantic_axioms)
        .or_else(|| cast_selection::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| prove_derived_cast_bound(context, goal, assumptions, semantic_axioms))
        .or_else(|| {
            affine_selection::prove_without_cast(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
            )
        })
        .or_else(|| shift::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| range::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| derived::prove(context, goal, assumptions, semantic_axioms))
        .or_else(|| order::prove_equal_integer_bound(context, goal, assumptions, semantic_axioms))
}

fn prove_derived_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = goal else {
        return None;
    };
    let (target, endpoint, lower) = if left.integer_value().is_some() {
        (right, left, true)
    } else if right.integer_value().is_some() {
        (left, right, false)
    } else {
        return None;
    };
    let (root, _) = cast_custody::source_root(target, semantic_axioms)?;
    let ScalarType::Integer(integer_type) = root.scalar_type() else {
        return None;
    };
    // Compute source endpoints before transporting them: the cast's carrier
    // bound alone loses the tighter range established by source arithmetic.
    let endpoint = cast_custody::remap_integer_literal(endpoint, integer_type)?;
    let source_goal = if lower {
        Proposition::LessOrEqual(endpoint, root.clone())
    } else {
        Proposition::LessOrEqual(root.clone(), endpoint)
    };
    let root_bound = derived::prove(context, &source_goal, assumptions, semantic_axioms)?;
    cast_custody::prove_from_root(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &root,
        root_bound,
    )
}
