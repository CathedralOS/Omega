//! Producer-local affine/cast/affine proof composition for one resolved root.

use psi_core::{IntegerType, Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::super::{affine_custody, cast_custody};
use super::super::endpoint;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    source: &ScalarTerm,
    first_cast: usize,
    cast_root: &ScalarTerm,
    cast_type: IntegerType,
    last_cast: usize,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    let source_bound = affine_custody::prove_mapped_to_target_before(
        context,
        assumptions,
        semantic_axioms,
        definitions,
        root,
        source,
        first_cast,
        &root_bound,
    )?;
    let cast_goal = endpoint::remap(&source_bound.conclusion, source, cast_root, cast_type)?;
    let cast_bound = cast_custody::prove_from_root(
        context,
        &cast_goal,
        assumptions,
        semantic_axioms,
        source,
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
}
