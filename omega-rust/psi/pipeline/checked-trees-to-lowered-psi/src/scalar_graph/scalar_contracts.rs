//! Scalar contracts over entry parameters and the normal-return result.
//! Graph and ordered-operation bodies share the same predicate meaning. Only
//! their execution schedules differ; normal guarantees still use the declared
//! result pseudo-value and require independently reconstructed return proofs.
use super::{
    CheckedBooleanExpression, CheckedScalarExpression, ClosedScalarContractValue,
    ClosedScalarValueContractPlan, IntegerValue, LoweringError, Proposition, ScalarTerm,
    ValueDeclaration, unsupported,
};
#[cfg(test)]
use crate::proofs::contract_predicates::canonical_equality;
use crate::proofs::contract_predicates::{PredicateTerms, connective};

mod namespace;
mod result_range;
mod source;
pub(crate) use result_range::with_result_range;
pub(crate) use source::validate_guarantees;

/// The requires clauses that still discharge as propositions. Floating entry
/// ranges keep an explicit `None` placeholder in the requires tail because the
/// closed scalar predicate language cannot spell IEEE membership; those rows
/// are delivered through the retained roster instead of `clauses`.
/// `validate_graph_parameter_ranges` rejoins every authored range against the
/// roster row-for-row; what remains here is the roster/tail correspondence
/// itself. The roster must be present and cover every placeholder exactly, so
/// an authored clause that lost its predicate can never hide behind a
/// floating range: placeholder count and roster length disagree the moment
/// either side carries an extra row.
pub(crate) fn covered_requires(
    plan: &ClosedScalarValueContractPlan,
) -> Result<Vec<Option<ClosedScalarContractValue>>, LoweringError> {
    let placeholders = plan
        .requires()
        .iter()
        .filter(|clause| clause.is_none())
        .count();
    if plan
        .float_entry_ranges()
        .map_or(placeholders != 0, |ranges| ranges.len() != placeholders)
    {
        return unsupported("scalar contract contains an unsupported clause");
    }
    Ok(plan
        .requires()
        .iter()
        .filter(|clause| clause.is_some())
        .cloned()
        .collect())
}

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
pub(crate) fn proposition(
    predicate: &CheckedBooleanExpression,
    namespace: &[ValueDeclaration],
) -> Result<Proposition, LoweringError> {
    self::namespace::validate(predicate)?;
    crate::proofs::contract_predicates::proposition(predicate, &ScalarContractTerms { namespace })
}

struct ScalarContractTerms<'namespace> {
    namespace: &'namespace [ValueDeclaration],
}

impl PredicateTerms for ScalarContractTerms<'_> {
    fn integer(&self, expression: &CheckedScalarExpression) -> Result<ScalarTerm, LoweringError> {
        crate::proofs::crash_routes::checked_scalar_term(expression, self.namespace)
    }

    fn boolean(&self, expression: &CheckedBooleanExpression) -> Result<ScalarTerm, LoweringError> {
        crate::proofs::crash_routes::checked_boolean_scalar_term(expression, self.namespace)
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
    use super::super::{IntegerType, ScalarType};
    use super::{Proposition, ScalarTerm, strict_result_bound};
    use semantic_vocabulary::IntegerSign;
    use semantic_vocabulary::ValueId;

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
