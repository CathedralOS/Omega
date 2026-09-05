//! Established flat endpoint-candidate selection for direct exact add.

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
};

use super::super::dispatch;
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
    let mut global_candidates =
        dispatch::add_endpoint_candidates(integer_type, left, lower, assumptions, semantic_axioms);
    for candidate in
        dispatch::add_endpoint_candidates(integer_type, right, lower, assumptions, semantic_axioms)
    {
        if !global_candidates.contains(&candidate) {
            global_candidates.push(candidate);
        }
    }
    let mut candidates = operand_candidates(integer_type, left, assumptions, semantic_axioms)
        .into_iter()
        .filter(|candidate| global_candidates.contains(candidate))
        .collect::<Vec<_>>();
    for candidate in operand_candidates(integer_type, right, assumptions, semantic_axioms) {
        let Some(candidate) = complement(integer_type, candidate, lower) else {
            continue;
        };
        if global_candidates.contains(&candidate) && !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    let carrier = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    if global_candidates.contains(&carrier) && !candidates.contains(&carrier) {
        candidates.push(carrier);
    }
    for candidate in global_candidates {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    }
    for left_bound in candidates {
        let Some(left_proof) = dispatch::prove_add_operand_endpoint(
            context,
            integer_type,
            left,
            left_bound,
            lower,
            assumptions,
            semantic_axioms,
            definitions,
        ) else {
            continue;
        };
        let Some(right_bound) = exact_value(integer_type, right, assumptions, semantic_axioms)
            .or_else(|| complement(integer_type, left_bound, lower))
        else {
            continue;
        };
        let Some(right_proof) = dispatch::prove_add_operand_endpoint(
            context,
            integer_type,
            right,
            right_bound,
            lower,
            assumptions,
            semantic_axioms,
            definitions,
        ) else {
            continue;
        };
        let evidence = ProofNode {
            conclusion: Proposition::Conjunction(vec![
                left_proof.conclusion.clone(),
                right_proof.conclusion.clone(),
            ]),
            rule: ProofRule::ConjunctionIntroduction(vec![left_proof, right_proof]),
        };
        let witness = IntegerAffineWitness {
            root: left.clone(),
            target: target.clone(),
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        };
        let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
        let mapped = map_integer_affine_bound(&form, &evidence.conclusion).ok()?;
        let mapped_proof = ProofNode {
            conclusion: mapped.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(evidence),
                witness,
            },
        };
        if &mapped == goal {
            return Some(mapped_proof);
        }
        if let Some(proof) = dispatch::relax_math_bound(goal, mapped_proof) {
            return Some(proof);
        }
    }
    None
}

fn exact_value(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<IntegerValue> {
    if let Some((actual, value)) = operand.integer_value() {
        return (actual == integer_type).then_some(value);
    }
    cited_facts(assumptions, semantic_axioms).find_map(|(_, fact)| {
        let Proposition::Equal(left, right) = fact else {
            return None;
        };
        let (actual, value) = if left == operand {
            right.integer_value()
        } else if right == operand {
            left.integer_value()
        } else {
            None
        }?;
        (actual == integer_type).then_some(value)
    })
}

fn operand_candidates(
    integer_type: IntegerType,
    operand: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Vec<IntegerValue> {
    let mut candidates = Vec::new();
    if let Some((actual, value)) = operand.integer_value()
        && actual == integer_type
    {
        candidates.push(value);
    }
    for (_, fact) in cited_facts(assumptions, semantic_axioms) {
        let (left, right) = match fact {
            Proposition::Equal(left, right) | Proposition::LessOrEqual(left, right) => {
                (left, right)
            }
            _ => continue,
        };
        let literal = if left == operand {
            right
        } else if right == operand {
            left
        } else {
            continue;
        };
        let Some((actual, value)) = literal.integer_value() else {
            continue;
        };
        let Some(value) = (if actual == integer_type {
            Some(value)
        } else {
            actual.exact_cast_value_to(integer_type, value)
        }) else {
            continue;
        };
        if !candidates.contains(&value) {
            candidates.push(value);
        }
    }
    candidates
}

fn complement(
    integer_type: IntegerType,
    operand_bound: IntegerValue,
    lower: bool,
) -> Option<IntegerValue> {
    match (integer_type.sign(), operand_bound) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => {
            let IntegerValue::Signed(carrier) = (if lower {
                integer_type.minimum_value()
            } else {
                integer_type.maximum_value()
            }) else {
                unreachable!("signed carrier has signed endpoints")
            };
            Some(IntegerValue::Signed(
                carrier.checked_sub(value).unwrap_or(carrier),
            ))
        }
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) if !lower => {
            let IntegerValue::Unsigned(carrier) = integer_type.maximum_value() else {
                unreachable!("unsigned carrier has unsigned endpoint")
            };
            Some(IntegerValue::Unsigned(carrier.checked_sub(value)?))
        }
        _ => None,
    }
}
