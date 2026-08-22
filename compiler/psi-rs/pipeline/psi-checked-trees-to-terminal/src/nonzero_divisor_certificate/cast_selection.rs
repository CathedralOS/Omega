//! Side-local evidence selection for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::integer_evidence::{cited_facts, closed_integer_relation};
use super::{alias_transport, cast_custody};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if !matches!(goal, Proposition::LessOrEqual(_, _)) {
        return None;
    }
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(root_left, root_right) = root_bound else {
            continue;
        };
        for root in [root_left, root_right]
            .into_iter()
            .filter(|root| matches!(root, psi_core::ScalarTerm::Value { .. }))
        {
            if let Some(proof) = cast_custody::prove_from_root(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                citation.proof(root_bound),
            ) {
                return Some(proof);
            }
        }
    }
    prove_landed_literal_cast_bound(context, goal, assumptions, semantic_axioms)
        .or_else(|| prove_alias_substituted_cast_bound(context, goal, assumptions, semantic_axioms))
}

fn prove_landed_literal_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(left, right) = equality else {
            continue;
        };
        for (root, literal) in [(left, right), (right, left)] {
            if !matches!(root, psi_core::ScalarTerm::Value { .. }) {
                continue;
            }
            let Some((integer_type, _)) = literal.integer_value() else {
                continue;
            };
            if root.scalar_type() != psi_core::ScalarType::Integer(integer_type) {
                continue;
            }
            if let Some(proof) = prove_cast_bound_from_landed_literal(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                literal,
                citation.proof(equality),
            ) {
                return Some(proof);
            }
        }
    }
    None
}

fn prove_cast_bound_from_landed_literal(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &psi_core::ScalarTerm,
    landed_literal: &psi_core::ScalarTerm,
    equality: ProofNode,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let psi_core::ScalarType::Integer(root_type) = root.scalar_type() else {
        return None;
    };
    for (target, target_endpoint, endpoint) in
        [(goal_right, goal_left, 1), (goal_left, goal_right, 0)]
    {
        if !matches!(target, psi_core::ScalarTerm::Value { .. }) {
            continue;
        }
        let Some(source_endpoint) = cast_custody::remap_integer_literal(target_endpoint, root_type)
        else {
            continue;
        };
        let closed_relation = if endpoint == 1 {
            Proposition::LessOrEqual(source_endpoint.clone(), landed_literal.clone())
        } else {
            Proposition::LessOrEqual(landed_literal.clone(), source_endpoint.clone())
        };
        let Some(closed_relation) = closed_integer_relation(closed_relation) else {
            continue;
        };
        let root_bound = ProofNode {
            conclusion: if endpoint == 1 {
                Proposition::LessOrEqual(source_endpoint, root.clone())
            } else {
                Proposition::LessOrEqual(root.clone(), source_endpoint)
            },
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(closed_relation),
                equality: Box::new(equality.clone()),
                endpoint,
            },
        };
        if let Some(proof) = cast_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        ) {
            return Some(proof);
        }
    }
    None
}

fn prove_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_one(assumptions, semantic_axioms, |root, root_bound| {
        cast_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
    .or_else(|| alias_transport::prove_stronger_cast(context, goal, assumptions, semantic_axioms))
    .or_else(|| {
        alias_transport::prove_landed_literal_cast(context, goal, assumptions, semantic_axioms)
    })
    .or_else(|| prove_two_alias_substituted_cast_bound(context, goal, assumptions, semantic_axioms))
}

fn prove_two_alias_substituted_cast_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_two(assumptions, semantic_axioms, |root, root_bound| {
        cast_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
}
