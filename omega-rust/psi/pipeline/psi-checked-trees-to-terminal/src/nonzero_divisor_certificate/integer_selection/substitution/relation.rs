//! Fixed inner-relation precedence for one endpoint substitution.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

use super::super::super::integer_evidence::closed_integer_relation;
use super::super::super::{affine_custody::DefinitionIndex, affine_selection};
use super::super::order::{
    prove_exact_or_closed_transitive_integer_bound, prove_two_fact_transitive_integer_bound,
};

pub(super) fn prove(
    context: &PropositionContext,
    relation: &Proposition,
    replacement_is_literal: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    allow_cast: bool,
) -> Option<ProofNode> {
    prove_exact_or_closed_transitive_integer_bound(relation, assumptions, semantic_axioms)
        .or_else(|| prove_two_fact_transitive_integer_bound(relation, assumptions, semantic_axioms))
        .or_else(|| {
            if allow_cast {
                affine_selection::prove_with_definitions(
                    context,
                    relation,
                    assumptions,
                    semantic_axioms,
                    definitions,
                )
            } else {
                affine_selection::prove_without_cast(
                    context,
                    relation,
                    assumptions,
                    semantic_axioms,
                    definitions,
                )
            }
        })
        .or_else(|| {
            replacement_is_literal
                .then(|| closed_integer_relation(relation.clone()))
                .flatten()
        })
}
