//! Mathematical contract terms retain finite value identity at their leaves.

#[cfg(test)]
use super::{CheckedBooleanExpression, CheckedIntegerComparisonKind, Proposition};
use super::{IntegerMathTerm, LoweringError, PredicateTerms, ScalarTerm, ScalarType, unsupported};
use checked_trees::CheckedScalarExpression;

/// Mathematical operations have no finite carrier and cannot inherit the
/// overflow policy of an embedded operand. Only the embedded leaf selects a
/// runtime carrier; its exact value identity is resolved by the caller's same
/// entry/result namespace used for ordinary scalar contracts.
pub(super) fn lower(
    expression: &checked_trees::CheckedIntegerMathExpression,
    terms: &impl PredicateTerms,
    remaining: &mut usize,
) -> Result<IntegerMathTerm, LoweringError> {
    fn lower(
        expression: &checked_trees::CheckedIntegerMathExpression,
        handle: arena::Handle<checked_trees::CheckedIntegerMathTerm>,
        terms: &impl PredicateTerms,
        remaining: &mut usize,
        depth: usize,
    ) -> Result<IntegerMathTerm, LoweringError> {
        use checked_trees::CheckedIntegerMathTerm;
        *remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
            "mathematical contract exceeds its lowering budget",
        ))?;
        if depth >= 64 || !expression.terms.is_valid(handle) {
            return unsupported("mathematical contract has an invalid term graph");
        }
        match expression.terms.get(handle) {
            CheckedIntegerMathTerm::Embed {
                primitive_type,
                operand,
            } => {
                let expected =
                    crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?;
                let scalar = terms.scalar(operand)?;
                if scalar.scalar_type() != expected {
                    return unsupported("mathematical embedding changed its source carrier");
                }
                match scalar {
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(source_type),
                    } => Ok(IntegerMathTerm::MathValue {
                        source_type,
                        value: id,
                    }),
                    ScalarTerm::Integer { value, .. } => Ok(IntegerMathTerm::literal(value)),
                    // A computed finite operand needs its own exact value
                    // identity. Reinterpreting its operation tree as math
                    // would erase Wrapping/Saturating policy and is unsound.
                    _ => unsupported(
                        "mathematical embedding requires an exact scalar value identity",
                    ),
                }
            }
            CheckedIntegerMathTerm::Literal(literal) => {
                let term = IntegerMathTerm::IntegerLiteral(*literal);
                term.validate()
                    .map_err(LoweringError::InvalidCrashPredicate)?;
                Ok(term)
            }
            CheckedIntegerMathTerm::Add { left, right }
            | CheckedIntegerMathTerm::Subtract { left, right }
            | CheckedIntegerMathTerm::Multiply { left, right } => {
                let left = Box::new(lower(expression, *left, terms, remaining, depth + 1)?);
                let right = Box::new(lower(expression, *right, terms, remaining, depth + 1)?);
                Ok(match expression.terms.get(handle) {
                    CheckedIntegerMathTerm::Add { .. } => IntegerMathTerm::Add(left, right),
                    CheckedIntegerMathTerm::Subtract { .. } => {
                        IntegerMathTerm::Subtract(left, right)
                    }
                    _ => IntegerMathTerm::Multiply(left, right),
                })
            }
        }
    }
    lower(expression, expression.root, terms, remaining, 0)
}

#[cfg(test)]
mod mathematical_contract_tests {
    use super::*;
    use checked_trees::{CheckedIntegerMathExpression, CheckedIntegerMathTerm};
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ValueId};
    use terminal_psi::ValueDeclaration;
    use typed_trees::types::PrimitiveType;

    fn embedded(position: usize) -> CheckedIntegerMathExpression {
        let mut terms = arena::Arena::new();
        let root = terms.insert(CheckedIntegerMathTerm::Embed {
            primitive_type: PrimitiveType::U64,
            operand: CheckedScalarExpression::Parameter {
                position,
                primitive_type: PrimitiveType::U64,
            },
        });
        CheckedIntegerMathExpression { terms, root }
    }

    fn lower(predicate: &CheckedBooleanExpression) -> Result<Proposition, LoweringError> {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let namespace = [101, 203].map(|identity| ValueDeclaration {
            id: ValueId::new(identity).unwrap(),
            scalar_type: ScalarType::Integer(integer),
            qualifications: Default::default(),
        });
        crate::scalar_graph::scalar_contracts::proposition(
            predicate,
            &namespace,
            &[],
            &crate::scalar_graph::scalar_contracts::ContractViewNamespace::EMPTY,
        )
    }

    #[test]
    fn mathematical_contract_retains_signed_subtraction_and_exact_subjects() {
        let mut difference = embedded(0);
        let right = difference.terms.insert(CheckedIntegerMathTerm::Embed {
            primitive_type: PrimitiveType::U64,
            operand: CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U64,
            },
        });
        difference.root = difference.terms.insert(CheckedIntegerMathTerm::Subtract {
            left: difference.root,
            right,
        });
        let mut zero_terms = arena::Arena::new();
        let zero_root = zero_terms.insert(CheckedIntegerMathTerm::Literal(
            semantic_vocabulary::IntegerMathLiteral::from_integer_value(IntegerValue::Signed(0)),
        ));
        let predicate = CheckedBooleanExpression::IntegerMathComparison {
            kind: CheckedIntegerComparisonKind::LessThan,
            left: difference,
            right: CheckedIntegerMathExpression {
                terms: zero_terms,
                root: zero_root,
            },
        };
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let left = IntegerMathTerm::Subtract(
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer,
                value: ValueId::new(101).unwrap(),
            }),
            Box::new(IntegerMathTerm::MathValue {
                source_type: integer,
                value: ValueId::new(203).unwrap(),
            }),
        );
        assert_eq!(
            lower(&predicate).unwrap(),
            Proposition::IntegerMathLessThan(
                left.clone(),
                IntegerMathTerm::literal(IntegerValue::Signed(0))
            )
        );
        assert_eq!(
            lower(&CheckedBooleanExpression::Not(Box::new(predicate))).unwrap(),
            Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::literal(IntegerValue::Signed(0)),
                left
            )
        );
    }

    #[test]
    fn embedding_a_finite_sum_never_erases_its_wrapping_policy() {
        let mut expression = embedded(0);
        let CheckedIntegerMathTerm::Embed { operand, .. } =
            expression.terms.get_mut(expression.root)
        else {
            unreachable!()
        };
        *operand = CheckedScalarExpression::IntegerBinary {
            kind: checked_trees::CheckedIntegerBinaryKind::WrappingAdd,
            primitive_type: PrimitiveType::U64,
            left: Box::new(CheckedScalarExpression::Parameter {
                position: 0,
                primitive_type: PrimitiveType::U64,
            }),
            right: Box::new(CheckedScalarExpression::Parameter {
                position: 1,
                primitive_type: PrimitiveType::U64,
            }),
        };
        let predicate = CheckedBooleanExpression::IntegerMathComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left: expression,
            right: embedded(0),
        };
        // A policy-selected sum has no fresh result identity in this
        // contract namespace. Refuse it instead of publishing unbounded +.
        assert!(matches!(
            lower(&predicate),
            Err(LoweringError::Unsupported(
                "mathematical embedding requires an exact scalar value identity"
            ))
        ));
    }

    #[test]
    fn mathematical_contract_rejects_forged_carrier_and_cyclic_graph() {
        let mut expression = embedded(0);
        let CheckedIntegerMathTerm::Embed { primitive_type, .. } =
            expression.terms.get_mut(expression.root)
        else {
            unreachable!()
        };
        *primitive_type = PrimitiveType::I64;
        let compare = |left| CheckedBooleanExpression::IntegerMathComparison {
            kind: CheckedIntegerComparisonKind::Equal,
            left,
            right: embedded(0),
        };
        assert!(lower(&compare(expression)).is_err());
        let mut expression = embedded(0);
        *expression.terms.get_mut(expression.root) = CheckedIntegerMathTerm::Add {
            left: expression.root,
            right: expression.root,
        };
        assert!(lower(&compare(expression)).is_err());
        let mut expression = embedded(0);
        expression.root = arena::Handle::invalid();
        assert!(lower(&compare(expression)).is_err());
    }
}

/// Apply the owning contract namespace to every embedded finite operand.
/// Graph validation is shared; each caller still decides which subjects its
/// own contract can observe. Mathematical operators never create body reads.
pub(crate) fn validate_namespace(
    expression: &checked_trees::CheckedIntegerMathExpression,
    scalar: &impl Fn(&CheckedScalarExpression) -> Result<(), LoweringError>,
) -> Result<(), LoweringError> {
    use checked_trees::CheckedIntegerMathTerm;
    let mut pending = vec![(expression.root, 0)];
    let mut remaining = 4096usize;
    while let Some((handle, depth)) = pending.pop() {
        remaining = remaining.checked_sub(1).ok_or(LoweringError::Unsupported(
            "mathematical contract namespace exceeds its traversal budget",
        ))?;
        if depth >= 64 || !expression.terms.is_valid(handle) {
            return Err(LoweringError::Unsupported(
                "mathematical contract has an invalid term graph",
            ));
        }
        match expression.terms.get(handle) {
            CheckedIntegerMathTerm::Embed { operand, .. } => scalar(operand)?,
            CheckedIntegerMathTerm::Literal(_) => {}
            CheckedIntegerMathTerm::Add { left, right }
            | CheckedIntegerMathTerm::Subtract { left, right }
            | CheckedIntegerMathTerm::Multiply { left, right } => {
                pending.push((*right, depth + 1));
                pending.push((*left, depth + 1));
            }
        }
    }
    Ok(())
}
