use crate::LoweringError;
use abstract_operations::AbstractParameter;
use calling_conventions::{ValueLocation, ValuePlacement, ValueShape};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
};
use std::collections::BTreeMap;
use target_operations::{
    ScalarParameterLocation, TargetBooleanExpression, TargetIntegerExpression,
    TargetScalarExpression,
};

pub(in crate::lowering) fn scalar_shape(
    value: ValueId,
    scalar_type: ScalarType,
    require_native_parameter: bool,
) -> Result<ValueShape, LoweringError> {
    let bytes = match scalar_type {
        ScalarType::Boolean => 1,
        ScalarType::Integer(integer_type) => {
            let bits = integer_type.bits();
            if require_native_parameter && !matches!(bits, 8 | 16 | 32 | 64) {
                return Err(LoweringError::ParameterWidthNotNativelySupported { value, bits });
            }
            bits.div_ceil(8)
        }
        ScalarType::IeeeFloat(format) => {
            return Ok(ValueShape::float(format.byte_size()));
        }
    };
    Ok(ValueShape::integer(
        u32::from(bytes),
        bytes.next_power_of_two().min(8),
    ))
}

pub(in crate::lowering) fn scalar_parameter_location(
    parameter: &AbstractParameter,
    placement: &ValuePlacement,
) -> Result<ScalarParameterLocation, LoweringError> {
    let expected_bytes = scalar_shape(parameter.value, parameter.scalar_type, true)?.byte_size;
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == expected_bytes => Ok(ScalarParameterLocation::Register(*register)),
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == expected_bytes => Ok(ScalarParameterLocation::IncomingStack {
            byte_offset: *stack_byte_offset,
        }),
        _ => Err(LoweringError::UnsupportedScalarParameterPlacement(
            parameter.value,
        )),
    }
}

pub(super) fn insert_value(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    id: ValueId,
    value: KnownScalar,
) -> Result<(), LoweringError> {
    if values.insert(id, value).is_some() {
        return Err(LoweringError::DuplicateValue(id));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::lowering) enum KnownScalar {
    Boolean(bool),
    BooleanRuntime(TargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        value: KnownInteger,
    },
}

impl KnownScalar {
    pub(in crate::lowering) fn into_expression(
        self,
        source_value: ValueId,
    ) -> Result<TargetScalarExpression, LoweringError> {
        Ok(match self {
            Self::Boolean(value) => {
                TargetScalarExpression::Boolean(TargetBooleanExpression::Immediate {
                    source_value,
                    value,
                })
            }
            Self::BooleanRuntime(expression) => TargetScalarExpression::Boolean(expression),
            Self::Integer { scalar_type, value } => TargetScalarExpression::Integer {
                scalar_type,
                expression: value.into_expression(source_value),
            },
        })
    }
}

pub(in crate::lowering) fn negate_boolean(
    value: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
    match value {
        KnownScalar::Boolean(value) => Ok(KnownScalar::Boolean(!value)),
        KnownScalar::BooleanRuntime(TargetBooleanExpression::Not { operand, .. }) => {
            Ok(KnownScalar::BooleanRuntime(*operand))
        }
        KnownScalar::BooleanRuntime(expression) => {
            Ok(KnownScalar::BooleanRuntime(TargetBooleanExpression::Not {
                psi_operation,
                operand: Box::new(expression),
            }))
        }
        KnownScalar::Integer { .. } => Err(LoweringError::ValueTypeMismatch(result)),
    }
}

pub(in crate::lowering) fn equal_boolean(
    left: KnownScalar,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
    match (left, right) {
        (KnownScalar::Boolean(left), KnownScalar::Boolean(right)) => {
            Ok(KnownScalar::Boolean(left == right))
        }
        (value, KnownScalar::Boolean(true)) | (KnownScalar::Boolean(true), value) => Ok(value),
        (value, KnownScalar::Boolean(false)) | (KnownScalar::Boolean(false), value) => {
            negate_boolean(value, psi_operation, result)
        }
        (KnownScalar::BooleanRuntime(left), KnownScalar::BooleanRuntime(right)) => Ok(
            KnownScalar::BooleanRuntime(TargetBooleanExpression::Equal {
                psi_operation,
                left: Box::new(left),
                right: Box::new(right),
            }),
        ),
        _ => Err(LoweringError::ValueTypeMismatch(result)),
    }
}

/// Split an integer pair, refusing non-integers and mixed address types.
fn integer_pair(
    left: KnownScalar,
    right: KnownScalar,
    result: ValueId,
) -> Result<(IntegerType, KnownInteger, IntegerType, KnownInteger), LoweringError> {
    let (
        KnownScalar::Integer {
            scalar_type: left_type,
            value: left,
        },
        KnownScalar::Integer {
            scalar_type: right_type,
            value: right,
        },
    ) = (left, right)
    else {
        return Err(LoweringError::ValueTypeMismatch(result));
    };
    if left_type != right_type && (left_type.is_address() || right_type.is_address()) {
        return Err(LoweringError::ValueTypeMismatch(result));
    }
    Ok((left_type, left, right_type, right))
}

pub(in crate::lowering) fn equal_integer(
    left_id: ValueId,
    left: KnownScalar,
    right_id: ValueId,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
    let (left_type, left, right_type, right) = integer_pair(left, right, result)?;
    match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
            Ok(KnownScalar::Boolean(
                left_type
                    .compare_with(left, right_type, right)
                    .ok_or(LoweringError::ValueTypeMismatch(result))?
                    .is_eq(),
            ))
        }
        (left, right) => Ok(KnownScalar::BooleanRuntime(
            TargetBooleanExpression::IntegerEqual {
                psi_operation,
                scalar_type: left_type,
                right_scalar_type: right_type,
                left: Box::new(left.into_expression(left_id)),
                right: Box::new(right.into_expression(right_id)),
            },
        )),
    }
}

pub(in crate::lowering) fn order_integer(
    left_id: ValueId,
    left: KnownScalar,
    right_id: ValueId,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
    inclusive: bool,
) -> Result<KnownScalar, LoweringError> {
    let (left_type, left, right_type, right) = integer_pair(left, right, result)?;
    match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
            let ordering = left_type
                .compare_with(left, right_type, right)
                .ok_or(LoweringError::ValueTypeMismatch(result))?;
            Ok(KnownScalar::Boolean(if inclusive {
                !ordering.is_gt()
            } else {
                ordering.is_lt()
            }))
        }
        (left, right) => {
            let left = Box::new(left.into_expression(left_id));
            let right = Box::new(right.into_expression(right_id));
            Ok(KnownScalar::BooleanRuntime(if inclusive {
                TargetBooleanExpression::IntegerLessOrEqual {
                    psi_operation,
                    scalar_type: left_type,
                    right_scalar_type: right_type,
                    left,
                    right,
                }
            } else {
                TargetBooleanExpression::IntegerLessThan {
                    psi_operation,
                    scalar_type: left_type,
                    right_scalar_type: right_type,
                    left,
                    right,
                }
            }))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::lowering) enum KnownInteger {
    Immediate(IntegerValue),
    Runtime(TargetIntegerExpression),
}

impl KnownInteger {
    pub(in crate::lowering) fn into_expression(
        self,
        source_value: ValueId,
    ) -> TargetIntegerExpression {
        match self {
            Self::Immediate(value) => TargetIntegerExpression::Immediate {
                source_value,
                value,
            },
            Self::Runtime(expression) => expression,
        }
    }
}

/// One unsigned-u64 endpoint read shared by the byte- and element-view
/// lowering match arms: the bound must be a known unsigned 64-bit integer or
/// the arm refuses with `invalid`.
pub(in crate::lowering) fn unsigned_u64_endpoint<'a, E>(
    values: &'a BTreeMap<ValueId, KnownScalar>,
    invalid: impl Fn() -> E + 'a,
) -> impl Fn(ValueId) -> Result<Box<TargetIntegerExpression>, E> + 'a {
    move |value| {
        let Some(KnownScalar::Integer {
            scalar_type,
            value: known,
        }) = values.get(&value)
        else {
            return Err(invalid());
        };
        if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
            return Err(invalid());
        }
        Ok(Box::new(known.clone().into_expression(value)))
    }
}
