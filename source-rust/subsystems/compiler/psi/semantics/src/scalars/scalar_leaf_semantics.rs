use crate::operations::OperationSemanticTag;
use crate::{
    GoalFreeScalarLeafSchema, OperationSemanticError, ScalarLeafDenotation, ScalarLeafFactShape,
    ScalarLeafOperandShape, ScalarLeafResultShape, operation_semantic_row,
};
use semantic_vocabulary::{IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};
use std::collections::BTreeMap;
use terminal_psi::{Operation, OperationKind};

/// The scalar type of each value the scalar-leaf semantics reads. A
/// caller that holds value types in a dense table passes the table; it
/// need not build a map keyed by value identity to ask.
pub trait ValueTypes {
    fn value_type(&self, value: ValueId) -> Option<ScalarType>;
}

impl ValueTypes for BTreeMap<ValueId, ScalarType> {
    fn value_type(&self, value: ValueId) -> Option<ScalarType> {
        self.get(&value).copied()
    }
}

impl ValueTypes for arena::IdTable<ValueId, ScalarType> {
    fn value_type(&self, value: ValueId) -> Option<ScalarType> {
        self.get(value).copied()
    }
}

/// Value types read from declarations whose identities need not be
/// dense, sorted once and searched by binary search.
pub struct SortedValueTypes(Vec<(ValueId, ScalarType)>);

impl SortedValueTypes {
    /// `None` when one value is declared twice.
    pub fn new(mut rows: Vec<(ValueId, ScalarType)>) -> Option<Self> {
        rows.sort_unstable_by_key(|(value, _)| *value);
        if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return None;
        }
        Some(Self(rows))
    }
}

impl ValueTypes for SortedValueTypes {
    fn value_type(&self, value: ValueId) -> Option<ScalarType> {
        self.0
            .binary_search_by_key(&value, |(row, _)| *row)
            .ok()
            .map(|position| self.0[position].1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScalarLeafInputs {
    IntegerLiteral(IntegerValue),
    FloatLiteral(semantic_vocabulary::IeeeFloatValue),
    BooleanLiteral(bool),
    Unary(ValueId),
    Binary(ValueId, ValueId),
}

fn scalar_leaf_inputs(operation: &OperationKind) -> Option<ScalarLeafInputs> {
    match operation {
        OperationKind::IntegerConstant { value } => {
            Some(ScalarLeafInputs::IntegerLiteral(*value))
        }
        OperationKind::BooleanConstant { value } => {
            Some(ScalarLeafInputs::BooleanLiteral(*value))
        }
        OperationKind::IeeeFloatConstant { value } => {
            Some(ScalarLeafInputs::FloatLiteral(*value))
        }
        OperationKind::BooleanNot { operand }
        | OperationKind::IntegerBitwiseNot { operand }
        | OperationKind::IntegerWiden { operand } => Some(ScalarLeafInputs::Unary(*operand)),
        OperationKind::BooleanEqual { left, right }
        | OperationKind::IntegerEqual { left, right }
        | terminal_psi::integer_less_or_equal_pair_pattern!(left, right, ..)
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
    value_types: &dyn ValueTypes,
) -> Result<ScalarTerm, OperationSemanticError> {
    value_types
        .value_type(value)
        .map(|scalar_type| ScalarTerm::value(value, scalar_type))
        .ok_or(OperationSemanticError::UnknownValue(value))
}

pub(crate) fn integer_type(term: &ScalarTerm) -> Option<semantic_vocabulary::IntegerType> {
    match term.scalar_type() {
        ScalarType::Integer(integer_type) => Some(integer_type),
        ScalarType::Boolean | ScalarType::IeeeFloat(_) | ScalarType::Callable(_) => None,
    }
}

fn validate_result_shape(
    tag: OperationSemanticTag,
    shape: ScalarLeafResultShape,
    actual: ScalarType,
) -> Result<(), OperationSemanticError> {
    let valid = match shape {
        ScalarLeafResultShape::DeclaredInteger => matches!(actual, ScalarType::Integer(_)),
        ScalarLeafResultShape::DeclaredFloat => matches!(actual, ScalarType::IeeeFloat(_)),
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
    value_types: &dyn ValueTypes,
) -> Result<(), OperationSemanticError> {
    let type_of = |value: ValueId| {
        value_types
            .value_type(value)
            .ok_or(OperationSemanticError::UnknownValue(value))
    };
    let valid = match (schema.operands, inputs) {
        (ScalarLeafOperandShape::IntegerLiteral, ScalarLeafInputs::IntegerLiteral(_))
        | (ScalarLeafOperandShape::BooleanLiteral, ScalarLeafInputs::BooleanLiteral(_)) => true,
        (ScalarLeafOperandShape::FloatLiteral, ScalarLeafInputs::FloatLiteral(value)) => {
            result_type == ScalarType::IeeeFloat(value.format())
        }
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
            if result_type == ScalarType::Boolean {
                matches!((left_type, right_type),
                    (ScalarType::Integer(left), ScalarType::Integer(right))
                        if left == right || (!left.is_address() && !right.is_address()))
            } else {
                matches!(left_type, ScalarType::Integer(_))
                    && left_type == right_type
                    && result_type == left_type
            }
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
    value_types: &dyn ValueTypes,
) -> Result<ScalarTerm, OperationSemanticError> {
    let term = |value| value_term(value, value_types);
    let invalid = || OperationSemanticError::DenotationShapeMismatch(tag);
    let built = match (schema.denotation, inputs) {
        (ScalarLeafDenotation::IntegerConstant, ScalarLeafInputs::IntegerLiteral(value)) => {
            let Some(integer_type) = result_type.as_integer() else {
                return Err(invalid());
            };
            ScalarTerm::integer(integer_type, value)
        }
        (ScalarLeafDenotation::BooleanConstant, ScalarLeafInputs::BooleanLiteral(value)) => {
            return Ok(ScalarTerm::boolean(value));
        }
        (ScalarLeafDenotation::FloatConstant, ScalarLeafInputs::FloatLiteral(value)) => {
            return Ok(ScalarTerm::IeeeFloatLiteral(value));
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
            let Some(integer_type) = result_type.as_integer() else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_not(integer_type, term(operand)?)
        }
        (ScalarLeafDenotation::IntegerWiden, ScalarLeafInputs::Unary(operand)) => {
            let operand = term(operand)?;
            let source_type = integer_type(&operand).ok_or_else(invalid)?;
            let Some(target_type) = result_type.as_integer() else {
                return Err(invalid());
            };
            ScalarTerm::integer_widen(source_type, target_type, operand)
        }
        (ScalarLeafDenotation::IntegerBitwiseAnd, ScalarLeafInputs::Binary(left, right)) => {
            let Some(integer_type) = result_type.as_integer() else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_and(integer_type, term(left)?, term(right)?)
        }
        (ScalarLeafDenotation::IntegerBitwiseOr, ScalarLeafInputs::Binary(left, right)) => {
            let Some(integer_type) = result_type.as_integer() else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_or(integer_type, term(left)?, term(right)?)
        }
        (ScalarLeafDenotation::IntegerBitwiseXor, ScalarLeafInputs::Binary(left, right)) => {
            let Some(integer_type) = result_type.as_integer() else {
                return Err(invalid());
            };
            ScalarTerm::integer_bitwise_xor(integer_type, term(left)?, term(right)?)
        }
        (
            ScalarLeafDenotation::WrappingIntegerShiftLeft,
            ScalarLeafInputs::Binary(value, count),
        ) => {
            let Some(value_type) = result_type.as_integer() else {
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
            let Some(value_type) = result_type.as_integer() else {
                return Err(invalid());
            };
            let count = term(count)?;
            let count_type = integer_type(&count).ok_or_else(invalid)?;
            ScalarTerm::wrapping_integer_shift_right(
                value_type,
                count_type,
                term(value)?,
                count,
            )
        }
        (denotation, ScalarLeafInputs::Binary(left, right)) => {
            let Some(integer_type) = result_type.as_integer() else {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoalFreeScalarLeafSemantics {
    result_equation: Proposition,
    fact_shape: ScalarLeafFactShape,
}

impl GoalFreeScalarLeafSemantics {
    pub fn result_equation(&self) -> &Proposition {
        &self.result_equation
    }
}

pub fn goal_free_scalar_leaf_semantics(
    operation: &Operation,
    value_types: &dyn ValueTypes,
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
