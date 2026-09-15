//! Interpreting one goal-free scalar leaf: constant folding, operand and
//! result validation, and the denotation term it produces.

use crate::semantic_rows::OperationSemanticTag;
use crate::{
    GoalFreeScalarLeafSchema, OperationSemanticError, ScalarLeafDenotation, ScalarLeafFactShape,
    ScalarLeafOperandShape, ScalarLeafResultShape, operation_semantic_row,
};
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use terminal_psi::{Operation, OperationKind};

/// The exact literal one goal-free scalar leaf denotes once every operand
/// resolves to a literal. This is the value language of constant folding:
/// integer and Boolean leaves only — no goal-free leaf produces a float, so a
/// float literal can never be required here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarLeafLiteral {
    Integer(IntegerValue),
    Boolean(bool),
}

/// Evaluate one goal-free scalar leaf whose operands are already literals.
///
/// `literals` binds each value identity known to denote an exact literal: the
/// results of literal operations and of leaves already folded under this same
/// rule. `value_types` carries the declared scalar type of every value the
/// operation may read. Literal denotations are seed values, not candidates,
/// and every operand must resolve through `literals`; `None` leaves the
/// operation unchanged in either case.
pub fn constant_goal_free_scalar_leaf(
    operation: &Operation,
    literals: &BTreeMap<ValueId, ScalarLeafLiteral>,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Option<ScalarLeafLiteral> {
    let row = operation_semantic_row(&operation.kind).ok()?;
    let schema = row.goal_free_scalar_leaf()?;
    let inputs = scalar_leaf_inputs(&operation.kind)?;
    let result = operation.result.scalar_ref()?;
    validate_result_shape(row.tag, schema.result, result.scalar_type).ok()?;
    validate_operand_shape(row.tag, schema, inputs, result.scalar_type, value_types).ok()?;
    let integer_literal = |value: ValueId| match literals.get(&value) {
        Some(ScalarLeafLiteral::Integer(literal)) => Some(*literal),
        _ => None,
    };
    let boolean_literal = |value: ValueId| match literals.get(&value) {
        Some(ScalarLeafLiteral::Boolean(literal)) => Some(*literal),
        _ => None,
    };
    let integer_type_of = |value: ValueId| match value_types.get(&value) {
        Some(ScalarType::Integer(integer_type)) => Some(*integer_type),
        _ => None,
    };
    let result_integer_type = || match result.scalar_type {
        ScalarType::Integer(integer_type) => Some(integer_type),
        _ => None,
    };
    let binary_integer = |left: ValueId, right: ValueId| {
        Some((
            integer_type_of(left)?,
            integer_literal(left)?,
            integer_literal(right)?,
        ))
    };
    Some(match (schema.denotation, inputs) {
        (ScalarLeafDenotation::BooleanNot, ScalarLeafInputs::Unary(operand)) => {
            ScalarLeafLiteral::Boolean(!boolean_literal(operand)?)
        }
        (ScalarLeafDenotation::BooleanEqual, ScalarLeafInputs::Binary(left, right)) => {
            ScalarLeafLiteral::Boolean(boolean_literal(left)? == boolean_literal(right)?)
        }
        (ScalarLeafDenotation::IntegerEqual, ScalarLeafInputs::Binary(left, right)) => {
            let (operand_type, left, right) = binary_integer(left, right)?;
            ScalarLeafLiteral::Boolean(operand_type.compare(left, right)? == Ordering::Equal)
        }
        (ScalarLeafDenotation::IntegerLessThan, ScalarLeafInputs::Binary(left, right)) => {
            let (operand_type, left, right) = binary_integer(left, right)?;
            ScalarLeafLiteral::Boolean(operand_type.compare(left, right)? == Ordering::Less)
        }
        (ScalarLeafDenotation::IntegerLessOrEqual, ScalarLeafInputs::Binary(left, right)) => {
            let (operand_type, left, right) = binary_integer(left, right)?;
            ScalarLeafLiteral::Boolean(operand_type.compare(left, right)? != Ordering::Greater)
        }
        (ScalarLeafDenotation::IntegerBitwiseNot, ScalarLeafInputs::Unary(operand)) => {
            ScalarLeafLiteral::Integer(
                result_integer_type()?.bitwise_not(integer_literal(operand)?)?,
            )
        }
        (ScalarLeafDenotation::IntegerWiden, ScalarLeafInputs::Unary(operand)) => {
            ScalarLeafLiteral::Integer(
                integer_type_of(operand)?
                    .widen_value_to(result_integer_type()?, integer_literal(operand)?)?,
            )
        }
        (ScalarLeafDenotation::IntegerBitwiseAnd, ScalarLeafInputs::Binary(left, right)) => {
            ScalarLeafLiteral::Integer(
                result_integer_type()?
                    .bitwise_and(integer_literal(left)?, integer_literal(right)?)?,
            )
        }
        (ScalarLeafDenotation::IntegerBitwiseOr, ScalarLeafInputs::Binary(left, right)) => {
            ScalarLeafLiteral::Integer(
                result_integer_type()?
                    .bitwise_or(integer_literal(left)?, integer_literal(right)?)?,
            )
        }
        (ScalarLeafDenotation::IntegerBitwiseXor, ScalarLeafInputs::Binary(left, right)) => {
            ScalarLeafLiteral::Integer(
                result_integer_type()?
                    .bitwise_xor(integer_literal(left)?, integer_literal(right)?)?,
            )
        }
        (
            ScalarLeafDenotation::WrappingIntegerShiftLeft,
            ScalarLeafInputs::Binary(value, count),
        ) => ScalarLeafLiteral::Integer(result_integer_type()?.wrapping_shift_left(
            integer_literal(value)?,
            integer_type_of(count)?,
            integer_literal(count)?,
        )?),
        (
            ScalarLeafDenotation::WrappingIntegerShiftRight,
            ScalarLeafInputs::Binary(value, count),
        ) => ScalarLeafLiteral::Integer(result_integer_type()?.wrapping_shift_right(
            integer_literal(value)?,
            integer_type_of(count)?,
            integer_literal(count)?,
        )?),
        (ScalarLeafDenotation::WrappingIntegerAdd, ScalarLeafInputs::Binary(left, right)) => {
            ScalarLeafLiteral::Integer(
                result_integer_type()?
                    .wrapping_add(integer_literal(left)?, integer_literal(right)?)?,
            )
        }
        (ScalarLeafDenotation::SaturatingIntegerAdd, ScalarLeafInputs::Binary(left, right)) => {
            ScalarLeafLiteral::Integer(
                result_integer_type()?
                    .saturating_add(integer_literal(left)?, integer_literal(right)?)?,
            )
        }
        (ScalarLeafDenotation::WrappingIntegerSubtract, ScalarLeafInputs::Binary(left, right)) => {
            ScalarLeafLiteral::Integer(
                result_integer_type()?
                    .wrapping_sub(integer_literal(left)?, integer_literal(right)?)?,
            )
        }
        (
            ScalarLeafDenotation::SaturatingIntegerSubtract,
            ScalarLeafInputs::Binary(left, right),
        ) => ScalarLeafLiteral::Integer(
            result_integer_type()?
                .saturating_sub(integer_literal(left)?, integer_literal(right)?)?,
        ),
        (ScalarLeafDenotation::WrappingIntegerMultiply, ScalarLeafInputs::Binary(left, right)) => {
            ScalarLeafLiteral::Integer(
                result_integer_type()?
                    .wrapping_mul(integer_literal(left)?, integer_literal(right)?)?,
            )
        }
        (
            ScalarLeafDenotation::SaturatingIntegerMultiply,
            ScalarLeafInputs::Binary(left, right),
        ) => ScalarLeafLiteral::Integer(
            result_integer_type()?
                .saturating_mul(integer_literal(left)?, integer_literal(right)?)?,
        ),
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarLeafInputs {
    IntegerLiteral(IntegerValue),
    BooleanLiteral(bool),
    Unary(ValueId),
    Binary(ValueId, ValueId),
}

fn scalar_leaf_inputs(operation: &OperationKind) -> Option<ScalarLeafInputs> {
    match operation {
        OperationKind::IntegerConstant { value } => Some(ScalarLeafInputs::IntegerLiteral(*value)),
        OperationKind::BooleanConstant { value } => Some(ScalarLeafInputs::BooleanLiteral(*value)),
        OperationKind::BooleanNot { operand }
        | OperationKind::IntegerBitwiseNot { operand }
        | OperationKind::IntegerWiden { operand } => Some(ScalarLeafInputs::Unary(*operand)),
        OperationKind::BooleanEqual { left, right }
        | OperationKind::IntegerEqual { left, right }
        | OperationKind::IntegerLessThan { left, right }
        | OperationKind::IntegerLessOrEqual { left, right }
        | OperationKind::IntegerBitwiseAnd { left, right }
        | OperationKind::IntegerBitwiseOr { left, right }
        | OperationKind::IntegerBitwiseXor { left, right }
        | OperationKind::WrappingIntegerShiftLeft {
            value: left,
            count: right,
        }
        | OperationKind::WrappingIntegerShiftRight {
            value: left,
            count: right,
        }
        | OperationKind::WrappingIntegerAdd { left, right }
        | OperationKind::SaturatingIntegerAdd { left, right }
        | OperationKind::WrappingIntegerSubtract { left, right }
        | OperationKind::SaturatingIntegerSubtract { left, right }
        | OperationKind::WrappingIntegerMultiply { left, right }
        | OperationKind::SaturatingIntegerMultiply { left, right } => {
            Some(ScalarLeafInputs::Binary(*left, *right))
        }
        _ => None,
    }
}

pub(crate) fn value_term(
    value: ValueId,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<ScalarTerm, OperationSemanticError> {
    value_types
        .get(&value)
        .copied()
        .map(|scalar_type| ScalarTerm::value(value, scalar_type))
        .ok_or(OperationSemanticError::UnknownValue(value))
}

pub(crate) fn integer_type(term: &ScalarTerm) -> Option<semantic_vocabulary::IntegerType> {
    match term.scalar_type() {
        ScalarType::Integer(integer_type) => Some(integer_type),
        ScalarType::Boolean | ScalarType::IeeeFloat(_) => None,
    }
}

fn validate_result_shape(
    tag: OperationSemanticTag,
    shape: ScalarLeafResultShape,
    actual: ScalarType,
) -> Result<(), OperationSemanticError> {
    let valid = match shape {
        ScalarLeafResultShape::DeclaredInteger => matches!(actual, ScalarType::Integer(_)),
        ScalarLeafResultShape::Boolean => actual == ScalarType::Boolean,
    };
    valid
        .then_some(())
        .ok_or(OperationSemanticError::ResultShapeMismatch(tag))
}

fn validate_operand_shape(
    tag: OperationSemanticTag,
    schema: GoalFreeScalarLeafSchema,
    inputs: ScalarLeafInputs,
    result_type: ScalarType,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<(), OperationSemanticError> {
    let type_of = |value: ValueId| {
        value_types
            .get(&value)
            .copied()
            .ok_or(OperationSemanticError::UnknownValue(value))
    };
    let valid = match (schema.operands, inputs) {
        (ScalarLeafOperandShape::IntegerLiteral, ScalarLeafInputs::IntegerLiteral(_))
        | (ScalarLeafOperandShape::BooleanLiteral, ScalarLeafInputs::BooleanLiteral(_)) => true,
        (ScalarLeafOperandShape::UnaryBoolean, ScalarLeafInputs::Unary(operand)) => {
            type_of(operand)? == ScalarType::Boolean
        }
        (ScalarLeafOperandShape::BinaryBoolean, ScalarLeafInputs::Binary(left, right)) => {
            type_of(left)? == ScalarType::Boolean && type_of(right)? == ScalarType::Boolean
        }
        (ScalarLeafOperandShape::UnaryInteger, ScalarLeafInputs::Unary(operand)) => {
            matches!(result_type, ScalarType::Integer(_)) && type_of(operand)? == result_type
        }
        (ScalarLeafOperandShape::BinaryInteger, ScalarLeafInputs::Binary(left, right)) => {
            let left_type = type_of(left)?;
            let right_type = type_of(right)?;
            matches!(left_type, ScalarType::Integer(_))
                && left_type == right_type
                && (result_type == ScalarType::Boolean || result_type == left_type)
        }
        (ScalarLeafOperandShape::WideningInteger, ScalarLeafInputs::Unary(operand)) => {
            let (ScalarType::Integer(source), ScalarType::Integer(target)) =
                (type_of(operand)?, result_type)
            else {
                return Err(OperationSemanticError::OperandShapeMismatch(tag));
            };
            source.can_widen_to(target)
        }
        (ScalarLeafOperandShape::IntegerShift, ScalarLeafInputs::Binary(value, count)) => {
            type_of(value)? == result_type
                && matches!(result_type, ScalarType::Integer(_))
                && matches!(type_of(count)?, ScalarType::Integer(_))
        }
        _ => false,
    };
    valid
        .then_some(())
        .ok_or(OperationSemanticError::OperandShapeMismatch(tag))
}

fn denotation_term(
    tag: OperationSemanticTag,
    schema: GoalFreeScalarLeafSchema,
    inputs: ScalarLeafInputs,
    result_type: ScalarType,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<ScalarTerm, OperationSemanticError> {
    let term = |value| value_term(value, value_types);
    let invalid = || OperationSemanticError::DenotationShapeMismatch(tag);
    let built = match (schema.denotation, inputs) {
        (ScalarLeafDenotation::IntegerConstant, ScalarLeafInputs::IntegerLiteral(value)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer(integer_type, value)
        }
        (ScalarLeafDenotation::BooleanConstant, ScalarLeafInputs::BooleanLiteral(value)) => {
            return Ok(ScalarTerm::boolean(value));
        }
        (ScalarLeafDenotation::BooleanNot, ScalarLeafInputs::Unary(operand)) => {
            ScalarTerm::boolean_not(term(operand)?)
        }
        (ScalarLeafDenotation::BooleanEqual, ScalarLeafInputs::Binary(left, right)) => {
            ScalarTerm::boolean_equal(term(left)?, term(right)?)
        }
        (ScalarLeafDenotation::IntegerEqual, ScalarLeafInputs::Binary(left, right)) => {
            let left = term(left)?;
            let integer_type = integer_type(&left).ok_or_else(invalid)?;
            ScalarTerm::integer_equal(integer_type, left, term(right)?)
        }
        (ScalarLeafDenotation::IntegerLessThan, ScalarLeafInputs::Binary(left, right)) => {
            let left = term(left)?;
            let integer_type = integer_type(&left).ok_or_else(invalid)?;
            ScalarTerm::integer_less_than(integer_type, left, term(right)?)
        }
        (ScalarLeafDenotation::IntegerLessOrEqual, ScalarLeafInputs::Binary(left, right)) => {
            let left = term(left)?;
            let integer_type = integer_type(&left).ok_or_else(invalid)?;
            ScalarTerm::integer_less_or_equal(integer_type, left, term(right)?)
        }
        (ScalarLeafDenotation::IntegerBitwiseNot, ScalarLeafInputs::Unary(operand)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_not(integer_type, term(operand)?)
        }
        (ScalarLeafDenotation::IntegerWiden, ScalarLeafInputs::Unary(operand)) => {
            let operand = term(operand)?;
            let source_type = integer_type(&operand).ok_or_else(invalid)?;
            let ScalarType::Integer(target_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_widen(source_type, target_type, operand)
        }
        (ScalarLeafDenotation::IntegerBitwiseAnd, ScalarLeafInputs::Binary(left, right)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_and(integer_type, term(left)?, term(right)?)
        }
        (ScalarLeafDenotation::IntegerBitwiseOr, ScalarLeafInputs::Binary(left, right)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_or(integer_type, term(left)?, term(right)?)
        }
        (ScalarLeafDenotation::IntegerBitwiseXor, ScalarLeafInputs::Binary(left, right)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_xor(integer_type, term(left)?, term(right)?)
        }
        (
            ScalarLeafDenotation::WrappingIntegerShiftLeft,
            ScalarLeafInputs::Binary(value, count),
        ) => {
            let ScalarType::Integer(value_type) = result_type else {
                return Err(invalid());
            };
            let count = term(count)?;
            let count_type = integer_type(&count).ok_or_else(invalid)?;
            ScalarTerm::wrapping_integer_shift_left(value_type, count_type, term(value)?, count)
        }
        (
            ScalarLeafDenotation::WrappingIntegerShiftRight,
            ScalarLeafInputs::Binary(value, count),
        ) => {
            let ScalarType::Integer(value_type) = result_type else {
                return Err(invalid());
            };
            let count = term(count)?;
            let count_type = integer_type(&count).ok_or_else(invalid)?;
            ScalarTerm::wrapping_integer_shift_right(value_type, count_type, term(value)?, count)
        }
        (denotation, ScalarLeafInputs::Binary(left, right)) => {
            let ScalarType::Integer(integer_type) = result_type else {
                return Err(invalid());
            };
            let left = term(left)?;
            let right = term(right)?;
            match denotation {
                ScalarLeafDenotation::WrappingIntegerAdd => {
                    ScalarTerm::wrapping_integer_add(integer_type, left, right)
                }
                ScalarLeafDenotation::SaturatingIntegerAdd => {
                    ScalarTerm::saturating_integer_add(integer_type, left, right)
                }
                ScalarLeafDenotation::WrappingIntegerSubtract => {
                    ScalarTerm::wrapping_integer_subtract(integer_type, left, right)
                }
                ScalarLeafDenotation::SaturatingIntegerSubtract => {
                    ScalarTerm::saturating_integer_subtract(integer_type, left, right)
                }
                ScalarLeafDenotation::WrappingIntegerMultiply => {
                    ScalarTerm::wrapping_integer_multiply(integer_type, left, right)
                }
                ScalarLeafDenotation::SaturatingIntegerMultiply => {
                    ScalarTerm::saturating_integer_multiply(integer_type, left, right)
                }
                _ => return Err(invalid()),
            }
        }
        _ => return Err(invalid()),
    };
    built.map_err(OperationSemanticError::InvalidProposition)
}

/// Validated operation-local result meaning and its declared fact policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalFreeScalarLeafSemantics {
    result_equation: Proposition,
    fact_shape: ScalarLeafFactShape,
}

impl GoalFreeScalarLeafSemantics {
    pub fn result_equation(&self) -> &Proposition {
        &self.result_equation
    }

    pub fn fact_shape(&self) -> ScalarLeafFactShape {
        self.fact_shape
    }
}

/// Interpret one goal-free scalar leaf through its exact declarative row.
/// `Ok(None)` means the operation belongs to a different semantic algebra.
pub fn goal_free_scalar_leaf_semantics(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<Option<GoalFreeScalarLeafSemantics>, OperationSemanticError> {
    let row = operation_semantic_row(&operation.kind)?;
    let Some(schema) = row.goal_free_scalar_leaf else {
        return Ok(None);
    };
    let inputs = scalar_leaf_inputs(&operation.kind)
        .ok_or(OperationSemanticError::OperandShapeMismatch(row.tag))?;
    let result = operation
        .result
        .scalar_ref()
        .ok_or(OperationSemanticError::MissingScalarResult(row.tag))?;
    validate_result_shape(row.tag, schema.result, result.scalar_type)?;
    validate_operand_shape(row.tag, schema, inputs, result.scalar_type, value_types)?;
    let denotation = denotation_term(row.tag, schema, inputs, result.scalar_type, value_types)?;
    Ok(Some(GoalFreeScalarLeafSemantics {
        result_equation: Proposition::Equal(
            ScalarTerm::value(result.id, result.scalar_type),
            denotation,
        ),
        fact_shape: schema.fact(),
    }))
}
