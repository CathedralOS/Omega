//! Targeted operand endpoints through cast prefixes.

use crate::proofs::nonzero_divisor_certificate::affine_custody::DefinitionIndex;
use crate::proofs::nonzero_divisor_certificate::integer_evidence::cited_facts;
use crate::proofs::nonzero_divisor_certificate::integer_selection::dispatch::add_endpoint_candidates;
use crate::proofs::nonzero_divisor_certificate::integer_selection::multiply::direct_endpoints::{
    direct_cited_operand_endpoints, prove_direct_computed_multiply_endpoints,
    prove_multiply_affine_operand_endpoints,
};
use crate::proofs::nonzero_divisor_certificate::integer_selection::multiply::targeted_witnesses::{
    TargetedPrefixBoundary, prove_targeted_affine_prefix_endpoint,
    prove_targeted_remainder_prefix_endpoint, targeted_multiply_operand_prefix_witnesses,
    targeted_multiply_operand_witness, targeted_prefix_boundary,
};
use crate::proofs::nonzero_divisor_certificate::integer_selection::range;
use crate::proofs::nonzero_divisor_certificate::{cast_custody, cast_selection};
use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType};

#[allow(clippy::too_many_arguments)]
pub(crate) fn targeted_operand_endpoints(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Vec<ProofNode> {
    if matches!(
        operand,
        ScalarTerm::Integer { scalar_type, .. } if *scalar_type == integer_type
    ) {
        return vec![ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(proof_admission::PrimitiveJudgment::Truth),
        }];
    }
    let mut proofs =
        direct_cited_operand_endpoints(integer_type, operand, lower, assumptions, semantic_axioms);
    for proof in direct_cast_operand_endpoints(
        context,
        integer_type,
        operand,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
    ) {
        if !proofs
            .iter()
            .any(|existing| existing.conclusion == proof.conclusion)
        {
            proofs.push(proof);
        }
    }
    // A total-image definition (exact remainder or nonzero exact divide)
    // already bounds its own image, so such an operand carries oriented
    // endpoints without any external contract.
    for proof in range::target_bounds(context, operand, semantic_axioms) {
        let same_oriented_operand = match &proof.conclusion {
            Proposition::LessOrEqual(_, actual_operand) if lower => actual_operand == operand,
            Proposition::LessOrEqual(actual_operand, _) if !lower => actual_operand == operand,
            _ => false,
        };
        if same_oriented_operand
            && !proofs
                .iter()
                .any(|existing| existing.conclusion == proof.conclusion)
        {
            proofs.push(proof);
        }
    }
    // A multiply-defined operand keeps its own definition witness; its landed
    // literal predecessors map through it into oriented operand endpoints.
    for proof in prove_direct_computed_multiply_endpoints(
        context,
        operand,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
    ) {
        if !proofs
            .iter()
            .any(|existing| existing.conclusion == proof.conclusion)
        {
            proofs.push(proof);
        }
    }
    let affine_proofs = prove_multiply_affine_operand_endpoints(
        context,
        integer_type,
        operand,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
    );
    let candidates =
        add_endpoint_candidates(integer_type, operand, lower, assumptions, semantic_axioms);
    let prefix_witnesses = if affine_proofs.is_empty() {
        {
            targeted_multiply_operand_prefix_witnesses(
                context,
                semantic_axioms,
                definitions,
                operand,
            )
        }
    } else {
        Default::default()
    };
    for affine_proof in affine_proofs {
        if !proofs
            .iter()
            .any(|proof| proof.conclusion == affine_proof.conclusion)
        {
            proofs.push(affine_proof);
        }
    }
    let mut prefix_endpoint_proved = false;
    for witness in &prefix_witnesses {
        for proof in prove_targeted_cast_prefix_endpoints(
            context,
            operand,
            witness,
            lower,
            assumptions,
            semantic_axioms,
            definitions,
        ) {
            if !proofs
                .iter()
                .any(|existing| existing.conclusion == proof.conclusion)
            {
                proofs.push(proof);
                prefix_endpoint_proved = true;
            }
        }
    }
    if let Some(proof) = prefix_witnesses.iter().find_map(|witness| {
        prove_targeted_remainder_prefix_endpoint(context, operand, witness, lower, semantic_axioms)
    }) {
        proofs.push(proof);
        prefix_endpoint_proved = true;
    }
    if !prefix_witnesses.is_empty()
        && !prefix_endpoint_proved
        && let Some(proof) = candidates.iter().copied().find_map(|bound_value| {
            prove_targeted_affine_prefix_endpoint(
                context,
                integer_type,
                operand,
                &prefix_witnesses,
                bound_value,
                lower,
                assumptions,
                semantic_axioms,
                definitions,
            )
        })
    {
        proofs.push(proof);
    }
    proofs
}

fn prove_targeted_cast_prefix_endpoints(
    context: &PropositionContext,
    operand: &ScalarTerm,
    witness: &IntegerAffineWitness,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Vec<ProofNode> {
    let first_definition = witness
        .definition_axioms
        .first()
        .copied()
        .unwrap_or(semantic_axioms.len());
    if !matches!(
        targeted_prefix_boundary(semantic_axioms, &witness.root, first_definition),
        Some(TargetedPrefixBoundary::Cast)
    ) {
        return Vec::new();
    }
    let Ok(checked) = check_integer_affine_witness(context, semantic_axioms, witness) else {
        return Vec::new();
    };
    let root_lower = if checked.coefficient() < 0 {
        !lower
    } else {
        lower
    };
    let Some((source, _)) = cast_custody::source_root(&witness.root, semantic_axioms) else {
        return Vec::new();
    };
    let (ScalarType::Integer(source_type), ScalarType::Integer(cast_type)) =
        (source.scalar_type(), witness.root.scalar_type())
    else {
        return Vec::new();
    };
    let mut proofs = Vec::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, right) = fact else {
            continue;
        };
        for root in [left, right] {
            if !matches!(root, ScalarTerm::Value { .. })
                || root.scalar_type() != ScalarType::Integer(source_type)
            {
                continue;
            }
            let root_proof = citation.proof(fact);
            let source_proof = if root == &source {
                root_proof
            } else {
                let Some(source_witness) = targeted_multiply_operand_witness(
                    context,
                    semantic_axioms,
                    definitions,
                    root,
                    &source,
                ) else {
                    continue;
                };
                let Ok(checked_source) =
                    check_integer_affine_witness(context, semantic_axioms, &source_witness)
                else {
                    continue;
                };
                let Ok(mapped_source) =
                    map_integer_affine_bound(&checked_source, &root_proof.conclusion)
                else {
                    continue;
                };
                ProofNode {
                    conclusion: mapped_source,
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(root_proof),
                        witness: source_witness,
                    },
                }
            };
            let endpoint = match &source_proof.conclusion {
                Proposition::LessOrEqual(endpoint, actual_source)
                    if root_lower && actual_source == &source =>
                {
                    endpoint
                }
                Proposition::LessOrEqual(actual_source, endpoint)
                    if !root_lower && actual_source == &source =>
                {
                    endpoint
                }
                _ => continue,
            };
            let Some((actual_type, value)) = endpoint.integer_value() else {
                continue;
            };
            if actual_type != source_type {
                continue;
            }
            let Some(value) = source_type.exact_cast_value_to(cast_type, value) else {
                continue;
            };
            let Ok(endpoint) = ScalarTerm::integer(cast_type, value) else {
                continue;
            };
            let cast_goal = if root_lower {
                Proposition::LessOrEqual(endpoint, witness.root.clone())
            } else {
                Proposition::LessOrEqual(witness.root.clone(), endpoint)
            };
            let Some(cast_proof) = cast_custody::prove_from_root(
                context,
                &cast_goal,
                assumptions,
                semantic_axioms,
                &source,
                source_proof,
            ) else {
                continue;
            };
            let Ok(mapped) = map_integer_affine_bound(&checked, &cast_proof.conclusion) else {
                continue;
            };
            let same_oriented_operand = match &mapped {
                Proposition::LessOrEqual(_, actual_operand) if lower => actual_operand == operand,
                Proposition::LessOrEqual(actual_operand, _) if !lower => actual_operand == operand,
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
                        root_bound: Box::new(cast_proof),
                        witness: witness.clone(),
                    },
                });
            }
        }
    }
    proofs
}

fn direct_cast_operand_endpoints(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Vec<ProofNode> {
    if !semantic_axioms.iter().any(|axiom| {
        matches!(
            axiom,
            Proposition::Equal(
                output,
                ScalarTerm::IntegerExactCast { .. } | ScalarTerm::IntegerWiden { .. },
            ) if output == operand
        )
    }) {
        return Vec::new();
    }
    let Some((root, _)) = cast_custody::source_root(operand, semantic_axioms) else {
        return Vec::new();
    };
    let ScalarType::Integer(root_type) = root.scalar_type() else {
        return Vec::new();
    };
    let mut proofs = Vec::new();
    let root_proofs = prove_direct_computed_multiply_endpoints(
        context,
        &root,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
    );
    for root_proof in root_proofs {
        let endpoint = match &root_proof.conclusion {
            Proposition::LessOrEqual(endpoint, actual_root) if lower && actual_root == &root => {
                endpoint
            }
            Proposition::LessOrEqual(actual_root, endpoint) if !lower && actual_root == &root => {
                endpoint
            }
            _ => continue,
        };
        let Some((actual_type, value)) = endpoint.integer_value() else {
            continue;
        };
        if actual_type != root_type {
            continue;
        }
        let Some(value) = root_type.exact_cast_value_to(integer_type, value) else {
            continue;
        };
        let Ok(literal) = ScalarTerm::integer(integer_type, value) else {
            continue;
        };
        let goal = if lower {
            Proposition::LessOrEqual(literal, operand.clone())
        } else {
            Proposition::LessOrEqual(operand.clone(), literal)
        };
        if let Some(proof) = cast_custody::prove_from_root(
            context,
            &goal,
            assumptions,
            semantic_axioms,
            &root,
            root_proof,
        ) {
            proofs.push(proof);
        }
    }
    let mut candidates = Vec::new();
    for (_, fact) in cited_facts(assumptions, semantic_axioms) {
        let literal = match fact {
            Proposition::Equal(left, right) if left == &root => right,
            Proposition::Equal(left, right) if right == &root => left,
            Proposition::LessOrEqual(left, right) if lower && right == &root => left,
            Proposition::LessOrEqual(left, right) if !lower && left == &root => right,
            _ => continue,
        };
        let Some((actual_type, value)) = literal.integer_value() else {
            continue;
        };
        if actual_type != root_type {
            continue;
        }
        let Some(value) = root_type.exact_cast_value_to(integer_type, value) else {
            continue;
        };
        if !candidates.contains(&value) {
            candidates.push(value);
        }
    }
    let root_carrier = if lower {
        root_type.minimum_value()
    } else {
        root_type.maximum_value()
    };
    if let Some(value) = root_type.exact_cast_value_to(integer_type, root_carrier)
        && !candidates.contains(&value)
    {
        candidates.push(value);
    }
    let carrier = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    if !candidates.contains(&carrier) {
        candidates.push(carrier);
    }
    proofs.extend(candidates.into_iter().filter_map(|candidate| {
        let literal = ScalarTerm::integer(integer_type, candidate).ok()?;
        let goal = if lower {
            Proposition::LessOrEqual(literal, operand.clone())
        } else {
            Proposition::LessOrEqual(operand.clone(), literal)
        };
        cast_selection::prove(context, &goal, assumptions, semantic_axioms)
    }));
    proofs
}
