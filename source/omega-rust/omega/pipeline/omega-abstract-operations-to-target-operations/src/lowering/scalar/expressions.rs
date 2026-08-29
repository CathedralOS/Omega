use super::super::shared::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_call(
    psi_operation: psi_core::OperationId,
    result: ValueId,
    scalar_type: ScalarType,
    callee: MachineId,
    arguments: &[ValueId],
    values: &BTreeMap<ValueId, KnownScalar>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
) -> Result<KnownScalar, LoweringError> {
    let callee_function = functions
        .get(&callee)
        .copied()
        .ok_or(LoweringError::UnknownCallTarget(callee))?;
    let callee_result = scalar_function_result(callee_function)?;
    let callee_signature = CallSignature {
        parameters: callee_function
            .parameters
            .iter()
            .map(|parameter| scalar_shape(parameter.value, parameter.scalar_type, true))
            .collect::<Result<Vec<_>, _>>()?,
        result: Some(scalar_shape(
            callee_result.value,
            callee_result.scalar_type,
            false,
        )?),
    };
    let callee_call_plan =
        evaluate_call_plan(CallingPolicy::native_for_target(target), &callee_signature)
            .map_err(LoweringError::AbiPlan)?;
    if arguments.len() != callee_function.parameters.len()
        || arguments.len() != callee_call_plan.parameters.len()
    {
        return Err(LoweringError::CallArgumentCountMismatch {
            callee,
            expected: callee_function.parameters.len(),
            actual: arguments.len(),
        });
    }
    let arguments = arguments
        .iter()
        .zip(&callee_function.parameters)
        .zip(&callee_call_plan.parameters)
        .map(|((argument, parameter), placement)| {
            let expression = values
                .get(argument)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*argument))?
                .into_expression(*argument)?;
            if expression.scalar_type() != parameter.scalar_type {
                return Err(LoweringError::CallArgumentTypeMismatch {
                    callee,
                    argument: *argument,
                });
            }
            Ok(TargetCallArgument {
                scalar_type: parameter.scalar_type,
                location: scalar_parameter_location(parameter, placement)?,
                expression,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(match scalar_type {
        ScalarType::Boolean => KnownScalar::BooleanRuntime(TargetBooleanExpression::Call {
            psi_operation,
            source_value: result,
            callee,
            arguments,
        }),
        ScalarType::Integer(scalar_type) => KnownScalar::Integer {
            scalar_type,
            value: KnownInteger::Runtime(TargetIntegerExpression::Call {
                psi_operation,
                source_value: result,
                callee,
                arguments,
            }),
        },
    })
}

pub(super) fn scalar_function_result(
    function: &AbstractFunction,
) -> Result<AbstractResult, LoweringError> {
    function
        .result
        .scalar()
        .ok_or(LoweringError::FunctionResultKindMismatch(function.machine))
}

pub(super) fn scalar_shape(
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
    };
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}

pub(super) fn scalar_parameter_location(
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
pub(super) enum KnownScalar {
    Boolean(bool),
    BooleanRuntime(TargetBooleanExpression),
    Integer {
        scalar_type: IntegerType,
        value: KnownInteger,
    },
}

impl KnownScalar {
    pub(super) const fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::BooleanRuntime(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(*scalar_type),
        }
    }

    pub(super) fn rebind_direct_parameter(self, source_value: ValueId) -> Self {
        match self {
            Self::Integer { scalar_type, value } => Self::Integer {
                scalar_type,
                value: value.rebind_direct_parameter(source_value),
            },
            Self::BooleanRuntime(TargetBooleanExpression::Parameter {
                parameter_index,
                location,
                ..
            }) => Self::BooleanRuntime(TargetBooleanExpression::Parameter {
                source_value,
                parameter_index,
                location,
            }),
            value @ (Self::Boolean(_) | Self::BooleanRuntime(_)) => value,
        }
    }

    pub(super) fn into_expression(
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

pub(super) fn negate_boolean(
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

pub(super) fn equal_boolean(
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

pub(super) fn equal_integer(
    left_id: ValueId,
    left: KnownScalar,
    right_id: ValueId,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
) -> Result<KnownScalar, LoweringError> {
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
    if left_type != right_type {
        return Err(LoweringError::ValueTypeMismatch(result));
    }
    match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
            Ok(KnownScalar::Boolean(left == right))
        }
        (left, right) => Ok(KnownScalar::BooleanRuntime(
            TargetBooleanExpression::IntegerEqual {
                psi_operation,
                scalar_type: left_type,
                left: Box::new(left.into_expression(left_id)),
                right: Box::new(right.into_expression(right_id)),
            },
        )),
    }
}

pub(super) fn order_integer(
    left_id: ValueId,
    left: KnownScalar,
    right_id: ValueId,
    right: KnownScalar,
    psi_operation: OperationId,
    result: ValueId,
    inclusive: bool,
) -> Result<KnownScalar, LoweringError> {
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
    if left_type != right_type {
        return Err(LoweringError::ValueTypeMismatch(result));
    }
    match (left, right) {
        (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
            let ordering = left_type
                .compare(left, right)
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
                    left,
                    right,
                }
            } else {
                TargetBooleanExpression::IntegerLessThan {
                    psi_operation,
                    scalar_type: left_type,
                    left,
                    right,
                }
            }))
        }
    }
}

pub(super) fn direct_boolean_condition(
    expression: TargetBooleanExpression,
    value: ValueId,
) -> Result<(usize, ScalarParameterLocation, bool), LoweringError> {
    match expression {
        TargetBooleanExpression::Parameter {
            parameter_index,
            location,
            ..
        } => Ok((parameter_index, location, false)),
        TargetBooleanExpression::Not { operand, .. } => match *operand {
            TargetBooleanExpression::Parameter {
                parameter_index,
                location,
                ..
            } => Ok((parameter_index, location, true)),
            _ => Err(LoweringError::UnsupportedRuntimeBooleanCondition(value)),
        },
        _ => Err(LoweringError::UnsupportedRuntimeBooleanCondition(value)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum KnownInteger {
    Immediate(IntegerValue),
    Runtime(TargetIntegerExpression),
}

impl KnownInteger {
    pub(super) fn into_expression(self, source_value: ValueId) -> TargetIntegerExpression {
        match self {
            Self::Immediate(value) => TargetIntegerExpression::Immediate {
                source_value,
                value,
            },
            Self::Runtime(expression) => expression,
        }
    }

    pub(super) fn rebind_direct_parameter(self, source_value: ValueId) -> Self {
        match self {
            Self::Runtime(TargetIntegerExpression::Parameter {
                parameter_index,
                location,
                ..
            }) => Self::Runtime(TargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            }),
            value => value,
        }
    }
}

pub(super) fn conditional_provenance(
    function: &AbstractFunction,
    operations: Vec<psi_core::OperationId>,
    edges: Vec<psi_core::EdgeId>,
) -> TerminalPsiProvenance {
    let mut operations = operations.into_iter().collect::<BTreeSet<_>>();
    let mut edges = edges.into_iter().collect::<BTreeSet<_>>();
    let mut provenance = TerminalPsiProvenance::default();
    for operation in &function.operations {
        let psi_operation = match operation {
            AbstractOperation::EstablishPayloadlessCase { psi_operation, .. }
            | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
            | AbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
            | AbstractOperation::CallUnit { psi_operation, .. }
            | AbstractOperation::CallStructuralScalar { psi_operation, .. }
            | AbstractOperation::CallStructural { psi_operation, .. }
            | AbstractOperation::BoundaryCall { psi_operation, .. }
            | AbstractOperation::PortWrite { psi_operation, .. }
            | AbstractOperation::Call { psi_operation, .. }
            | AbstractOperation::IntegerConstant { psi_operation, .. }
            | AbstractOperation::BooleanConstant { psi_operation, .. }
            | AbstractOperation::BooleanStructuralField { psi_operation, .. }
            | AbstractOperation::BooleanNot { psi_operation, .. }
            | AbstractOperation::BooleanEqual { psi_operation, .. }
            | AbstractOperation::IntegerEqual { psi_operation, .. }
            | AbstractOperation::IntegerLessThan { psi_operation, .. }
            | AbstractOperation::IntegerLessOrEqual { psi_operation, .. }
            | AbstractOperation::IntegerBitwiseNot { psi_operation, .. }
            | AbstractOperation::IntegerWiden { psi_operation, .. }
            | AbstractOperation::IntegerExactCast { psi_operation, .. }
            | AbstractOperation::IntegerBitwiseAnd { psi_operation, .. }
            | AbstractOperation::IntegerBitwiseOr { psi_operation, .. }
            | AbstractOperation::IntegerBitwiseXor { psi_operation, .. }
            | AbstractOperation::WrappingIntegerShiftLeft { psi_operation, .. }
            | AbstractOperation::WrappingIntegerShiftRight { psi_operation, .. }
            | AbstractOperation::ExactIntegerShiftLeft { psi_operation, .. }
            | AbstractOperation::ExactIntegerShiftRight { psi_operation, .. }
            | AbstractOperation::WrappingIntegerAdd { psi_operation, .. }
            | AbstractOperation::ExactIntegerAdd { psi_operation, .. }
            | AbstractOperation::SaturatingIntegerAdd { psi_operation, .. }
            | AbstractOperation::WrappingIntegerSubtract { psi_operation, .. }
            | AbstractOperation::ExactIntegerSubtract { psi_operation, .. }
            | AbstractOperation::SaturatingIntegerSubtract { psi_operation, .. }
            | AbstractOperation::WrappingIntegerMultiply { psi_operation, .. }
            | AbstractOperation::ExactIntegerMultiply { psi_operation, .. }
            | AbstractOperation::SaturatingIntegerMultiply { psi_operation, .. } => {
                Some(*psi_operation)
            }
            AbstractOperation::ExactIntegerDivide { psi_operation, .. } => Some(*psi_operation),
            AbstractOperation::ExactIntegerRemainder { psi_operation, .. } => Some(*psi_operation),
            AbstractOperation::WrappingIntegerDivide { psi_operation, .. } => Some(*psi_operation),
            AbstractOperation::WrappingIntegerRemainder { psi_operation, .. } => {
                Some(*psi_operation)
            }
            AbstractOperation::SaturatingIntegerDivide { psi_operation, .. } => {
                Some(*psi_operation)
            }
            AbstractOperation::SaturatingIntegerRemainder { psi_operation, .. } => {
                Some(*psi_operation)
            }
            AbstractOperation::Jump { .. }
            | AbstractOperation::Conditional { .. }
            | AbstractOperation::Return { .. }
            | AbstractOperation::ReturnUnit { .. }
            | AbstractOperation::ReturnStructural { .. }
            | AbstractOperation::Crash { .. } => None,
        };
        if let Some(psi_operation) = psi_operation
            && operations.remove(&psi_operation)
        {
            provenance.operations.push(psi_operation);
        }
        match operation {
            AbstractOperation::Jump { psi_edge, .. }
            | AbstractOperation::Return { psi_edge, .. }
            | AbstractOperation::ReturnUnit { psi_edge, .. }
            | AbstractOperation::ReturnStructural { psi_edge, .. }
            | AbstractOperation::Crash { psi_edge, .. } => {
                if edges.remove(psi_edge) {
                    provenance.edges.push(*psi_edge);
                }
            }
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for psi_edge in [when_true.psi_edge, when_false.psi_edge] {
                    if edges.remove(&psi_edge) {
                        provenance.edges.push(psi_edge);
                    }
                }
            }
            _ => {}
        }
    }
    debug_assert!(operations.is_empty());
    debug_assert!(edges.is_empty());
    provenance
}
