//! Mathematical contract terms retain finite value identity at their leaves.

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
