//! Shared logical contract conversion; callers retain ownership of their leaf namespaces.

use super::*;

pub(super) trait PredicateTerms {
    fn integer(&self, expression: &CheckedScalarExpression) -> Result<ScalarTerm, LoweringError>;
    fn boolean(&self, expression: &CheckedBooleanExpression) -> Result<ScalarTerm, LoweringError>;
    fn strict_bound(&self, left: ScalarTerm, right: ScalarTerm) -> Proposition {
        Proposition::LessThan(left, right)
    }
}

pub(super) fn proposition(
    predicate: &CheckedBooleanExpression,
    terms: &impl PredicateTerms,
) -> Result<Proposition, LoweringError> {
    // Compound Boolean equality expands both polarities under one shared budget.
    proposition_with_polarity(predicate, terms, true, &mut 4096)
}

fn is_boolean_atom(predicate: &CheckedBooleanExpression) -> bool {
    matches!(
        predicate,
        CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
    )
}

fn proposition_with_polarity(
    predicate: &CheckedBooleanExpression,
    terms: &impl PredicateTerms,
    positive: bool,
    remaining: &mut usize,
) -> Result<Proposition, LoweringError> {
    *remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
        "scalar contract Boolean expansion exceeds its lowering budget",
    ))?;
    match predicate {
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let left = terms.integer(left)?;
            let right = terms.integer(right)?;
            Ok(match (kind, positive) {
                (CheckedIntegerComparisonKind::Equal, true) => canonical_equality(left, right)?,
                (CheckedIntegerComparisonKind::Equal, false) => connective(
                    terms.strict_bound(left.clone(), right.clone()),
                    terms.strict_bound(right, left),
                    false,
                )?,
                (CheckedIntegerComparisonKind::LessThan, true) => terms.strict_bound(left, right),
                (CheckedIntegerComparisonKind::LessOrEqual, false) => {
                    terms.strict_bound(right, left)
                }
                (CheckedIntegerComparisonKind::LessOrEqual, true) => {
                    Proposition::LessOrEqual(left, right)
                }
                (CheckedIntegerComparisonKind::LessThan, false) => {
                    Proposition::LessOrEqual(right, left)
                }
            })
        }
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            let left = proposition_with_polarity(left, terms, positive, remaining)?;
            let right = proposition_with_polarity(right, terms, positive, remaining)?;
            connective(
                left,
                right,
                matches!(predicate, CheckedBooleanExpression::And { .. }) == positive,
            )
        }
        CheckedBooleanExpression::Not(operand) => {
            proposition_with_polarity(operand, terms, !positive, remaining)
        }
        CheckedBooleanExpression::Constant(value) if *value == positive => Ok(Proposition::Truth),
        CheckedBooleanExpression::Constant(_)
        | CheckedBooleanExpression::Parameter { .. }
        | CheckedBooleanExpression::StructuralParameterField { .. } => {
            canonical_equality(terms.boolean(predicate)?, ScalarTerm::boolean(positive))
        }
        CheckedBooleanExpression::Equal { left, right } => {
            if let CheckedBooleanExpression::Constant(value) = left.as_ref() {
                return proposition_with_polarity(right, terms, *value == positive, remaining);
            }
            if let CheckedBooleanExpression::Constant(value) = right.as_ref() {
                return proposition_with_polarity(left, terms, *value == positive, remaining);
            }
            if positive && is_boolean_atom(left) && is_boolean_atom(right) {
                return canonical_equality(terms.boolean(left)?, terms.boolean(right)?);
            }
            // Equality selects equal polarities; inequality selects opposite
            // polarities. Keep logical facts in the proposition language so
            // calls can prove them from their evaluated argument equations.
            equality_from_polarities(
                left,
                right,
                positive,
                remaining,
                |operand, polarity, budget| {
                    proposition_with_polarity(operand, terms, polarity, budget)
                },
            )
        }
        _ => unsupported("result contract has an unsupported scalar predicate"),
    }
}

/// Share the Boolean equivalence law without sharing atomic namespaces or
/// denotations. Every expanded operand uses the caller's same work budget.
pub(super) fn equality_from_polarities(
    left: &CheckedBooleanExpression,
    right: &CheckedBooleanExpression,
    positive: bool,
    remaining: &mut usize,
    mut lower: impl FnMut(
        &CheckedBooleanExpression,
        bool,
        &mut usize,
    ) -> Result<Proposition, LoweringError>,
) -> Result<Proposition, LoweringError> {
    connective(
        connective(
            lower(left, true, remaining)?,
            lower(right, positive, remaining)?,
            true,
        )?,
        connective(
            lower(left, false, remaining)?,
            lower(right, !positive, remaining)?,
            true,
        )?,
        false,
    )
}

pub(super) fn canonical_equality(
    left: ScalarTerm,
    right: ScalarTerm,
) -> Result<Proposition, LoweringError> {
    let left_key = terminal_codec::canonical_scalar_term_order_key(&left)
        .map_err(LoweringError::DebugSemanticCodec)?;
    let right_key = terminal_codec::canonical_scalar_term_order_key(&right)
        .map_err(LoweringError::DebugSemanticCodec)?;
    Ok(if left_key <= right_key {
        Proposition::Equal(left, right)
    } else {
        Proposition::Equal(right, left)
    })
}

pub(super) fn connective(
    left: Proposition,
    right: Proposition,
    conjunction: bool,
) -> Result<Proposition, LoweringError> {
    match (&left, &right, conjunction) {
        (Proposition::Truth, _, true) => return Ok(right),
        (_, Proposition::Truth, true) => return Ok(left),
        (Proposition::Truth, _, false) | (_, Proposition::Truth, false) => {
            return Ok(Proposition::Truth);
        }
        _ => {}
    }
    let mut parts = Vec::new();
    for proposition in [left, right] {
        match proposition {
            Proposition::Conjunction(nested) if conjunction => parts.extend(nested),
            Proposition::Disjunction(nested) if !conjunction => parts.extend(nested),
            proposition => parts.push(proposition),
        }
    }
    let mut keyed = parts
        .into_iter()
        .map(|proposition| {
            terminal_codec::canonical_proposition_order_key(&proposition)
                .map(|key| (key, proposition))
                .map_err(LoweringError::DebugSemanticCodec)
        })
        .collect::<Result<Vec<_>, _>>()?;
    keyed.sort_by(|left, right| left.0.cmp(&right.0));
    keyed.dedup_by(|left, right| left.0 == right.0);
    let mut parts = keyed
        .into_iter()
        .map(|(_, proposition)| proposition)
        .collect::<Vec<_>>();
    if parts.len() == 1 {
        Ok(parts.pop().expect("one distinct predicate"))
    } else if conjunction {
        Ok(Proposition::Conjunction(parts))
    } else {
        Ok(Proposition::Disjunction(parts))
    }
}
