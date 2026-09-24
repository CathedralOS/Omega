//! Correlated multiply relations and their operand endpoints.

use crate::proofs::nonzero_divisor_certificate::affine_custody::DefinitionIndex;
use crate::proofs::nonzero_divisor_certificate::integer_evidence::cited_facts;
use crate::proofs::nonzero_divisor_certificate::integer_selection::bound;
use crate::proofs::nonzero_divisor_certificate::integer_selection::multiply::targeted_endpoints::targeted_operand_endpoints;
use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_correlated_multiply_relation(
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
    for positive in [true, false] {
        if !positive && integer_type.sign() == IntegerSign::Unsigned {
            continue;
        }
        let expected_endpoint = if lower {
            integer_type.minimum_value()
        } else {
            integer_type.maximum_value()
        };
        if !has_correlated_multiply_candidate(
            integer_type,
            left,
            right,
            lower,
            positive,
            expected_endpoint,
            assumptions,
            semantic_axioms,
        ) {
            continue;
        }
        let sign_goal = if positive {
            Proposition::LessOrEqual(
                ScalarTerm::integer(
                    integer_type,
                    match integer_type.sign() {
                        IntegerSign::Signed => IntegerValue::Signed(1),
                        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
                    },
                )
                .ok()?,
                right.clone(),
            )
        } else {
            Proposition::LessOrEqual(
                right.clone(),
                ScalarTerm::integer(integer_type, IntegerValue::Signed(-2)).ok()?,
            )
        };
        let Some(sign_proof) = bound::prove(
            context,
            &sign_goal,
            assumptions,
            semantic_axioms,
            definitions,
        ) else {
            continue;
        };
        for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
            let quotient = match fact {
                Proposition::LessOrEqual(quotient, actual_left)
                    if lower == positive && actual_left == left =>
                {
                    quotient
                }
                Proposition::LessOrEqual(actual_left, quotient)
                    if lower != positive && actual_left == left =>
                {
                    quotient
                }
                _ => continue,
            };
            let ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left: endpoint,
                right: divide_right,
            } = quotient
            else {
                continue;
            };
            if *scalar_type != integer_type
                || divide_right.as_ref() != right
                || endpoint.integer_value() != Some((integer_type, expected_endpoint))
            {
                continue;
            }
            let evidence = ProofNode {
                conclusion: Proposition::Conjunction(vec![
                    sign_proof.conclusion.clone(),
                    fact.clone(),
                ]),
                rule: ProofRule::ConjunctionIntroduction(vec![
                    sign_proof.clone(),
                    citation.proof(fact),
                ]),
            };
            let witness = IntegerAffineWitness {
                root: quotient.clone(),
                target: target.clone(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            };
            let Some(form) = check_integer_affine_witness(context, semantic_axioms, &witness).ok()
            else {
                continue;
            };
            let Some(mapped) = map_integer_affine_bound(&form, &evidence.conclusion).ok() else {
                continue;
            };
            if &mapped != goal {
                continue;
            }
            return Some(ProofNode {
                conclusion: mapped,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(evidence),
                    witness,
                },
            });
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
                let ScalarTerm::ExactIntegerDivide {
                    scalar_type,
                    left: endpoint,
                    right: divide_right,
                } = expression
                else {
                    continue;
                };
                if *root_type != integer_type
                    || *scalar_type != integer_type
                    || divide_right.as_ref() != right
                {
                    continue;
                }
                let literal_axiom = if endpoint.integer_value()
                    == Some((integer_type, expected_endpoint))
                {
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
                let bound_goal = if lower == positive {
                    Proposition::LessOrEqual(root.clone(), left.clone())
                } else {
                    Proposition::LessOrEqual(left.clone(), root.clone())
                };
                let Some(bound_proof) = bound::prove(
                    context,
                    &bound_goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                ) else {
                    continue;
                };
                let evidence = ProofNode {
                    conclusion: Proposition::Conjunction(vec![
                        sign_proof.conclusion.clone(),
                        bound_proof.conclusion.clone(),
                    ]),
                    rule: ProofRule::ConjunctionIntroduction(vec![sign_proof.clone(), bound_proof]),
                };
                let witness = IntegerAffineWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms: vec![index],
                    literal_axioms: vec![literal_axiom],
                };
                let Some(form) =
                    check_integer_affine_witness(context, semantic_axioms, &witness).ok()
                else {
                    continue;
                };
                let Some(mapped) = map_integer_affine_bound(&form, &evidence.conclusion).ok()
                else {
                    continue;
                };
                if &mapped != goal {
                    continue;
                }
                return Some(ProofNode {
                    conclusion: mapped,
                    rule: ProofRule::IntegerAffineBound {
                        root_bound: Box::new(evidence),
                        witness,
                    },
                });
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn has_correlated_multiply_candidate(
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
    lower: bool,
    positive: bool,
    expected_endpoint: IntegerValue,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    if cited_facts(assumptions, semantic_axioms).any(|(_, fact)| {
        let quotient = match fact {
            Proposition::LessOrEqual(quotient, actual_left)
                if lower == positive && actual_left == left =>
            {
                quotient
            }
            Proposition::LessOrEqual(actual_left, quotient)
                if lower != positive && actual_left == left =>
            {
                quotient
            }
            _ => return false,
        };
        matches!(
            quotient,
            ScalarTerm::ExactIntegerDivide {
                scalar_type,
                left: endpoint,
                right: divide_right,
            } if *scalar_type == integer_type
                && divide_right.as_ref() == right
                && endpoint.integer_value() == Some((integer_type, expected_endpoint))
        )
    }) {
        return true;
    }

    semantic_axioms.iter().enumerate().any(|(index, axiom)| {
        let Proposition::Equal(equal_left, equal_right) = axiom else {
            return false;
        };
        [(equal_left, equal_right), (equal_right, equal_left)]
            .into_iter()
            .any(|(root, expression)| {
                let ScalarTerm::Value {
                    scalar_type: ScalarType::Integer(root_type),
                    ..
                } = root
                else {
                    return false;
                };
                let ScalarTerm::ExactIntegerDivide {
                    scalar_type,
                    left: endpoint,
                    right: divide_right,
                } = expression
                else {
                    return false;
                };
                if *root_type != integer_type
                    || *scalar_type != integer_type
                    || divide_right.as_ref() != right
                {
                    return false;
                }
                if endpoint.integer_value() == Some((integer_type, expected_endpoint)) {
                    return true;
                }
                semantic_axioms[..index].iter().any(|landing| {
                    let Proposition::Equal(landing_left, landing_right) = landing else {
                        return false;
                    };
                    let literal = if landing_left == endpoint.as_ref() {
                        landing_right
                    } else if landing_right == endpoint.as_ref() {
                        landing_left
                    } else {
                        return false;
                    };
                    literal.integer_value() == Some((integer_type, expected_endpoint))
                })
            })
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn multiply_operand_endpoints(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Vec<ProofNode> {
    let mut proofs = targeted_operand_endpoints(
        context,
        integer_type,
        operand,
        lower,
        assumptions,
        semantic_axioms,
        definitions,
    );
    if proofs.is_empty() {
        proofs.push(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(proof_admission::PrimitiveJudgment::Truth),
        });
    }
    proofs
}
