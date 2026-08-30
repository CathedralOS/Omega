//! Exact-multiplication canonical certificate production.

use psi_core::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarTerm, ScalarType,
};
use psi_proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};

use super::super::affine_custody::DefinitionIndex;
use super::super::integer_evidence::cited_facts;
use super::dispatch::{
    add_endpoint_candidates, lower_add_math_leaf, prove_add_operand_endpoint, relax_math_bound,
};
use super::{bound, range};

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
    if integer_type.carrier() != psi_core::IntegerCarrier::Fixed {
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

#[allow(clippy::too_many_arguments)]
fn prove_correlated_multiply_relation(
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
fn multiply_operand_endpoints(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Vec<ProofNode> {
    let affine_proofs = prove_multiply_affine_operand_endpoints(
        context,
        integer_type,
        operand,
        lower,
        assumptions,
        semantic_axioms,
    );
    let candidates =
        add_endpoint_candidates(integer_type, operand, lower, assumptions, semantic_axioms);
    let prefix_witnesses = affine_proofs
        .is_empty()
        .then(|| targeted_multiply_operand_prefix_witnesses(context, semantic_axioms, operand))
        .unwrap_or_default();
    let mut proofs = affine_proofs;
    if let Some(proof) = prefix_witnesses.iter().find_map(|witness| {
        prove_targeted_remainder_prefix_endpoint(context, operand, witness, lower, semantic_axioms)
    }) {
        proofs.push(proof);
    }
    if !prefix_witnesses.is_empty()
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
    if proofs.is_empty() {
        proofs.extend(candidates.iter().copied().filter_map(|bound_value| {
            prove_add_operand_endpoint(
                context,
                integer_type,
                operand,
                bound_value,
                lower,
                assumptions,
                semantic_axioms,
                definitions,
            )
        }));
    }
    proofs
}

fn prove_multiply_affine_operand_endpoints(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
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
            let Some(witness) =
                targeted_multiply_operand_witness(context, semantic_axioms, root, operand)
            else {
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

fn targeted_multiply_operand_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    target: &ScalarTerm,
) -> Option<IntegerAffineWitness> {
    fn visit(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        root: &ScalarTerm,
        target: &ScalarTerm,
        current: &ScalarTerm,
        before: usize,
        reverse_definitions: &mut Vec<usize>,
        reverse_literals: &mut Vec<Option<usize>>,
    ) -> Option<IntegerAffineWitness> {
        if current == root {
            let mut definition_axioms = reverse_definitions.clone();
            definition_axioms.reverse();
            let mut literal_axioms = reverse_literals.clone();
            literal_axioms.reverse();
            let witness = IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms,
                literal_axioms,
            };
            return check_integer_affine_witness(context, semantic_axioms, &witness)
                .is_ok()
                .then_some(witness);
        }
        if reverse_definitions.len() == 5 {
            return None;
        }
        for index in (0..before).rev() {
            let Proposition::Equal(equal_left, equal_right) = &semantic_axioms[index] else {
                continue;
            };
            for (defined, expression) in [(equal_left, equal_right), (equal_right, equal_left)] {
                if defined != current || !matches!(defined, ScalarTerm::Value { .. }) {
                    continue;
                }
                for (predecessor, sibling) in targeted_affine_predecessors(expression) {
                    let Some(literal_axiom) =
                        targeted_literal_axiom(context, semantic_axioms, index, sibling)
                    else {
                        continue;
                    };
                    reverse_definitions.push(index);
                    reverse_literals.push(literal_axiom);
                    if let Some(witness) = visit(
                        context,
                        semantic_axioms,
                        root,
                        target,
                        predecessor,
                        index,
                        reverse_definitions,
                        reverse_literals,
                    ) {
                        return Some(witness);
                    }
                    reverse_literals.pop();
                    reverse_definitions.pop();
                }
            }
        }
        None
    }

    visit(
        context,
        semantic_axioms,
        root,
        target,
        target,
        semantic_axioms.len(),
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
fn prove_targeted_affine_prefix_endpoint(
    context: &PropositionContext,
    integer_type: IntegerType,
    operand: &ScalarTerm,
    witnesses: &[IntegerAffineWitness],
    bound_value: IntegerValue,
    lower: bool,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let bound = ScalarTerm::integer(integer_type, bound_value).ok()?;
    let endpoint = if lower {
        Proposition::LessOrEqual(bound, operand.clone())
    } else {
        Proposition::LessOrEqual(operand.clone(), bound)
    };
    for witness in witnesses {
        let Some(checked) = check_integer_affine_witness(context, semantic_axioms, &witness).ok()
        else {
            continue;
        };
        let Some(root_goal) = targeted_affine_root_goal(&checked, &endpoint, lower) else {
            continue;
        };
        let first_definition = witness
            .definition_axioms
            .first()
            .copied()
            .unwrap_or(semantic_axioms.len());
        let Some(boundary) =
            targeted_prefix_boundary(semantic_axioms, &witness.root, first_definition)
        else {
            continue;
        };
        let root_proof = match boundary {
            TargetedPrefixBoundary::Cast => bound::prove(
                context,
                &root_goal,
                assumptions,
                semantic_axioms,
                definitions,
            ),
            TargetedPrefixBoundary::Remainder => continue,
        };
        let Some(root_proof) = root_proof else {
            continue;
        };
        let Some(mapped) = map_integer_affine_bound(&checked, &root_proof.conclusion).ok() else {
            continue;
        };
        if mapped == endpoint {
            return Some(ProofNode {
                conclusion: mapped,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(root_proof),
                    witness: witness.clone(),
                },
            });
        }
    }
    None
}

fn prove_targeted_remainder_prefix_endpoint(
    context: &PropositionContext,
    operand: &ScalarTerm,
    witness: &IntegerAffineWitness,
    lower: bool,
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let first_definition = witness
        .definition_axioms
        .first()
        .copied()
        .unwrap_or(semantic_axioms.len());
    if !matches!(
        targeted_prefix_boundary(semantic_axioms, &witness.root, first_definition),
        Some(TargetedPrefixBoundary::Remainder)
    ) {
        return None;
    }
    let checked = check_integer_affine_witness(context, semantic_axioms, witness).ok()?;
    range::target_bounds(context, &witness.root, semantic_axioms)
        .into_iter()
        .find_map(|root_proof| {
            let mapped = map_integer_affine_bound(&checked, &root_proof.conclusion).ok()?;
            let same_oriented_operand = match &mapped {
                Proposition::LessOrEqual(_, mapped_operand) if lower => mapped_operand == operand,
                Proposition::LessOrEqual(mapped_operand, _) if !lower => mapped_operand == operand,
                _ => false,
            };
            same_oriented_operand.then_some(ProofNode {
                conclusion: mapped,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(root_proof),
                    witness: witness.clone(),
                },
            })
        })
}

fn targeted_multiply_operand_prefix_witnesses(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    target: &ScalarTerm,
) -> Vec<IntegerAffineWitness> {
    fn visit(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        target: &ScalarTerm,
        current: &ScalarTerm,
        before: usize,
        reverse_definitions: &mut Vec<usize>,
        reverse_literals: &mut Vec<Option<usize>>,
        witnesses: &mut Vec<IntegerAffineWitness>,
    ) -> bool {
        if reverse_definitions.len() == 5 {
            return false;
        }
        for index in (0..before).rev() {
            let Proposition::Equal(equal_left, equal_right) = &semantic_axioms[index] else {
                continue;
            };
            for (defined, expression) in [(equal_left, equal_right), (equal_right, equal_left)] {
                if defined != current || !matches!(defined, ScalarTerm::Value { .. }) {
                    continue;
                }
                for (predecessor, sibling) in targeted_affine_predecessors(expression) {
                    let Some(literal_axiom) =
                        targeted_literal_axiom(context, semantic_axioms, index, sibling)
                    else {
                        continue;
                    };
                    reverse_definitions.push(index);
                    reverse_literals.push(literal_axiom);
                    let mut definition_axioms = reverse_definitions.clone();
                    definition_axioms.reverse();
                    let mut literal_axioms = reverse_literals.clone();
                    literal_axioms.reverse();
                    let witness = IntegerAffineWitness {
                        root: predecessor.clone(),
                        target: target.clone(),
                        definition_axioms,
                        literal_axioms,
                    };
                    if check_integer_affine_witness(context, semantic_axioms, &witness).is_ok() {
                        let checked_boundary =
                            match targeted_prefix_boundary(semantic_axioms, predecessor, index) {
                                Some(TargetedPrefixBoundary::Cast) => true,
                                Some(TargetedPrefixBoundary::Remainder) => {
                                    reverse_definitions.len() == 1
                                }
                                None => false,
                            };
                        if checked_boundary {
                            witnesses.push(witness.clone());
                            reverse_literals.pop();
                            reverse_definitions.pop();
                            return true;
                        }
                        if visit(
                            context,
                            semantic_axioms,
                            target,
                            predecessor,
                            index,
                            reverse_definitions,
                            reverse_literals,
                            witnesses,
                        ) {
                            reverse_literals.pop();
                            reverse_definitions.pop();
                            return true;
                        }
                    }
                    reverse_literals.pop();
                    reverse_definitions.pop();
                }
            }
        }
        false
    }

    let mut witnesses = Vec::new();
    let _ = visit(
        context,
        semantic_axioms,
        target,
        target,
        semantic_axioms.len(),
        &mut Vec::new(),
        &mut Vec::new(),
        &mut witnesses,
    );
    witnesses
}

#[derive(Clone, Copy)]
enum TargetedPrefixBoundary {
    Cast,
    Remainder,
}

fn targeted_prefix_boundary(
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    before: usize,
) -> Option<TargetedPrefixBoundary> {
    semantic_axioms[..before]
        .iter()
        .rev()
        .find_map(|proposition| {
            let Proposition::Equal(left, right) = proposition else {
                return None;
            };
            [(left, right), (right, left)]
                .into_iter()
                .find_map(|(output, expression)| {
                    if output != root {
                        return None;
                    }
                    match expression {
                        ScalarTerm::IntegerExactCast { .. } => Some(TargetedPrefixBoundary::Cast),
                        ScalarTerm::ExactIntegerRemainder { .. } => {
                            Some(TargetedPrefixBoundary::Remainder)
                        }
                        _ => None,
                    }
                })
        })
}

fn targeted_affine_root_goal(
    checked: &psi_proof_admission::CheckedIntegerAffineForm,
    endpoint: &Proposition,
    lower: bool,
) -> Option<Proposition> {
    let Proposition::LessOrEqual(endpoint_left, endpoint_right) = endpoint else {
        return None;
    };
    let endpoint_bound = if lower { endpoint_left } else { endpoint_right };
    let (_, endpoint_value) = endpoint_bound.integer_value()?;
    let endpoint_value = targeted_integer_as_i128(endpoint_value)?;
    let integer_type = checked.integer_type();
    let mut low = targeted_integer_as_i128(integer_type.minimum_value())?;
    let mut high = targeted_integer_as_i128(integer_type.maximum_value())?;
    let valid = |candidate| {
        let candidate = targeted_scalar_from_i128(integer_type, candidate)?;
        let root_goal = if lower {
            Proposition::LessOrEqual(candidate, checked.root().clone())
        } else {
            Proposition::LessOrEqual(checked.root().clone(), candidate)
        };
        let mapped = map_integer_affine_bound(checked, &root_goal).ok()?;
        let Proposition::LessOrEqual(mapped_left, mapped_right) = mapped else {
            return None;
        };
        let mapped_bound = if lower { mapped_left } else { mapped_right };
        let (_, mapped_value) = mapped_bound.integer_value()?;
        let mapped_value = targeted_integer_as_i128(mapped_value)?;
        Some(if lower {
            mapped_value >= endpoint_value
        } else {
            mapped_value <= endpoint_value
        })
    };
    if lower {
        while low < high {
            let middle = (low & high) + ((low ^ high) >> 1);
            if valid(middle)? {
                high = middle;
            } else {
                low = middle.checked_add(1)?;
            }
        }
    } else {
        while low < high {
            let difference = low ^ high;
            let middle = (low & high) + (difference >> 1) + (difference & 1);
            if valid(middle)? {
                low = middle;
            } else {
                high = middle.checked_sub(1)?;
            }
        }
    }
    if !valid(low)? {
        return None;
    }
    let root_bound = targeted_scalar_from_i128(integer_type, low)?;
    Some(if lower {
        Proposition::LessOrEqual(root_bound, checked.root().clone())
    } else {
        Proposition::LessOrEqual(checked.root().clone(), root_bound)
    })
}

fn targeted_integer_as_i128(value: IntegerValue) -> Option<i128> {
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

fn targeted_scalar_from_i128(integer_type: IntegerType, value: i128) -> Option<ScalarTerm> {
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(value).ok()?),
    };
    ScalarTerm::integer(integer_type, value).ok()
}

fn targeted_affine_predecessors(expression: &ScalarTerm) -> Vec<(&ScalarTerm, &ScalarTerm)> {
    match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. } => vec![
            (left.as_ref(), right.as_ref()),
            (right.as_ref(), left.as_ref()),
        ],
        ScalarTerm::ExactIntegerSubtract { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. } => {
            vec![(left.as_ref(), right.as_ref())]
        }
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            vec![(value.as_ref(), count.as_ref())]
        }
        _ => Vec::new(),
    }
}

fn targeted_literal_axiom(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_index: usize,
    sibling: &ScalarTerm,
) -> Option<Option<usize>> {
    if sibling.integer_value().is_some() {
        return Some(None);
    }
    matches!(sibling, ScalarTerm::Value { .. }).then_some(())?;
    semantic_axioms[..definition_index]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, proposition)| {
            context.validate(proposition).ok()?;
            let Proposition::Equal(left, right) = proposition else {
                return None;
            };
            [(left, right), (right, left)]
                .into_iter()
                .any(|(value, literal)| {
                    value == sibling
                        && literal.integer_value().is_some_and(|(integer_type, _)| {
                            ScalarType::Integer(integer_type) == sibling.scalar_type()
                        })
                })
                .then_some(Some(index))
        })
}
