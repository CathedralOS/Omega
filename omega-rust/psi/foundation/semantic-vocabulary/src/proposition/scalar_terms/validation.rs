//! Term validation: post-order operand checking shared by every term
//! shape, extracted from `scalar_terms` so constructor and query browsing is
//! not buried under the rule bodies.

use super::{
    ScalarTerm, ScalarType, Step, validate_exact_integer_operands,
    validate_integer_comparison_operands, validate_integer_operands,
    validate_integer_shift_operands,
};
use crate::proposition::{IntegerType, PropositionError};

impl ScalarTerm {
    pub fn validate(&self) -> Result<(), PropositionError> {
        // An explicit worklist keeps deep terms off the call stack, matching
        // the canonical-order guards in terminal-codec: `Visit` pushes a
        // `Finalize` step behind the term's operands so each node's own
        // typing rule runs only after its operands validated — the same
        // post-order position the recursive validator gave it. Children push
        // in reverse so the left operand still validates first.
        let mut pending = vec![Step::Visit(self)];
        while let Some(step) = pending.pop() {
            match step {
                Step::Visit(term) => {
                    pending.push(Step::Finalize(term));
                    for operand in term.operands().into_iter().rev() {
                        pending.push(Step::Visit(operand));
                    }
                }
                Step::Finalize(term) => term.validate_after_operands()?,
            }
        }
        Ok(())
    }

    /// The node's own typing rule, run once its operands have validated:
    /// literal range admission, operand type agreement, and cast or shift
    /// legality. Never walks children — `validate` drives the traversal.
    fn validate_after_operands(&self) -> Result<(), PropositionError> {
        match self {
            Self::StructuralRead { path, .. } => {
                if path.is_empty() {
                    return Err(PropositionError::EmptyStructuralReadPath);
                }
                for operand in self.operands() {
                    if operand
                        .scalar_type()
                        .as_integer()
                        .is_none_or(|integer| integer.is_address())
                    {
                        return Err(PropositionError::StructuralReadIndexType(
                            operand.scalar_type(),
                        ));
                    }
                }
                Ok(())
            }
            Self::Value { .. }
            | Self::BooleanField { .. }
            | Self::IntegerField { .. }
            | Self::ViewExtent { .. }
            | Self::ByteSequencePredicate { .. }
            | Self::IeeeFloatField { .. }
            | Self::Boolean(_)
            | Self::IeeeFloatLiteral(_) => Ok(()),
            Self::BooleanNot { operand } => {
                if operand.scalar_type() != ScalarType::Boolean {
                    return Err(PropositionError::BooleanNotTypeMismatch(
                        operand.scalar_type(),
                    ));
                }
                Ok(())
            }
            Self::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => {
                let expected = ScalarType::Integer(*source_type);
                if operand.scalar_type() != expected || !source_type.can_exact_cast_to(*target_type)
                {
                    return Err(PropositionError::IntegerExactCastTypeMismatch {
                        source: expected,
                        target: ScalarType::Integer(*target_type),
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::BooleanEqual { left, right } => {
                if left.scalar_type() != ScalarType::Boolean
                    || right.scalar_type() != ScalarType::Boolean
                {
                    return Err(PropositionError::BooleanEqualTypeMismatch {
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerEqual {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerLessThan {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => validate_integer_comparison_operands(*scalar_type, left, right),
            Self::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => {
                let expected = ScalarType::Integer(*scalar_type);
                if operand.scalar_type() != expected {
                    return Err(PropositionError::IntegerBitwiseNotTypeMismatch {
                        expected,
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => {
                let expected = ScalarType::Integer(*source_type);
                if operand.scalar_type() != expected || !source_type.can_widen_to(*target_type) {
                    return Err(PropositionError::IntegerWidenTypeMismatch {
                        source: expected,
                        target: ScalarType::Integer(*target_type),
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            } => validate_integer_operands(*scalar_type, left, right),
            Self::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => validate_integer_shift_operands(*value_type, *count_type, value, count),
            Self::Integer { scalar_type, value } => {
                if scalar_type.admits(*value) {
                    Ok(())
                } else {
                    Err(PropositionError::IntegerLiteralOutsideType {
                        scalar_type: *scalar_type,
                        value: *value,
                    })
                }
            }
            Self::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } => validate_exact_integer_operands(
                *scalar_type,
                left,
                right,
                IntegerType::admits_exact_subtract_operands,
            ),
            Self::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => validate_exact_integer_operands(
                *scalar_type,
                left,
                right,
                IntegerType::admits_exact_add_operands,
            ),
            Self::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            }
            | Self::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | Self::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | Self::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | Self::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | Self::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            }
            | Self::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            }
            | Self::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | Self::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | Self::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | Self::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | Self::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            }
            | Self::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => validate_integer_operands(*scalar_type, left, right),
        }
    }
}
