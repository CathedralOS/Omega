//! Fixed inner-relation precedence for one endpoint substitution.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::super::affine_selection;
use super::super::super::integer_evidence::closed_integer_relation;
use super::super::order::{
    prove_exact_or_closed_transitive_integer_bound, prove_two_fact_transitive_integer_bound,
};

pub(super) fn prove(
    context: &PropositionContext,
    relation: &Proposition,
    replacement_is_literal: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    prove_exact_or_closed_transitive_integer_bound(relation, assumptions, semantic_axioms)
        .or_else(|| prove_two_fact_transitive_integer_bound(relation, assumptions, semantic_axioms))
        .or_else(|| affine_selection::prove(context, relation, assumptions, semantic_axioms))
        .or_else(|| {
            replacement_is_literal
                .then(|| closed_integer_relation(relation.clone()))
                .flatten()
        })
}
