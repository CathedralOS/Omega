//! Checked crash predicates and boolean propositions under an expansion
//! budget. Constants retain their Boolean denotation in site guards and
//! connective leaves. Route-bucket normalization separately turns an authored
//! false ceiling into no allowed route; a false predicate never grants a crash.

use crate::proofs::crash_routes::scalar_terms::{checked_boolean_scalar_term, checked_scalar_term};
use crate::proofs::{
    CheckedBooleanExpression, CheckedScalarExpression, LoweringError, Proposition, ScalarTerm,
    ValueDeclaration, unsupported,
};
use checked_trees::CheckedIeeeFloatComparisonKind;

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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
) -> Result<Proposition, LoweringError> {
    let mut remaining = boolean_input_budget(expression)?;
    checked_boolean_proposition_with_budget(identities, expression, values, &mut remaining, 0)
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
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
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

pub(super) fn checked_boolean_proposition_with_budget(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
    remaining: &mut usize,
    depth: usize,
) -> Result<Proposition, LoweringError> {
    charge_boolean_expansion(remaining, depth)?;
    match expression {
        CheckedBooleanExpression::Not(operand) if contains_boolean_connective(operand) => {
            checked_boolean_connective_polarity(
                identities,
                operand,
                values,
                false,
                remaining,
                depth + 1,
            )
        }
        // IEEE `!=` is the true negation of `==` (NaN included), so negation
        // distributes into the atomic kind rather than an outer proposition.
        CheckedBooleanExpression::Not(operand)
            if let CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } =
                operand.as_ref() =>
        {
            scalar_ieee_float_comparison_proposition(
                identities,
                negated_float_comparison_kind(*kind),
                left,
                right,
                values,
            )
        }
        CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } => {
            scalar_ieee_float_comparison_proposition(identities, *kind, left, right, values)
        }
        CheckedBooleanExpression::Equal { .. } if contains_boolean_connective(expression) => {
            checked_boolean_connective_polarity(
                identities,
                expression,
                values,
                true,
                remaining,
                depth + 1,
            )
        }
        CheckedBooleanExpression::Constant(value) => Ok(if *value {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        }),
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            let conjunction = matches!(expression, CheckedBooleanExpression::And { .. });
            let mut leaves = Vec::new();
            flatten_checked_boolean_connective(left, conjunction, &mut leaves);
            flatten_checked_boolean_connective(right, conjunction, &mut leaves);
            let propositions = leaves
                .into_iter()
                .map(|leaf| {
                    checked_boolean_proposition_with_budget(
                        identities,
                        leaf,
                        values,
                        remaining,
                        depth + 1,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            crate::proofs::contract_predicates::canonical_connectives(propositions, conjunction)
        }
        expression => {
            let mut left = checked_boolean_scalar_term(identities, expression, values, &[])?;
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
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
    positive: bool,
    remaining: &mut usize,
    depth: usize,
) -> Result<Proposition, LoweringError> {
    charge_boolean_expansion(remaining, depth)?;
    match expression {
        CheckedBooleanExpression::Constant(value) => Ok(if *value == positive {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        }),
        CheckedBooleanExpression::Not(operand) => checked_boolean_connective_polarity(
            identities,
            operand,
            values,
            !positive,
            remaining,
            depth + 1,
        ),
        CheckedBooleanExpression::Equal { left, right } => match (left.as_ref(), right.as_ref()) {
            (CheckedBooleanExpression::Constant(value), operand)
            | (operand, CheckedBooleanExpression::Constant(value)) => {
                checked_boolean_connective_polarity(
                    identities,
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
                            identities,
                            operand,
                            values,
                            polarity,
                            budget,
                            depth + 1,
                        )
                    },
                )
            }
            _ => checked_boolean_atom_polarity(
                identities,
                expression,
                values,
                positive,
                remaining,
                depth + 1,
            ),
        },
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            let conjunction =
                matches!(expression, CheckedBooleanExpression::And { .. }) == positive;
            let propositions = [left.as_ref(), right.as_ref()]
                .into_iter()
                .map(|operand| {
                    checked_boolean_connective_polarity(
                        identities,
                        operand,
                        values,
                        positive,
                        remaining,
                        depth + 1,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            crate::proofs::contract_predicates::canonical_connectives(propositions, conjunction)
        }
        _ => checked_boolean_atom_polarity(
            identities,
            expression,
            values,
            positive,
            remaining,
            depth + 1,
        ),
    }
}

pub(super) fn checked_boolean_atom_polarity(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &CheckedBooleanExpression,
    values: &[ValueDeclaration],
    positive: bool,
    remaining: &mut usize,
    depth: usize,
) -> Result<Proposition, LoweringError> {
    charge_boolean_expansion(remaining, depth)?;
    if let CheckedBooleanExpression::ScalarIeeeFloatComparison { kind, left, right } = expression {
        return scalar_ieee_float_comparison_proposition(
            identities,
            if positive {
                *kind
            } else {
                negated_float_comparison_kind(*kind)
            },
            left,
            right,
            values,
        );
    }
    if positive {
        return checked_boolean_proposition_with_budget(
            identities,
            expression,
            values,
            remaining,
            depth + 1,
        );
    }
    let mut left = ScalarTerm::boolean_not(checked_boolean_scalar_term(
        identities,
        expression,
        values,
        &[],
    )?)
    .map_err(LoweringError::InvalidCrashPredicate)?;
    let mut right = ScalarTerm::boolean(true);
    if left > right {
        std::mem::swap(&mut left, &mut right);
    }
    Ok(Proposition::Equal(left, right))
}

fn negated_float_comparison_kind(
    kind: CheckedIeeeFloatComparisonKind,
) -> CheckedIeeeFloatComparisonKind {
    kind.negated()
}

fn scalar_ieee_float_comparison_proposition(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    kind: CheckedIeeeFloatComparisonKind,
    left: &CheckedScalarExpression,
    right: &CheckedScalarExpression,
    values: &[ValueDeclaration],
) -> Result<Proposition, LoweringError> {
    crate::proofs::contract_predicates::ieee_float_proposition(
        kind,
        checked_scalar_term(identities, left, values, &[])?,
        checked_scalar_term(identities, right, values, &[])?,
    )
}
