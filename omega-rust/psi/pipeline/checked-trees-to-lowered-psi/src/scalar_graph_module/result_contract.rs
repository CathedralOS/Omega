//! Scalar contracts over entry parameters and the normal-return result.

use super::*;

mod namespace;

pub(super) fn clauses(
    clauses: &[Option<ClosedScalarContractValue>],
    namespace: &[ValueDeclaration],
) -> Result<Option<Proposition>, LoweringError> {
    let mut combined = None;
    for clause in clauses {
        let proposition = match clause {
            Some(ClosedScalarContractValue::Predicate(predicate)) => {
                proposition(predicate, namespace)?
            }
            // The checked selection gate established builtin reflexivity.
            Some(ClosedScalarContractValue::Boolean(_) | ClosedScalarContractValue::Integer(_)) => {
                Proposition::Truth
            }
            None => return unsupported("scalar contract clause has no checked predicate"),
        };
        combined = Some(if let Some(previous) = combined {
            connective(previous, proposition, true)?
        } else {
            proposition
        });
    }
    Ok(combined)
}

/// Preserve integer contract relations as propositions, not executable Boolean
/// comparisons equated with true. Call composition can then cite the exact
/// relation in ordinary fixed-integer proof rules.
pub(super) fn proposition(
    predicate: &CheckedBooleanExpression,
    namespace: &[ValueDeclaration],
) -> Result<Proposition, LoweringError> {
    self::namespace::validate(predicate)?;
    // Equality of compound predicates needs both child polarities. Bound the
    // total expansion before allocating an exponential proposition tree.
    proposition_with_polarity(predicate, namespace, true, &mut 4096)
}

fn proposition_with_polarity(
    predicate: &CheckedBooleanExpression,
    namespace: &[ValueDeclaration],
    positive: bool,
    remaining: &mut usize,
) -> Result<Proposition, LoweringError> {
    *remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
        "scalar contract Boolean expansion exceeds its lowering budget",
    ))?;
    match predicate {
        CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let left = crate::crash_routes::checked_scalar_term(left, namespace)?;
            let right = crate::crash_routes::checked_scalar_term(right, namespace)?;
            Ok(match (kind, positive) {
                (CheckedIntegerComparisonKind::Equal, true) => canonical_equality(left, right)?,
                (CheckedIntegerComparisonKind::Equal, false) => connective(
                    strict_result_bound(left.clone(), right.clone()),
                    strict_result_bound(right, left),
                    false,
                )?,
                (CheckedIntegerComparisonKind::LessThan, true) => strict_result_bound(left, right),
                (CheckedIntegerComparisonKind::LessOrEqual, false) => {
                    strict_result_bound(right, left)
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
            let left = proposition_with_polarity(left, namespace, positive, remaining)?;
            let right = proposition_with_polarity(right, namespace, positive, remaining)?;
            connective(
                left,
                right,
                matches!(predicate, CheckedBooleanExpression::And { .. }) == positive,
            )
        }
        CheckedBooleanExpression::Not(operand) => {
            proposition_with_polarity(operand, namespace, !positive, remaining)
        }
        CheckedBooleanExpression::Constant(value) if *value == positive => Ok(Proposition::Truth),
        CheckedBooleanExpression::Constant(_) | CheckedBooleanExpression::Parameter { .. } => {
            canonical_equality(
                crate::crash_routes::checked_boolean_scalar_term(predicate, namespace)?,
                ScalarTerm::boolean(positive),
            )
        }
        CheckedBooleanExpression::Equal { left, right } => {
            if let CheckedBooleanExpression::Constant(value) = left.as_ref() {
                return proposition_with_polarity(right, namespace, *value == positive, remaining);
            }
            if let CheckedBooleanExpression::Constant(value) = right.as_ref() {
                return proposition_with_polarity(left, namespace, *value == positive, remaining);
            }
            if positive
                && matches!(left.as_ref(), CheckedBooleanExpression::Parameter { .. })
                && matches!(right.as_ref(), CheckedBooleanExpression::Parameter { .. })
            {
                return canonical_equality(
                    crate::crash_routes::checked_boolean_scalar_term(left, namespace)?,
                    crate::crash_routes::checked_boolean_scalar_term(right, namespace)?,
                );
            }
            // Equality selects equal polarities; inequality selects opposite
            // polarities. Keep logical facts in the proposition language so
            // calls can prove them from their evaluated argument equations.
            connective(
                connective(
                    proposition_with_polarity(left, namespace, true, remaining)?,
                    proposition_with_polarity(right, namespace, positive, remaining)?,
                    true,
                )?,
                connective(
                    proposition_with_polarity(left, namespace, false, remaining)?,
                    proposition_with_polarity(right, namespace, !positive, remaining)?,
                    true,
                )?,
                false,
            )
        }
        _ => unsupported("result contract has an unsupported scalar predicate"),
    }
}

fn canonical_equality(left: ScalarTerm, right: ScalarTerm) -> Result<Proposition, LoweringError> {
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

fn connective(
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

/// Integer strict bounds with a literal endpoint have an exact inclusive
/// spelling. Emit that spelling before any obligation is reconstructed; no
/// certificate is allowed to reinterpret a strict fact as an inclusive one.
fn strict_result_bound(left: ScalarTerm, right: ScalarTerm) -> Proposition {
    let neighbor = |term: &ScalarTerm, increment: bool| {
        let ScalarTerm::Integer { scalar_type, value } = term else {
            return None;
        };
        let one = match value {
            IntegerValue::Signed(_) => IntegerValue::Signed(1),
            IntegerValue::Unsigned(_) => IntegerValue::Unsigned(1),
        };
        let value = if increment {
            scalar_type.exact_add(*value, one)
        } else {
            scalar_type.exact_sub(*value, one)
        }?;
        Some(ScalarTerm::Integer {
            scalar_type: *scalar_type,
            value,
        })
    };
    if let Some(endpoint) = neighbor(&right, false) {
        Proposition::LessOrEqual(left, endpoint)
    } else if let Some(endpoint) = neighbor(&left, true) {
        Proposition::LessOrEqual(endpoint, right)
    } else {
        // At an endpoint with no representable neighbor retain the original
        // proposition. In particular, never wrap x < MIN into x <= MAX.
        Proposition::LessThan(left, right)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parameter_namespace;

    #[test]
    fn strict_integer_endpoints_never_wrap() {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            for bits in [8, 16, 32, 64, 128] {
                let scalar_type = IntegerType::new(sign, bits).unwrap();
                let value = ScalarTerm::Value {
                    id: ValueId::new(1).unwrap(),
                    scalar_type: ScalarType::Integer(scalar_type),
                };
                let minimum = ScalarTerm::Integer {
                    scalar_type,
                    value: scalar_type.minimum_value(),
                };
                let maximum = ScalarTerm::Integer {
                    scalar_type,
                    value: scalar_type.maximum_value(),
                };
                assert_eq!(
                    strict_result_bound(value.clone(), minimum.clone()),
                    Proposition::LessThan(value.clone(), minimum.clone())
                );
                assert_eq!(
                    strict_result_bound(maximum.clone(), value.clone()),
                    Proposition::LessThan(maximum.clone(), value.clone())
                );
                assert!(matches!(
                    strict_result_bound(minimum, value.clone()),
                    Proposition::LessOrEqual(_, _)
                ));
                assert!(matches!(
                    strict_result_bound(value, maximum),
                    Proposition::LessOrEqual(_, _)
                ));
            }
        }
    }
}
