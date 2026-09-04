//! Producer-local cast-to-affine completion for one retained root bound.

use psi_core::{IntegerType, Proposition, PropositionContext, ScalarTerm};
use psi_proof_admission::{ProofNode, ProofRule};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::super::super::{affine_custody, cast_custody};
use super::super::endpoint;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    source: &ScalarTerm,
    cast_root: &ScalarTerm,
    cast_type: IntegerType,
    last_cast: usize,
    assumption: usize,
    root_bound: &Proposition,
) -> Option<ProofNode> {
    let cast_goal = endpoint::remap(root_bound, source, cast_root, cast_type)?;
    let cast_bound = cast_custody::prove_from_root(
        context,
        &cast_goal,
        assumptions,
        semantic_axioms,
        source,
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
}
