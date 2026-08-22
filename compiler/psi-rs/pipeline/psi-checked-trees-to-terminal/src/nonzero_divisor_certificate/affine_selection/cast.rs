//! Direct cast-root custody for one following affine word.

use psi_core::{Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::affine_custody::DefinitionIndex;
use super::super::{affine_custody, cast_custody};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    prove_from_direct_bound(context, goal, assumptions, semantic_axioms, definitions).or_else(
        || prove_affine_cast_affine(context, goal, assumptions, semantic_axioms, definitions),
    )
}

fn prove_from_direct_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    semantic_axioms.iter().find_map(|axiom| {
        let Proposition::Equal(cast_root, ScalarTerm::IntegerExactCast { .. }) = axiom else {
            return None;
        };
        let ScalarType::Integer(cast_type) = cast_root.scalar_type() else {
            return None;
        };
        let (source, _) = cast_custody::source_root(cast_root, semantic_axioms)?;
        let cast_word = cast_custody::definition_axioms(&source, cast_root, semantic_axioms)?;
        let last_cast = *cast_word.last()?;
        assumptions
            .iter()
            .enumerate()
            .find_map(|(assumption, root_bound)| {
                let cast_goal = remap_direct_bound(root_bound, &source, cast_root, cast_type)?;
                let cast_bound = cast_custody::prove_from_root(
                    context,
                    &cast_goal,
                    assumptions,
                    semantic_axioms,
                    &source,
                    ProofNode {
                        conclusion: root_bound.clone(),
                        rule: ProofRule::Assumption { index: assumption },
                    },
                )?;
                affine_custody::prove_from_root_after(
                    context,
                    goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                    cast_root,
                    last_cast,
                    cast_bound,
                )
            })
    })
}

fn prove_affine_cast_affine(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    semantic_axioms.iter().find_map(|axiom| {
        let Proposition::Equal(cast_root, ScalarTerm::IntegerExactCast { .. }) = axiom else {
            return None;
        };
        let ScalarType::Integer(cast_type) = cast_root.scalar_type() else {
            return None;
        };
        let (source, first_cast) = cast_custody::source_root(cast_root, semantic_axioms)?;
        let cast_word = cast_custody::definition_axioms(&source, cast_root, semantic_axioms)?;
        let last_cast = *cast_word.last()?;
        assumptions
            .iter()
            .enumerate()
            .find_map(|(assumption, root_bound)| {
                let Proposition::LessOrEqual(left, right) = root_bound else {
                    return None;
                };
                [left, right].into_iter().find_map(|root| {
                    if !matches!(root, ScalarTerm::Value { .. }) || root == &source {
                        return None;
                    }
                    let source_bound = affine_custody::prove_mapped_to_target_before(
                        context,
                        assumptions,
                        semantic_axioms,
                        definitions,
                        root,
                        &source,
                        first_cast,
                        &ProofNode {
                            conclusion: root_bound.clone(),
                            rule: ProofRule::Assumption { index: assumption },
                        },
                    )?;
                    let cast_goal = remap_direct_bound(
                        &source_bound.conclusion,
                        &source,
                        cast_root,
                        cast_type,
                    )?;
                    let cast_bound = cast_custody::prove_from_root(
                        context,
                        &cast_goal,
                        assumptions,
                        semantic_axioms,
                        &source,
                        source_bound,
                    )?;
                    affine_custody::prove_from_root_after(
                        context,
                        goal,
                        assumptions,
                        semantic_axioms,
                        definitions,
                        cast_root,
                        last_cast,
                        cast_bound,
                    )
                })
            })
    })
}

fn remap_direct_bound(
    bound: &Proposition,
    source: &ScalarTerm,
    cast_root: &ScalarTerm,
    cast_type: psi_core::IntegerType,
) -> Option<Proposition> {
    let Proposition::LessOrEqual(left, right) = bound else {
        return None;
    };
    if left == source {
        Some(Proposition::LessOrEqual(
            cast_root.clone(),
            cast_custody::remap_integer_literal(right, cast_type)?,
        ))
    } else if right == source {
        Some(Proposition::LessOrEqual(
            cast_custody::remap_integer_literal(left, cast_type)?,
            cast_root.clone(),
        ))
    } else {
        None
    }
}
