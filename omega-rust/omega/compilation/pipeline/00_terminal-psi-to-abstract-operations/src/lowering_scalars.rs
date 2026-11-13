/*
  Scalar operation lowering from Terminal Psi into `AbstractOperation` rows.

  Lowers integer constants and relations, wrapping/exact/saturating/trapping
  arithmetic, bitwise and shift operations, integer width/sign conversions,
  Boolean logic, IEEE 754 floating-point operations, and admitted target scalar
  applications.
*/
pub(crate) mod arithmetic {
    use abstract_operations::{
        AbstractOperation, IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
    };
    use semantic_vocabulary::OperationId;
    use terminal_psi::{Operation, OperationKind};

    use crate::lowering::LoweringError;

    pub(crate) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
        use IntegerArithmeticOperator as Operator;
        use IntegerOverflowPolicy as Policy;
        macro_rules! r {
            ($a:expr => $b:expr => $c:expr => $d:expr => $e:expr) => {
                ($a, $b, $c, $d, $e)
            };
        }
        let (policy, operator, obligation, left, right) = match operation.kind.clone() {
            OperationKind::WrappingIntegerAdd { left, right } => {
                (Policy::Wrapping, Operator::Add, None, left, right)
            }
            OperationKind::ExactIntegerAdd {
                left,
                right,
                obligation,
            } => (Policy::Exact, Operator::Add, Some(obligation), left, right),
            OperationKind::SaturatingIntegerAdd { left, right } => {
                (Policy::Saturating, Operator::Add, None, left, right)
            }
            OperationKind::WrappingIntegerSubtract { left, right } => {
                (Policy::Wrapping, Operator::Subtract, None, left, right)
            }
            OperationKind::ExactIntegerSubtract {
                left,
                right,
                obligation,
            } => r!(Policy::Exact => Operator::Subtract => Some(obligation) => left => right),
            OperationKind::SaturatingIntegerSubtract { left, right } => {
                (Policy::Saturating, Operator::Subtract, None, left, right)
            }
            OperationKind::WrappingIntegerMultiply { left, right } => {
                (Policy::Wrapping, Operator::Multiply, None, left, right)
            }
            OperationKind::ExactIntegerMultiply {
                left,
                right,
                obligation,
            } => r!(Policy::Exact => Operator::Multiply => Some(obligation) => left => right),
            OperationKind::SaturatingIntegerMultiply { left, right } => {
                (Policy::Saturating, Operator::Multiply, None, left, right)
            }
            OperationKind::WrappingIntegerDivide {
                left,
                right,
                obligation,
            } => r!(Policy::Wrapping => Operator::Divide => Some(obligation) => left => right),
            OperationKind::ExactIntegerDivide {
                left,
                right,
                obligation,
            } => r!(Policy::Exact => Operator::Divide => Some(obligation) => left => right),
            OperationKind::SaturatingIntegerDivide {
                left,
                right,
                obligation,
            } => r!(Policy::Saturating => Operator::Divide => Some(obligation) => left => right),
            OperationKind::WrappingIntegerRemainder {
                left,
                right,
                obligation,
            } => r!(Policy::Wrapping => Operator::Remainder => Some(obligation) => left => right),
            OperationKind::ExactIntegerRemainder {
                left,
                right,
                obligation,
            } => r!(Policy::Exact => Operator::Remainder => Some(obligation) => left => right),
            OperationKind::SaturatingIntegerRemainder {
                left,
                right,
                obligation,
            } => r!(Policy::Saturating => Operator::Remainder => Some(obligation) => left => right),
            _ => unreachable!("arithmetic router is exhaustive"),
        };
        let Some(scalar_type) = operation.result.expect_scalar().scalar_type.as_integer() else {
            return Err(malformed(policy, operator, operation.id));
        };
        let arithmetic = IntegerArithmetic::new(policy, operator, obligation)
            .ok_or_else(|| malformed(policy, operator, operation.id))?;
        Ok(AbstractOperation::IntegerArithmetic {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            scalar_type,
            arithmetic,
            left,
            right,
        })
    }

    fn malformed(
        policy: IntegerOverflowPolicy,
        operator: IntegerArithmeticOperator,
        operation: OperationId,
    ) -> LoweringError {
        use IntegerArithmeticOperator as Operator;
        use IntegerOverflowPolicy as Policy;
        match (policy, operator) {
            (Policy::Wrapping | Policy::Exact, Operator::Add) => {
                LoweringError::VerifiedWrappingAddMalformed(operation)
            }
            (Policy::Wrapping | Policy::Exact, Operator::Subtract) => {
                LoweringError::VerifiedWrappingSubtractMalformed(operation)
            }
            (Policy::Wrapping | Policy::Exact, Operator::Multiply) => {
                LoweringError::VerifiedWrappingMultiplyMalformed(operation)
            }
            (Policy::Saturating, Operator::Add) => {
                LoweringError::VerifiedSaturatingAddMalformed(operation)
            }
            (Policy::Saturating, Operator::Subtract) => {
                LoweringError::VerifiedSaturatingSubtractMalformed(operation)
            }
            (Policy::Saturating, Operator::Multiply) => {
                LoweringError::VerifiedSaturatingMultiplyMalformed(operation)
            }
            (Policy::Exact, Operator::Divide) => {
                LoweringError::VerifiedExactDivideMalformed(operation)
            }
            (Policy::Exact, Operator::Remainder) => {
                LoweringError::VerifiedExactRemainderMalformed(operation)
            }
            (Policy::Wrapping, Operator::Divide) => {
                LoweringError::VerifiedWrappingDivideMalformed(operation)
            }
            (Policy::Wrapping, Operator::Remainder) => {
                LoweringError::VerifiedWrappingRemainderMalformed(operation)
            }
            (Policy::Saturating, Operator::Divide) => {
                LoweringError::VerifiedSaturatingDivideMalformed(operation)
            }
            (Policy::Saturating, Operator::Remainder) => {
                LoweringError::VerifiedSaturatingRemainderMalformed(operation)
            }
        }
    }
}

pub(crate) mod boolean {
    use abstract_operations::{AbstractOperation, AbstractResult};
    use semantic_vocabulary::ScalarType;
    use terminal_psi::{Operation, OperationKind};

    use crate::lowering::LoweringError;

    pub(crate) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
        Ok(match operation.kind.clone() {
            OperationKind::BooleanConstant { value } => AbstractOperation::BooleanConstant {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                value,
            },
            OperationKind::BooleanStructuralField {
                source,
                path,
                field,
            } => AbstractOperation::StructuralScalarFieldRead {
                psi_operation: operation.id,
                result: AbstractResult {
                    value: operation.result.expect_scalar().id,
                    scalar_type: ScalarType::Boolean,
                },
                source,
                path,
                field,
            },
            OperationKind::BooleanNot { operand } => AbstractOperation::BooleanNot {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                operand,
            },
            OperationKind::BooleanEqual { left, right } => AbstractOperation::BooleanEqual {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                left,
                right,
            },
            _ => unreachable!("boolean router is exhaustive"),
        })
    }
}

pub(crate) mod ieee_float {
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::ScalarType;
    use terminal_psi::{Operation, OperationKind};

    use crate::lowering::LoweringError;

    pub(crate) fn lower(
        operation: &Operation,
        value_types: &std::collections::BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    ) -> Result<AbstractOperation, LoweringError> {
        let invalid = LoweringError::VerifiedIeeeFloatMalformed(operation.id);
        let result = operation.result.scalar().ok_or(invalid.clone())?;
        Ok(match operation.kind.clone() {
            OperationKind::FloatResultPolicy { policy } => {
                let Some(format) = result.scalar_type.as_ieee_float() else {
                    return Err(invalid);
                };
                if !policy.accepts_operand_types(format, |value| value_types.get(&value).copied()) {
                    return Err(invalid);
                }
                AbstractOperation::FloatResultPolicy {
                    psi_operation: operation.id,
                    result: result.id,
                    format,
                    policy,
                }
            }
            OperationKind::IeeeFloatCompare {
                comparison,
                left,
                right,
            } => {
                let Some(ScalarType::IeeeFloat(format)) = value_types.get(&left).copied() else {
                    return Err(invalid);
                };
                if result.scalar_type != ScalarType::Boolean
                    || value_types.get(&right) != Some(&ScalarType::IeeeFloat(format))
                {
                    return Err(invalid);
                }
                AbstractOperation::IeeeFloatCompare {
                    psi_operation: operation.id,
                    result: result.id,
                    comparison,
                    format,
                    left,
                    right,
                }
            }
            OperationKind::IeeeFloatConstant { value } => {
                if result.scalar_type != ScalarType::IeeeFloat(value.format()) {
                    return Err(LoweringError::VerifiedIeeeFloatMalformed(operation.id));
                }
                AbstractOperation::IeeeFloatConstant {
                    psi_operation: operation.id,
                    result: result.id,
                    value,
                }
            }
            OperationKind::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
            } => {
                let Some(format) = result.scalar_type.as_ieee_float() else {
                    return Err(LoweringError::VerifiedIeeeFloatMalformed(operation.id));
                };
                AbstractOperation::NearestIeeeFloatFusedMultiplyAdd {
                    psi_operation: operation.id,
                    result: result.id,
                    format,
                    left,
                    right,
                    addend,
                }
            }
            _ => return Err(invalid),
        })
    }
}

pub(crate) mod integer_bitwise {
    use abstract_operations::AbstractOperation;
    use terminal_psi::{Operation, OperationKind};

    use crate::lowering::LoweringError;

    pub(crate) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
        Ok(match operation.kind.clone() {
            OperationKind::IntegerBitwiseNot { operand } => {
                let Some(scalar_type) = operation.result.expect_scalar().scalar_type.as_integer()
                else {
                    return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
                };
                AbstractOperation::IntegerBitwiseNot {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    scalar_type,
                    operand,
                }
            }
            OperationKind::IntegerBitwiseAnd { left, right }
            | OperationKind::IntegerBitwiseOr { left, right }
            | OperationKind::IntegerBitwiseXor { left, right } => {
                let Some(scalar_type) = operation.result.expect_scalar().scalar_type.as_integer()
                else {
                    return Err(LoweringError::VerifiedIntegerBitwiseMalformed(operation.id));
                };
                match operation.kind.clone() {
                    OperationKind::IntegerBitwiseAnd { .. } => {
                        AbstractOperation::IntegerBitwiseAnd {
                            psi_operation: operation.id,
                            result: operation.result.expect_scalar().id,
                            scalar_type,
                            left,
                            right,
                        }
                    }
                    OperationKind::IntegerBitwiseOr { .. } => AbstractOperation::IntegerBitwiseOr {
                        psi_operation: operation.id,
                        result: operation.result.expect_scalar().id,
                        scalar_type,
                        left,
                        right,
                    },
                    OperationKind::IntegerBitwiseXor { .. } => {
                        AbstractOperation::IntegerBitwiseXor {
                            psi_operation: operation.id,
                            result: operation.result.expect_scalar().id,
                            scalar_type,
                            left,
                            right,
                        }
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!("integer-bitwise router is exhaustive"),
        })
    }
}

pub(crate) mod integer_constants_and_relations {
    use abstract_operations::AbstractOperation;
    use terminal_psi::{Operation, OperationKind};

    use crate::lowering::LoweringError;

    pub(crate) fn lower(operation: &Operation) -> Result<AbstractOperation, LoweringError> {
        Ok(match operation.kind.clone() {
            OperationKind::IntegerConstant { value } => AbstractOperation::IntegerConstant {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                scalar_type: operation.result.expect_scalar().scalar_type,
                value,
            },
            OperationKind::IntegerEqual { left, right } => AbstractOperation::IntegerEqual {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                left,
                right,
            },
            OperationKind::IntegerLessThan { left, right } => AbstractOperation::IntegerLessThan {
                psi_operation: operation.id,
                result: operation.result.expect_scalar().id,
                left,
                right,
            },
            OperationKind::IntegerLessOrEqual { left, right } => {
                AbstractOperation::IntegerLessOrEqual {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    left,
                    right,
                }
            }
            _ => unreachable!("integer constant-and-relation router is exhaustive"),
        })
    }
}

pub(crate) mod integer_conversion {
    use std::collections::BTreeMap;

    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::ScalarType;
    use terminal_psi::{Operation, OperationKind};

    use crate::lowering::LoweringError;

    pub(crate) fn lower(
        operation: &Operation,
        value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    ) -> Result<AbstractOperation, LoweringError> {
        Ok(match operation.kind.clone() {
            OperationKind::IntegerWiden { operand } => {
                let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied()
                else {
                    return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
                };
                let Some(target_type) = operation.result.expect_scalar().scalar_type.as_integer()
                else {
                    return Err(LoweringError::VerifiedIntegerWidenMalformed(operation.id));
                };
                AbstractOperation::IntegerWiden {
                    psi_operation: operation.id,
                    result: operation.result.expect_scalar().id,
                    source_type,
                    target_type,
                    operand,
                }
            }
            OperationKind::IntegerExactCast {
                operand,
                obligation,
            } => {
                let Some(ScalarType::Integer(source_type)) = value_types.get(&operand).copied()
                else {
                    return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                        operation.id,
                    ));
                };
                let Some(target_type) = operation.result.expect_scalar().scalar_type.as_integer()
                else {
                    return Err(LoweringError::VerifiedIntegerExactCastMalformed(
                        operation.id,
                    ));
                };
                AbstractOperation::IntegerExactCast {
                    psi_operation: operation.id,
                    obligation,
                    result: operation.result.expect_scalar().id,
                    source_type,
                    target_type,
                    operand,
                }
            }
            OperationKind::ScalarRepresentationView { operand, .. } => {
                let result = operation.result.expect_scalar();
                let source_type = value_types.get(&operand).copied().ok_or(
                    LoweringError::VerifiedScalarRepresentationViewMalformed(operation.id),
                )?;
                terminal_semantics::ScalarRepresentationViewPair::classify(
                    source_type,
                    result.scalar_type,
                )
                .ok_or(
                    LoweringError::VerifiedScalarRepresentationViewMalformed(operation.id),
                )?;
                AbstractOperation::ScalarRepresentationView {
                    psi_operation: operation.id,
                    result: result.id,
                    source_type,
                    target_type: result.scalar_type,
                    operand,
                }
            }
            _ => unreachable!("integer-conversion router is exhaustive"),
        })
    }
}

pub(crate) mod scalar_applications {
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::ScalarApplicationId;
    use terminal_installation::{AdmittedScalarApplications, TerminalScalarValue};
    use terminal_psi::Operation;

    use crate::lowering::LoweringError;

    pub(crate) fn lower(
        operation: &Operation,
        application: ScalarApplicationId,
        completed: Option<&AdmittedScalarApplications>,
    ) -> Result<AbstractOperation, LoweringError> {
        let value = completed
            .and_then(|completed| completed.values().value(application))
            .ok_or(LoweringError::UnclosedScalarApplication(application))?;
        let result = operation
            .result
            .scalar()
            .filter(|result| result.scalar_type == value.scalar_type())
            .ok_or(LoweringError::ScalarApplicationCarrierMismatch(
                operation.id,
            ))?;
        Ok(match value {
            TerminalScalarValue::Callable { .. } => {
                // Source scalar applications currently publish primitive results.
                // Callable results also need retained code and call dependencies.
                return Err(LoweringError::UnadmittedCallableScalarApplication {
                    operation: operation.id,
                    application,
                });
            }
            TerminalScalarValue::Boolean(value) => AbstractOperation::BooleanConstant {
                psi_operation: operation.id,
                result: result.id,
                value,
            },
            TerminalScalarValue::Integer { value, .. } => AbstractOperation::IntegerConstant {
                psi_operation: operation.id,
                result: result.id,
                scalar_type: result.scalar_type,
                value,
            },
            TerminalScalarValue::IeeeFloat(value) => AbstractOperation::IeeeFloatConstant {
                psi_operation: operation.id,
                result: result.id,
                value,
            },
        })
    }
}

pub(crate) mod shifts {
    use std::collections::BTreeMap;

    use abstract_operations::{AbstractOperation, IntegerShiftDirection, IntegerShiftPolicy};
    use semantic_vocabulary::ScalarType;
    use terminal_psi::{Operation, OperationKind};

    use crate::lowering::LoweringError;

    pub(crate) fn lower(
        operation: &Operation,
        value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    ) -> Result<AbstractOperation, LoweringError> {
        let (policy, direction, value, count) = match operation.kind.clone() {
            OperationKind::WrappingIntegerShiftLeft { value, count } => (
                IntegerShiftPolicy::Wrapping,
                IntegerShiftDirection::Left,
                value,
                count,
            ),
            OperationKind::WrappingIntegerShiftRight { value, count } => (
                IntegerShiftPolicy::Wrapping,
                IntegerShiftDirection::Right,
                value,
                count,
            ),
            OperationKind::ExactIntegerShiftLeft {
                value,
                count,
                obligation,
            } => (
                IntegerShiftPolicy::Exact { obligation },
                IntegerShiftDirection::Left,
                value,
                count,
            ),
            OperationKind::ExactIntegerShiftRight {
                value,
                count,
                obligation,
            } => (
                IntegerShiftPolicy::Exact { obligation },
                IntegerShiftDirection::Right,
                value,
                count,
            ),
            _ => unreachable!("shift router is exhaustive"),
        };
        let malformed = match policy {
            IntegerShiftPolicy::Wrapping => {
                LoweringError::VerifiedWrappingShiftMalformed(operation.id)
            }
            IntegerShiftPolicy::Exact { .. } => {
                LoweringError::VerifiedExactShiftMalformed(operation.id)
            }
        };
        let Some(value_type) = operation.result.expect_scalar().scalar_type.as_integer() else {
            return Err(malformed);
        };
        let Some(ScalarType::Integer(count_type)) = value_types.get(&count).copied() else {
            return Err(malformed);
        };
        Ok(AbstractOperation::IntegerShift {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            value_type,
            count_type,
            policy,
            direction,
            value,
            count,
        })
    }
}

pub(crate) mod trapping_integer {
    use std::collections::BTreeMap;

    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::ScalarType;
    use terminal_psi::{Operation, OperationKind, TrappingIntegerOperation};

    use crate::lowering::LoweringError;

    pub(crate) fn lower(
        operation: &Operation,
        value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    ) -> Result<AbstractOperation, LoweringError> {
        let OperationKind::TrappingInteger {
            operation: primitive,
        } = operation.kind
        else {
            unreachable!("the Trapping router dispatches only TrappingInteger");
        };
        let malformed = || LoweringError::VerifiedTrappingIntegerMalformed(operation.id);
        let Some(scalar_type) = operation.result.expect_scalar().scalar_type.as_integer() else {
            return Err(malformed());
        };
        let operand_type = |value| match value_types.get(&value) {
            Some(ScalarType::Integer(integer)) => Ok(*integer),
            _ => Err(malformed()),
        };
        let operand_type = match primitive {
            terminal_psi::trapping_arith_set_pattern!(left, right, ..) => {
                if operand_type(left)? != scalar_type || operand_type(right)? != scalar_type {
                    return Err(malformed());
                }
                scalar_type
            }
            TrappingIntegerOperation::ShiftLeft { value, count }
            | TrappingIntegerOperation::ShiftRight { value, count } => {
                if operand_type(value)? != scalar_type {
                    return Err(malformed());
                }
                operand_type(count)?
            }
            TrappingIntegerOperation::Convert { operand } => operand_type(operand)?,
        };
        Ok(AbstractOperation::TrappingInteger {
            psi_operation: operation.id,
            result: operation.result.expect_scalar().id,
            scalar_type,
            operand_type,
            operation: primitive,
        })
    }
}
