//! Correlated carrier-complement selection for direct exact add.

use psi_core::{IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType};
use psi_proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};

use super::super::bound;
use crate::nonzero_divisor_certificate::affine_custody::DefinitionIndex;
use crate::nonzero_divisor_certificate::integer_evidence::cited_facts;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    target: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let expected_endpoint = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let complement = match fact {
            Proposition::LessOrEqual(complement, actual_left) if lower && actual_left == left => {
                complement
            }
            Proposition::LessOrEqual(actual_left, complement) if !lower && actual_left == left => {
                complement
            }
            _ => continue,
        };
        let ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left: endpoint,
            right: subtract_right,
        } = complement
        else {
            continue;
        };
        if *scalar_type != integer_type
            || subtract_right.as_ref() != right
            || endpoint.integer_value() != Some((integer_type, expected_endpoint))
        {
            continue;
        }
        let endpoint_proof = citation.proof(fact);
        if let Some(proof) = mapped(
            context,
            goal,
            semantic_axioms,
            complement,
            target,
            endpoint_proof,
            Vec::new(),
            Vec::new(),
        ) {
            return Some(proof);
        }
    }
    for (index, axiom) in semantic_axioms.iter().enumerate() {
        let Proposition::Equal(equal_left, equal_right) = axiom else {
            continue;
        };
        for (root, expression) in [(equal_left, equal_right), (equal_right, equal_left)] {
            let ScalarTerm::Value {
                scalar_type: ScalarType::Integer(root_type),
                ..
            } = root
            else {
                continue;
            };
            let ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left: endpoint,
                right: subtract_right,
            } = expression
            else {
                continue;
            };
            if *root_type != integer_type
                || *scalar_type != integer_type
                || subtract_right.as_ref() != right
            {
                continue;
            }
            let literal_axiom =
                if endpoint.integer_value() == Some((integer_type, expected_endpoint)) {
                    None
                } else {
                    let Some(landing_index) = semantic_axioms[..index].iter().enumerate().find_map(
                        |(landing_index, landing)| {
                            let Proposition::Equal(landing_left, landing_right) = landing else {
                                return None;
                            };
                            let literal = if landing_left == endpoint.as_ref() {
                                landing_right
                            } else if landing_right == endpoint.as_ref() {
                                landing_left
                            } else {
                                return None;
                            };
                            (literal.integer_value() == Some((integer_type, expected_endpoint)))
                                .then_some(landing_index)
                        },
                    ) else {
                        continue;
                    };
                    Some(landing_index)
                };
            let endpoint_goal = if lower {
                Proposition::LessOrEqual(root.clone(), left.clone())
            } else {
                Proposition::LessOrEqual(left.clone(), root.clone())
            };
            let Some(endpoint_proof) = bound::prove(
                context,
                &endpoint_goal,
                assumptions,
                semantic_axioms,
                definitions,
            ) else {
                continue;
            };
            if let Some(proof) = mapped(
                context,
                goal,
                semantic_axioms,
                root,
                target,
                endpoint_proof,
                vec![index],
                vec![literal_axiom],
            ) {
                return Some(proof);
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn mapped(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    target: &ScalarTerm,
    endpoint_proof: ProofNode,
    definition_axioms: Vec<usize>,
    literal_axioms: Vec<Option<usize>>,
) -> Option<ProofNode> {
    let witness = IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms,
        literal_axioms,
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let conclusion = map_integer_affine_bound(&form, &endpoint_proof.conclusion).ok()?;
    (&conclusion == goal).then_some(ProofNode {
        conclusion,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(endpoint_proof),
            witness,
        },
    })
}
