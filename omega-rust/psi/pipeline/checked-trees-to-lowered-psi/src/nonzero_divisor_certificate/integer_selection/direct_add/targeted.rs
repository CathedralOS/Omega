//! Targeted operand-endpoint pairing for direct exact add.

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerType, Proposition, PropositionContext, ScalarTerm};

use super::super::{dispatch, multiply};
use crate::nonzero_divisor_certificate::affine_custody::DefinitionIndex;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    allow_relaxation: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let operand_endpoints = |operand: &ScalarTerm, definitions: &mut DefinitionIndex| {
        if let Some((actual, value)) = operand.integer_value() {
            return (actual == integer_type)
                .then(|| {
                    dispatch::prove_add_operand_endpoint(
                        context,
                        integer_type,
                        operand,
                        value,
                        lower,
                        assumptions,
                        semantic_axioms,
                        definitions,
                    )
                })
                .flatten()
                .into_iter()
                .collect::<Vec<_>>();
        }
        multiply::targeted_operand_endpoints(
            context,
            integer_type,
            operand,
            lower,
            assumptions,
            semantic_axioms,
            definitions,
        )
    };
    let left_endpoints = operand_endpoints(left, definitions);
    if left_endpoints.is_empty() {
        return None;
    }
    let right_endpoints = operand_endpoints(right, definitions);
    if right_endpoints.is_empty() {
        return None;
    }
    let witness = IntegerAffineWitness {
        root: left.clone(),
        target: target.clone(),
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let mut relaxed = None;
    for left_endpoint in left_endpoints {
        for right_endpoint in &right_endpoints {
            let evidence = ProofNode {
                conclusion: Proposition::Conjunction(vec![
                    left_endpoint.conclusion.clone(),
                    right_endpoint.conclusion.clone(),
                ]),
                rule: ProofRule::ConjunctionIntroduction(vec![
                    left_endpoint.clone(),
                    right_endpoint.clone(),
                ]),
            };
            let Ok(mapped) = map_integer_affine_bound(&form, &evidence.conclusion) else {
                continue;
            };
            let mapped_proof = ProofNode {
                conclusion: mapped.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(evidence),
                    witness: witness.clone(),
                },
            };
            if &mapped == goal {
                return Some(mapped_proof);
            }
            if relaxed.is_none() {
                relaxed = dispatch::relax_math_bound(goal, mapped_proof);
            }
        }
    }
    allow_relaxation.then_some(relaxed).flatten()
}
