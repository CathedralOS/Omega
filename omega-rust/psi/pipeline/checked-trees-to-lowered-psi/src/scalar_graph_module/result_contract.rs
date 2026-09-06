//! Scalar contracts over entry parameters and the normal-return result.

use super::*;
#[cfg(test)]
use crate::contract_predicates::canonical_equality;
use crate::contract_predicates::{PredicateTerms, connective};

mod namespace;

pub(crate) fn clauses(
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
    crate::contract_predicates::proposition(predicate, &ScalarContractTerms { namespace })
}

struct ScalarContractTerms<'namespace> {
    namespace: &'namespace [ValueDeclaration],
}

impl PredicateTerms for ScalarContractTerms<'_> {
    fn integer(&self, expression: &CheckedScalarExpression) -> Result<ScalarTerm, LoweringError> {
        crate::crash_routes::checked_scalar_term(expression, self.namespace)
    }

    fn boolean(&self, expression: &CheckedBooleanExpression) -> Result<ScalarTerm, LoweringError> {
        crate::crash_routes::checked_boolean_scalar_term(expression, self.namespace)
    }

    fn strict_bound(&self, left: ScalarTerm, right: ScalarTerm) -> Proposition {
        strict_result_bound(left, right)
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
