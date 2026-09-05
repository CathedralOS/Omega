//! Canonical prior-definition and direct-literal selection.

use proof_admission::ProofNode;
use semantic_vocabulary::{IntegerType, IntegerValue, Proposition, ScalarTerm};

use super::model::cited_proof;
use crate::nonzero_divisor_certificate::affine_custody::DefinitionIndex;

pub(super) struct LiteralLanding {
    pub(super) value: IntegerValue,
    pub(super) semantic_index: Option<usize>,
}

pub(super) struct ExactAddDefinition<'a> {
    pub(super) index: usize,
    pub(super) expression: &'a ScalarTerm,
    pub(super) left: &'a ScalarTerm,
    pub(super) right: &'a ScalarTerm,
}

pub(super) fn direct_literal(
    operand: &ScalarTerm,
    integer_type: IntegerType,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    cutoff: usize,
) -> Option<(IntegerValue, ProofNode)> {
    let mut matches =
        assumptions
            .iter()
            .enumerate()
            .filter_map(|(index, proposition)| {
                literal_equality(operand, integer_type, proposition)
                    .map(|value| (value, cited_proof(proposition, Some(index), None)))
            })
            .chain(semantic_axioms[..cutoff].iter().enumerate().filter_map(
                |(index, proposition)| {
                    literal_equality(operand, integer_type, proposition)
                        .map(|value| (value, cited_proof(proposition, None, Some(index))))
                },
            ));
    let result = matches.next()?;
    matches.next().is_none().then_some(result)
}

pub(super) fn semantic_literal_landing(
    operand: &ScalarTerm,
    integer_type: IntegerType,
    semantic_axioms: &[Proposition],
    cutoff: usize,
) -> Option<LiteralLanding> {
    if let Some((actual, value)) = operand.integer_value() {
        return (actual == integer_type).then_some(LiteralLanding {
            value,
            semantic_index: None,
        });
    }
    let mut matches =
        semantic_axioms[..cutoff]
            .iter()
            .enumerate()
            .filter_map(|(index, proposition)| {
                literal_equality(operand, integer_type, proposition).map(|value| LiteralLanding {
                    value,
                    semantic_index: Some(index),
                })
            });
    let result = matches.next()?;
    matches.next().is_none().then_some(result)
}

pub(super) fn exact_add<'a>(
    operand: &ScalarTerm,
    integer_type: IntegerType,
    semantic_axioms: &'a [Proposition],
    definitions: &DefinitionIndex,
    cutoff: usize,
) -> Option<ExactAddDefinition<'a>> {
    let mut matches = definitions
        .output_definitions_before(operand, cutoff)
        .iter()
        .filter_map(|&index| {
            let proposition = semantic_axioms.get(index)?;
            let Proposition::Equal(first, second) = proposition else {
                return None;
            };
            let expression = if first == operand {
                second
            } else if second == operand {
                first
            } else {
                return None;
            };
            let ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } = expression
            else {
                return None;
            };
            (*scalar_type == integer_type
                && left.scalar_type() == operand.scalar_type()
                && right.scalar_type() == operand.scalar_type())
            .then_some(ExactAddDefinition {
                index,
                expression,
                left,
                right,
            })
        });
    let result = matches.next()?;
    matches.next().is_none().then_some(result)
}

fn literal_equality(
    operand: &ScalarTerm,
    integer_type: IntegerType,
    proposition: &Proposition,
) -> Option<IntegerValue> {
    let Proposition::Equal(left, right) = proposition else {
        return None;
    };
    let literal = if left == operand {
        right
    } else if right == operand {
        left
    } else {
        return None;
    };
    let (actual, value) = literal.integer_value()?;
    (actual == integer_type).then_some(value)
}
