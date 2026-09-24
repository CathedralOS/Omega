//! Producer-local completion of one ordered exact-cast target.

use proof_admission::{
    IntegerCastChainWitness, ProofNode, ProofRule, check_certificate,
    check_integer_cast_bound_conversion, check_integer_cast_chain_witness,
    integer_cast_truth_bounds, lower_integer_math_relation,
};
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::chain;
use crate::proofs::nonzero_divisor_certificate::integer_evidence::relax;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: &ProofNode,
    target: &ScalarTerm,
) -> Option<ProofNode> {
    // The kernel's IntegerCastBound rule is this chain replay plus the bound
    // conversion, so a chain that cannot carry the bound can never certify
    // the candidate. Checking the chain and the conversion before the full
    // certificate acceptance is exact: every candidate that could pass still
    // pays the kernel's own check.
    let definition_axioms = chain::definition_axioms(root, target, semantic_axioms)?;
    let witness = IntegerCastChainWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms,
    };
    let chain = check_integer_cast_chain_witness(context, semantic_axioms, &witness).ok()?;
    let normalized = lower_integer_math_relation(goal).unwrap_or_else(|| goal.clone());
    let proof = || ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerCastBound {
            root_bound: Box::new(root_bound.clone()),
            witness: witness.clone(),
        },
    };
    if root_bound.conclusion == Proposition::Truth {
        let bounds = integer_cast_truth_bounds(&chain).ok()?;
        if bounds.contains(&normalized)
            && check_certificate(context, goal, assumptions, semantic_axioms, &proof()).is_ok()
        {
            return Some(proof());
        }
        for mapped in bounds.iter() {
            let mapped_proof = ProofNode {
                conclusion: mapped.clone(),
                rule: ProofRule::IntegerCastBound {
                    root_bound: Box::new(root_bound.clone()),
                    witness: witness.clone(),
                },
            };
            if let Some(relaxed) = relax(goal, mapped_proof)
                && check_certificate(context, goal, assumptions, semantic_axioms, &relaxed).is_ok()
            {
                return Some(relaxed);
            }
        }
        return None;
    }
    if check_integer_cast_bound_conversion(&chain, &root_bound.conclusion, &normalized).is_ok()
        && check_certificate(context, goal, assumptions, semantic_axioms, &proof()).is_ok()
    {
        return Some(proof());
    }
    None
}
