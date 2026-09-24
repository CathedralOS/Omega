//! Targeted multiply operand witnesses, prefix boundaries and axioms.

use crate::proofs::nonzero_divisor_certificate::affine_custody::DefinitionIndex;
use crate::proofs::nonzero_divisor_certificate::integer_selection::multiply::MAX_TARGETED_OPERAND_WITNESS_DEFINITIONS;
use crate::proofs::nonzero_divisor_certificate::integer_selection::{bound, range};
use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_integer_affine_witness,
    map_integer_affine_bound,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};

pub(crate) fn targeted_multiply_operand_witness(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
) -> Option<IntegerAffineWitness> {
    fn visit(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        definitions: &DefinitionIndex,
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
        if reverse_definitions.len() == MAX_TARGETED_OPERAND_WITNESS_DEFINITIONS {
            return None;
        }
        for &index in definitions
            .output_definitions_before(current, before)
            .iter()
            .rev()
        {
            let Proposition::Equal(equal_left, equal_right) = &semantic_axioms[index] else {
                continue;
            };
            for (defined, expression) in [(equal_left, equal_right), (equal_right, equal_left)] {
                if defined != current || !matches!(defined, ScalarTerm::Value { .. }) {
                    continue;
                }
                for (predecessor, sibling) in targeted_affine_predecessors(expression) {
                    let Some(literal_axiom) = targeted_literal_axiom(
                        context,
                        semantic_axioms,
                        definitions,
                        index,
                        sibling,
                    ) else {
                        continue;
                    };
                    reverse_definitions.push(index);
                    reverse_literals.push(literal_axiom);
                    if let Some(witness) = visit(
                        context,
                        semantic_axioms,
                        definitions,
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
        definitions,
        root,
        target,
        target,
        semantic_axioms.len(),
        &mut Vec::new(),
        &mut Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn prove_targeted_affine_prefix_endpoint(
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
        let Some(checked) = check_integer_affine_witness(context, semantic_axioms, witness).ok()
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

pub(crate) fn prove_targeted_remainder_prefix_endpoint(
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

pub(crate) fn targeted_multiply_operand_prefix_witnesses(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    target: &ScalarTerm,
) -> Vec<IntegerAffineWitness> {
    fn visit(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        definitions: &DefinitionIndex,
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
        for &index in definitions
            .output_definitions_before(current, before)
            .iter()
            .rev()
        {
            let Proposition::Equal(equal_left, equal_right) = &semantic_axioms[index] else {
                continue;
            };
            for (defined, expression) in [(equal_left, equal_right), (equal_right, equal_left)] {
                if defined != current || !matches!(defined, ScalarTerm::Value { .. }) {
                    continue;
                }
                for (predecessor, sibling) in targeted_affine_predecessors(expression) {
                    let Some(literal_axiom) = targeted_literal_axiom(
                        context,
                        semantic_axioms,
                        definitions,
                        index,
                        sibling,
                    ) else {
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
                            definitions,
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
        definitions,
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
pub(crate) enum TargetedPrefixBoundary {
    Cast,
    Remainder,
}

pub(crate) fn targeted_prefix_boundary(
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
    checked: &proof_admission::CheckedIntegerAffineForm,
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
    let root_lower = if checked.coefficient() < 0 {
        !lower
    } else {
        lower
    };
    let mut low = targeted_integer_as_i128(integer_type.minimum_value())?;
    let mut high = targeted_integer_as_i128(integer_type.maximum_value())?;
    let valid = |candidate| {
        let candidate = targeted_scalar_from_i128(integer_type, candidate)?;
        let root_goal = if root_lower {
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
    if root_lower {
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
    Some(if root_lower {
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

pub(crate) fn targeted_affine_predecessors(
    expression: &ScalarTerm,
) -> Vec<(&ScalarTerm, &ScalarTerm)> {
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

pub(crate) fn targeted_literal_axiom(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    definition_index: usize,
    sibling: &ScalarTerm,
) -> Option<Option<usize>> {
    if sibling.integer_value().is_some() {
        return Some(None);
    }
    matches!(sibling, ScalarTerm::Value { .. }).then_some(())?;
    definitions
        .output_definitions_before(sibling, definition_index)
        .iter()
        .rev()
        .find_map(|&index| {
            let proposition = &semantic_axioms[index];
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
