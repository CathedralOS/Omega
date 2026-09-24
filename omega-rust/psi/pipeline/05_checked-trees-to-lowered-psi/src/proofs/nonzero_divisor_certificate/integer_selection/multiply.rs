//! Exact-multiplication canonical certificate production.
//!
//! This file proves a multiply selection. `correlated_relations.rs`
//! proves correlated multiply relations, `targeted_endpoints.rs` and
//! `direct_endpoints.rs` derive targeted and direct operand endpoints and
//! `targeted_witnesses.rs` builds targeted operand witnesses.

mod correlated_relations;
mod direct_endpoints;
mod targeted_endpoints;
mod targeted_witnesses;

pub(crate) use targeted_endpoints::targeted_operand_endpoints;

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext, ScalarTerm};

use super::super::affine_custody::DefinitionIndex;
use super::dispatch::{lower_add_math_leaf, relax_math_bound};
use crate::proofs::nonzero_divisor_certificate::integer_selection::multiply::correlated_relations::{prove_correlated_multiply_relation, multiply_operand_endpoints};

const MAX_TARGETED_OPERAND_WITNESS_DEFINITIONS: usize = 8;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::IntegerMathLessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let (product, carrier_bound, lower) = match (goal_left, goal_right) {
        (IntegerMathTerm::Multiply(_, _), IntegerMathTerm::IntegerLiteral(bound)) => {
            (goal_left, *bound, false)
        }
        (IntegerMathTerm::IntegerLiteral(bound), IntegerMathTerm::Multiply(_, _)) => {
            (goal_right, *bound, true)
        }
        _ => return None,
    };
    let IntegerMathTerm::Multiply(left, right) = product else {
        unreachable!("matched direct mathematical multiplication")
    };
    let integer_type = match (left.as_ref(), right.as_ref()) {
        (
            IntegerMathTerm::MathValue { source_type, .. },
            IntegerMathTerm::MathValue {
                source_type: right_type,
                ..
            },
        ) if source_type == right_type => *source_type,
        (IntegerMathTerm::MathValue { source_type, .. }, IntegerMathTerm::IntegerLiteral(_))
        | (IntegerMathTerm::IntegerLiteral(_), IntegerMathTerm::MathValue { source_type, .. }) => {
            *source_type
        }
        _ => return None,
    };
    if integer_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed {
        return None;
    }
    let left = lower_add_math_leaf(left, integer_type)?;
    let right = lower_add_math_leaf(right, integer_type)?;
    let expected_carrier_bound = if lower {
        integer_type.minimum_value()
    } else {
        integer_type.maximum_value()
    };
    if carrier_bound.as_integer_value(integer_type) != Some(expected_carrier_bound) {
        return None;
    }
    let root = if matches!(left, ScalarTerm::Value { .. }) {
        left.clone()
    } else if matches!(right, ScalarTerm::Value { .. }) {
        right.clone()
    } else {
        return None;
    };
    let target =
        ScalarTerm::exact_integer_multiply(integer_type, left.clone(), right.clone()).ok()?;
    if let Some(proof) = prove_correlated_multiply_relation(
        context,
        goal,
        integer_type,
        &left,
        &right,
        &target,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
    ) {
        return Some(proof);
    }
    let left_lower = multiply_operand_endpoints(
        context,
        integer_type,
        &left,
        true,
        assumptions,
        semantic_axioms,
        definitions,
    );
    let left_upper = multiply_operand_endpoints(
        context,
        integer_type,
        &left,
        false,
        assumptions,
        semantic_axioms,
        definitions,
    );
    let right_lower = multiply_operand_endpoints(
        context,
        integer_type,
        &right,
        true,
        assumptions,
        semantic_axioms,
        definitions,
    );
    let right_upper = multiply_operand_endpoints(
        context,
        integer_type,
        &right,
        false,
        assumptions,
        semantic_axioms,
        definitions,
    );
    let (left_first, left_second, right_first, right_second) = if lower {
        (&left_lower, &left_upper, &right_lower, &right_upper)
    } else {
        (&left_upper, &left_lower, &right_upper, &right_lower)
    };
    let witness = IntegerAffineWitness {
        root,
        target,
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    for left_first in left_first {
        for left_second in left_second {
            for right_first in right_first {
                for right_second in right_second {
                    let proofs = vec![
                        left_first.clone(),
                        left_second.clone(),
                        right_first.clone(),
                        right_second.clone(),
                    ];
                    let evidence = ProofNode {
                        conclusion: Proposition::Conjunction(
                            proofs
                                .iter()
                                .map(|proof| proof.conclusion.clone())
                                .collect(),
                        ),
                        rule: ProofRule::ConjunctionIntroduction(proofs),
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
                    if let Some(proof) = relax_math_bound(goal, mapped_proof) {
                        return Some(proof);
                    }
                }
            }
        }
    }
    None
}
