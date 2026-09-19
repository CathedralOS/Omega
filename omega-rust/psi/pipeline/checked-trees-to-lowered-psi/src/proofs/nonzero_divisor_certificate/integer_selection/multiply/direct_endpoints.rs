//! Direct computed, cited and affine multiply operand endpoints.

use crate::proofs::nonzero_divisor_certificate::affine_custody::DefinitionIndex;
use crate::proofs::nonzero_divisor_certificate::integer_evidence::{
    cited_facts, closed_integer_relation, projected_facts,
};
use crate::proofs::nonzero_divisor_certificate::integer_selection::multiply::targeted_witnesses::{
    targeted_affine_predecessors, targeted_literal_axiom, targeted_multiply_operand_witness,
};
use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType};

pub(crate) fn prove_direct_computed_multiply_endpoints(
    context: &PropositionContext,
    target: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Vec<ProofNode> {
    let mut proofs = Vec::new();
    for (definition_index, axiom) in semantic_axioms.iter().enumerate() {
        let Proposition::Equal(output, expression @ ScalarTerm::ExactIntegerMultiply { .. }) =
            axiom
        else {
            continue;
        };
        if output != target {
            continue;
        }
        for (predecessor, sibling) in targeted_affine_predecessors(expression) {
            let Some(literal_axiom) = targeted_literal_axiom(
                context,
                semantic_axioms,
                definitions,
                definition_index,
                sibling,
            ) else {
                continue;
            };
            let witness = IntegerAffineWitness {
                root: predecessor.clone(),
                target: target.clone(),
                definition_axioms: vec![definition_index],
                literal_axioms: vec![literal_axiom],
            };
            let Ok(checked) = check_integer_affine_witness(context, semantic_axioms, &witness)
            else {
                continue;
            };
            for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
                let root_bounds: Vec<ProofNode> = match fact {
                    Proposition::LessOrEqual(left, right)
                        if left == predecessor || right == predecessor =>
                    {
                        vec![citation.proof(fact)]
                    }
                    // A landed literal equality on the predecessor also fixes
                    // both oriented endpoints; map each through the witness so
                    // `v = k` reaches `k*lit <= target` and `target <= k*lit`.
                    Proposition::Equal(..) => {
                        let ScalarType::Integer(predecessor_type) = predecessor.scalar_type()
                        else {
                            continue;
                        };
                        [true, false]
                            .into_iter()
                            .filter_map(|orientation| {
                                oriented_landed_literal_endpoint(
                                    predecessor_type,
                                    predecessor,
                                    orientation,
                                    citation.proof(fact),
                                )
                            })
                            .collect()
                    }
                    _ => continue,
                };
                for root_bound in root_bounds {
                    let Ok(mapped) = map_integer_affine_bound(&checked, &root_bound.conclusion)
                    else {
                        continue;
                    };
                    let same_oriented_target = match &mapped {
                        Proposition::LessOrEqual(_, actual_target) if lower => {
                            actual_target == target
                        }
                        Proposition::LessOrEqual(actual_target, _) if !lower => {
                            actual_target == target
                        }
                        _ => false,
                    };
                    if same_oriented_target
                        && !proofs
                            .iter()
                            .any(|proof: &ProofNode| proof.conclusion == mapped)
                    {
                        proofs.push(ProofNode {
                            conclusion: mapped,
                            rule: ProofRule::IntegerAffineBound {
                                root_bound: Box::new(root_bound),
                                witness: witness.clone(),
                            },
                        });
                    }
                }
            }
        }
    }
    proofs
}

pub(crate) fn direct_cited_operand_endpoints(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<ProofNode> {
    let mut proofs = Vec::new();
    // Range contracts retain both endpoints in one conjunction. Project its
    // unconditional leaves with elimination proofs; flattening assumptions
    // would change citation positions, and selecting a disjunction would be unsound.
    for projected in projected_facts(assumptions, semantic_axioms) {
        let fact = projected.proposition;
        // A landed literal equality also fixes both oriented endpoints: the
        // kernel's order substitution turns `operand = k` into `k <= operand`
        // or `operand <= k`, so exact operands still orient their operand pair.
        if let Some(proof) =
            oriented_landed_literal_endpoint(integer_type, operand, lower, projected.proof())
        {
            proofs.push(proof);
        }
        let matches = match fact {
            Proposition::Equal(left, right) => {
                (left == operand
                    && right
                        .integer_value()
                        .is_some_and(|(actual, _)| actual == integer_type))
                    || (right == operand
                        && left
                            .integer_value()
                            .is_some_and(|(actual, _)| actual == integer_type))
            }
            Proposition::LessOrEqual(left, right) if lower => {
                right == operand
                    && left
                        .integer_value()
                        .is_some_and(|(actual, _)| actual == integer_type)
            }
            Proposition::LessOrEqual(left, right) => {
                left == operand
                    && right
                        .integer_value()
                        .is_some_and(|(actual, _)| actual == integer_type)
            }
            _ => false,
        };
        if matches
            && !proofs
                .iter()
                .any(|proof: &ProofNode| &proof.conclusion == fact)
        {
            proofs.push(projected.proof());
        }
    }
    proofs
}

fn oriented_landed_literal_endpoint(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    equality: ProofNode,
) -> Option<ProofNode> {
    let Proposition::Equal(left, right) = &equality.conclusion else {
        return None;
    };
    let literal = if left == operand {
        right
    } else if right == operand {
        left
    } else {
        return None;
    };
    let (actual_type, _) = literal.integer_value()?;
    if actual_type != integer_type {
        return None;
    }
    let closed =
        closed_integer_relation(Proposition::LessOrEqual(literal.clone(), literal.clone()))?;
    let conclusion = if lower {
        Proposition::LessOrEqual(literal.clone(), operand.clone())
    } else {
        Proposition::LessOrEqual(operand.clone(), literal.clone())
    };
    Some(ProofNode {
        conclusion,
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(closed),
            equality: Box::new(equality),
            endpoint: usize::from(lower),
        },
    })
}

pub(crate) fn prove_multiply_affine_operand_endpoints(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Vec<ProofNode> {
    if operand.scalar_type() != ScalarType::Integer(integer_type) {
        return Vec::new();
    }
    let mut proofs = Vec::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, right) = fact else {
            continue;
        };
        for root in [left, right] {
            if !matches!(root, ScalarTerm::Value { .. }) {
                continue;
            }
            let Some(witness) = targeted_multiply_operand_witness(
                context,
                semantic_axioms,
                definitions,
                root,
                operand,
            ) else {
                continue;
            };
            let Some(checked) =
                check_integer_affine_witness(context, semantic_axioms, &witness).ok()
            else {
                continue;
            };
            let Some(mapped) = map_integer_affine_bound(&checked, fact).ok() else {
                continue;
            };
            let same_oriented_operand = match &mapped {
                Proposition::LessOrEqual(_, mapped_operand) if lower => mapped_operand == operand,
                Proposition::LessOrEqual(mapped_operand, _) if !lower => mapped_operand == operand,
                _ => false,
            };
            if same_oriented_operand
                && !proofs
                    .iter()
                    .any(|proof: &ProofNode| proof.conclusion == mapped)
            {
                proofs.push(ProofNode {
                    conclusion: mapped,
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(citation.proof(fact)),
                        witness,
                    },
                });
            }
        }
    }
    proofs
}
