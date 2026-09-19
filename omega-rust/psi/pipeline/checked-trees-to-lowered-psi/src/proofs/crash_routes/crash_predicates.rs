//! Checked crash predicates and boolean propositions under an expansion
//! budget.

use crate::proofs::crash_routes::scalar_terms::checked_boolean_scalar_term;
use crate::proofs::{
    CheckedBooleanExpression, LoweringError, Proposition, ScalarTerm, ValueDeclaration, unsupported,
};

pub(crate) fn lower_checked_crash_predicates(
    predicates: &[CheckedBooleanExpression],
    values: &[ValueDeclaration],
) -> Result<Vec<terminal_psi::CrashPredicateTerm>, LoweringError> {
    let mut predicates = predicates
        .iter()
        .map(|predicate| {
            checked_boolean_proposition(predicate, values)
                .map(terminal_psi::CrashPredicateTerm::new)
        })
        .collect::<Result<Vec<_>, _>>()?;
    predicates.sort();
    predicates.dedup();
    Ok(predicates)
}

pub(crate) fn flatten_checked_boolean_connective<'expression>(
    expression: &'expression CheckedBooleanExpression,
    conjunction: bool,
    output: &mut Vec<&'expression CheckedBooleanExpression>,
) {
    match expression {
        CheckedBooleanExpression::And { left, right } if conjunction => {
            flatten_checked_boolean_connective(left, conjunction, output);
            flatten_checked_boolean_connective(right, conjunction, output);
        }
        CheckedBooleanExpression::Or { left, right } if !conjunction => {
            flatten_checked_boolean_connective(left, conjunction, output);
            flatten_checked_boolean_connective(right, conjunction, output);
        }
        expression => output.push(expression),
    }
}

pub(crate) fn checked_boolean_proposition(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
) -> Result<Proposition, LoweringError> {
    let mut remaining = boolean_input_budget(expression)?;
    checked_boolean_proposition_with_budget(expression, values, &mut remaining, 0)
}

pub(crate) fn boolean_input_budget(
    expression: &CheckedBooleanExpression,
) -> Result<usize, LoweringError> {
    let mut remaining = 4096;
    // Bound logical input traversal before recursive connective discovery.
    // Expansion below consumes this same budget across the whole predicate.
    let mut pending = vec![(expression, 0)];
    while let Some((expression, depth)) = pending.pop() {
        charge_boolean_expansion(&mut remaining, depth)?;
        match expression {
            CheckedBooleanExpression::Not(operand) => pending.push((operand, depth + 1)),
            CheckedBooleanExpression::And { left, right }
            | CheckedBooleanExpression::Or { left, right }
            | CheckedBooleanExpression::Equal { left, right } => {
                pending.extend([(left.as_ref(), depth + 1), (right.as_ref(), depth + 1)]);
            }
            _ => {}
        }
    }
    Ok(remaining)
}

pub(crate) fn charge_boolean_expansion(
    remaining: &mut usize,
    depth: usize,
) -> Result<(), LoweringError> {
    if depth >= 64 {
        return unsupported("crash Boolean expansion exceeds its depth limit");
    }
    *remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
        "crash Boolean expansion exceeds its lowering budget",
    ))?;
    Ok(())
}

fn checked_boolean_proposition_with_budget(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
    remaining: &mut usize,
    depth: usize,
) -> Result<Proposition, LoweringError> {
    charge_boolean_expansion(remaining, depth)?;
    match expression {
        CheckedBooleanExpression::Not(operand) if contains_boolean_connective(operand) => {
            checked_boolean_connective_polarity(operand, values, false, remaining, depth + 1)
        }
        CheckedBooleanExpression::Equal { .. } if contains_boolean_connective(expression) => {
            checked_boolean_connective_polarity(expression, values, true, remaining, depth + 1)
        }
        CheckedBooleanExpression::Constant(_) => {
            unsupported("constant crash predicates must normalize before terminal lowering")
        }
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
            let mut leaves = Vec::new();
            flatten_checked_boolean_connective(left, conjunction, &mut leaves);
            flatten_checked_boolean_connective(right, conjunction, &mut leaves);
            let mut propositions = leaves
                .into_iter()
                .map(|leaf| {
                    checked_boolean_proposition_with_budget(leaf, values, remaining, depth + 1)
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "scalar crash connective is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            propositions.sort_by(|left, right| left.0.cmp(&right.0));
            propositions.dedup_by(|left, right| left.0 == right.0);
            if propositions.len() == 1 {
                return Ok(propositions.pop().expect("one distinct crash predicate").1);
            }
            let propositions = propositions
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect();
            Ok(if conjunction {
                Proposition::Conjunction(propositions)
            } else {
                Proposition::Disjunction(propositions)
            })
        }
        expression => {
            let mut left = checked_boolean_scalar_term(expression, values, &[])?;
            let mut right = ScalarTerm::boolean(true);
            if left > right {
                std::mem::swap(&mut left, &mut right);
            }
            Ok(Proposition::Equal(left, right))
        }
    }
}

pub(crate) fn contains_boolean_connective(expression: &CheckedBooleanExpression) -> bool {
    match expression {
        CheckedBooleanExpression::And { .. } | CheckedBooleanExpression::Or { .. } => true,
        CheckedBooleanExpression::Not(operand) => contains_boolean_connective(operand),
        CheckedBooleanExpression::Equal { left, right } => {
            contains_boolean_connective(left) || contains_boolean_connective(right)
        }
        _ => false,
    }
}

/// Negation of short-circuit predicates belongs to logical propositions, not
/// one eagerly evaluated scalar Boolean term. Keep the existing atomic leaf
/// lowering (including numeric meanings) and only distribute Boolean polarity.
fn checked_boolean_connective_polarity(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
    positive: bool,
    remaining: &mut usize,
    depth: usize,
) -> Result<Proposition, LoweringError> {
    charge_boolean_expansion(remaining, depth)?;
    match expression {
        CheckedBooleanExpression::Constant(_) => {
            unsupported("constant crash predicates must normalize before terminal lowering")
        }
        CheckedBooleanExpression::Not(operand) => {
            checked_boolean_connective_polarity(operand, values, !positive, remaining, depth + 1)
        }
        CheckedBooleanExpression::Equal { left, right } => match (left.as_ref(), right.as_ref()) {
            (CheckedBooleanExpression::Constant(value), operand)
            | (operand, CheckedBooleanExpression::Constant(value)) => {
                checked_boolean_connective_polarity(
                    operand,
                    values,
                    *value == positive,
                    remaining,
                    depth + 1,
                )
            }
            _ if contains_boolean_connective(expression) => {
                crate::proofs::contract_predicates::equality_from_polarities(
                    left,
                    right,
                    positive,
                    remaining,
                    |operand, polarity, budget| {
                        checked_boolean_connective_polarity(
                            operand,
                            values,
                            polarity,
                            budget,
                            depth + 1,
                        )
                    },
                )
            }
            _ => checked_boolean_atom_polarity(expression, values, positive, remaining, depth + 1),
        },
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            let conjunction =
                matches!(expression, CheckedBooleanExpression::And { .. }) == positive;
            let mut flattened = Vec::new();
            for operand in [left.as_ref(), right.as_ref()] {
                let proposition = checked_boolean_connective_polarity(
                    operand,
                    values,
                    positive,
                    remaining,
                    depth + 1,
                )?;
                match proposition {
                    Proposition::Conjunction(children) if conjunction => flattened.extend(children),
                    Proposition::Disjunction(children) if !conjunction => {
                        flattened.extend(children)
                    }
                    proposition => flattened.push(proposition),
                }
            }
            let mut keyed = flattened
                .into_iter()
                .map(|proposition| {
                    terminal_codec::canonical_proposition_order_key(&proposition)
                        .map(|key| (key, proposition))
                        .map_err(|_| {
                            LoweringError::Unsupported(
                                "scalar crash connective is not canonically encodable",
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            keyed.sort_by(|left, right| left.0.cmp(&right.0));
            keyed.dedup_by(|left, right| left.0 == right.0);
            if keyed.len() == 1 {
                return Ok(keyed.pop().expect("one distinct crash predicate").1);
            }
            let children = keyed
                .into_iter()
                .map(|(_, proposition)| proposition)
                .collect();
            Ok(if conjunction {
                Proposition::Conjunction(children)
            } else {
                Proposition::Disjunction(children)
            })
        }
        _ => checked_boolean_atom_polarity(expression, values, positive, remaining, depth + 1),
    }
}

fn checked_boolean_atom_polarity(
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
    positive: bool,
    remaining: &mut usize,
    depth: usize,
) -> Result<Proposition, LoweringError> {
    charge_boolean_expansion(remaining, depth)?;
    if positive {
        return checked_boolean_proposition_with_budget(expression, values, remaining, depth + 1);
    }
    let mut left = ScalarTerm::boolean_not(checked_boolean_scalar_term(expression, values, &[])?)
        .map_err(LoweringError::InvalidCrashPredicate)?;
    let mut right = ScalarTerm::boolean(true);
    if left > right {
        std::mem::swap(&mut left, &mut right);
    }
    Ok(Proposition::Equal(left, right))
}
