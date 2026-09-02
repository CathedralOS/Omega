use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &AbstractOperation,
    operation_index: usize,
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    mut values: &mut BTreeMap<ValueId, KnownScalar>,
    function_result: AbstractResult,
    call_plan: &CallPlan,
    target_structural_parameters: &[TargetStructuralParameter],
    provenance: &mut TerminalPsiProvenance,
    structural_scalar_field_stores: &mut Vec<TargetScalarStructuralFieldStore>,
    returned: &mut Option<TargetOperation>,
) -> Result<(), LoweringError> {
    match operation {
        AbstractOperation::DynamicDescriptorParameter { .. } => {}
        AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. } => {
            return Err(LoweringError::UnsupportedWriteOnlyPrimitiveStore {
                machine: function.machine,
                operation: *psi_operation,
            });
        }
        AbstractOperation::StructuralScalarFieldStore { .. } => {
            if structural_scalar_field_stores.len() >= 3 {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            let store = structural_scalar_field::lower_store(
                operation,
                operation_index,
                function,
                structural_types,
                target_structural_parameters,
                values,
                provenance,
            )?;
            if structural_scalar_field_stores.iter().any(|earlier| {
                earlier.destination.place == store.destination.place
                    && earlier.path == store.path
                    && earlier.field == store.field
            }) {
                return Err(LoweringError::UnsupportedOperationInScalarFunction(
                    function.machine,
                ));
            }
            structural_scalar_field_stores.push(store);
        }
        AbstractOperation::EstablishPayloadlessCase { psi_operation, .. }
        | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. } => {
            return Err(LoweringError::UnitOperationInScalarFunction {
                machine: function.machine,
                operation: *psi_operation,
            });
        }
        AbstractOperation::BoundaryCall {
            psi_operation,
            result,
            boundary,
            ..
        } => {
            if result.is_some() {
                return Err(
                    LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                        machine: function.machine,
                        operation: *psi_operation,
                        boundary: *boundary,
                    },
                );
            }
            return Err(LoweringError::UnitOperationInScalarFunction {
                machine: function.machine,
                operation: *psi_operation,
            });
        }
        AbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
        | AbstractOperation::EstablishAffineScalarRecord { psi_operation, .. }
        | AbstractOperation::CallUnit { psi_operation, .. }
        | AbstractOperation::PortWrite { psi_operation, .. } => {
            return Err(LoweringError::UnitOperationInScalarFunction {
                machine: function.machine,
                operation: *psi_operation,
            });
        }
        AbstractOperation::CallUnitWithDynamicArguments { psi_operation, .. }
        | AbstractOperation::CallDynamicUnit { psi_operation, .. }
        | AbstractOperation::CallDynamicParameterUnit { psi_operation, .. } => {
            return Err(LoweringError::UnsupportedDynamicUnitDispatch {
                machine: function.machine,
                operation: *psi_operation,
            });
        }
        AbstractOperation::CallStructuralScalar { .. }
        | AbstractOperation::CallStructuralScalarWithDynamicArguments { .. }
        | AbstractOperation::CallDynamicScalar { .. }
        | AbstractOperation::CallDynamicParameterScalar { .. }
        | AbstractOperation::CallStructural { .. } => {
            return Err(LoweringError::UnsupportedOperationInScalarFunction(
                function.machine,
            ));
        }
        AbstractOperation::Call { .. } => {
            call::lower(operation, target, functions, values, provenance)?
        }
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } => {
            let ScalarType::Integer(integer_type) = scalar_type else {
                return Err(LoweringError::IntegerConstantHasNonIntegerType(*result));
            };
            if !integer_type.admits(*value) {
                return Err(LoweringError::IntegerConstantOutsideType(*result));
            }
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *integer_type,
                    value: KnownInteger::Immediate(*value),
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::IeeeFloatConstant { .. }
        | AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { .. } => {
            return Err(LoweringError::UnsupportedOperationInScalarFunction(
                function.machine,
            ));
        }
        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            value,
        } => {
            insert_value(&mut values, *result, KnownScalar::Boolean(*value))?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::BooleanStructuralField { .. }
        | AbstractOperation::IntegerStructuralField { .. } => structural_scalar_field::lower(
            operation,
            function,
            structural_types,
            target_structural_parameters,
            values,
            provenance,
        )?,
        AbstractOperation::BooleanNot {
            psi_operation,
            result,
            operand,
        } => {
            let operand = values
                .get(operand)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*operand))?;
            insert_value(
                &mut values,
                *result,
                negate_boolean(operand, *psi_operation, *result)?,
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::BooleanEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let left = values
                .get(left)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*left))?;
            let right = values
                .get(right)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*right))?;
            insert_value(
                &mut values,
                *result,
                equal_boolean(left, right, *psi_operation, *result)?,
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::IntegerEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let left_value = values
                .get(left)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*left))?;
            let right_value = values
                .get(right)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*right))?;
            insert_value(
                &mut values,
                *result,
                equal_integer(
                    *left,
                    left_value,
                    *right,
                    right_value,
                    *psi_operation,
                    *result,
                )?,
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::IntegerLessThan {
            psi_operation,
            result,
            left,
            right,
        }
        | AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result,
            left,
            right,
        } => {
            let left_value = values
                .get(left)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*left))?;
            let right_value = values
                .get(right)
                .cloned()
                .ok_or(LoweringError::UnknownValue(*right))?;
            let inclusive = matches!(operation, AbstractOperation::IntegerLessOrEqual { .. });
            insert_value(
                &mut values,
                *result,
                order_integer(
                    *left,
                    left_value,
                    *right,
                    right_value,
                    *psi_operation,
                    *result,
                    inclusive,
                )?,
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::IntegerBitwiseOr {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        }
        | AbstractOperation::IntegerBitwiseXor {
            psi_operation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let kind = match operation {
                AbstractOperation::IntegerBitwiseAnd { .. } => IntegerBinaryKind::BitwiseAnd,
                AbstractOperation::IntegerBitwiseOr { .. } => IntegerBinaryKind::BitwiseOr,
                AbstractOperation::IntegerBitwiseXor { .. } => IntegerBinaryKind::BitwiseXor,
                _ => unreachable!(),
            };
            let value = lower_conditional_integer_binary(
                &values,
                *result,
                *scalar_type,
                *left,
                *right,
                kind,
                *psi_operation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *scalar_type,
                    value,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::IntegerBitwiseNot { .. }
        | AbstractOperation::IntegerWiden { .. }
        | AbstractOperation::IntegerExactCast { .. } => {
            integer_conversion::lower_integer_conversion(operation, values, provenance)?
        }
        AbstractOperation::WrappingIntegerShiftLeft {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        }
        | AbstractOperation::WrappingIntegerShiftRight {
            psi_operation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => {
            let kind = if matches!(
                operation,
                AbstractOperation::WrappingIntegerShiftLeft { .. }
            ) {
                WrappingShiftKind::Left
            } else {
                WrappingShiftKind::Right
            };
            let shifted = lower_wrapping_shift(
                &values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                kind,
                *psi_operation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *value_type,
                    value: shifted,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::ExactIntegerShiftRight {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => {
            let shifted = lower_exact_shift_right(
                &values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *psi_operation,
                *obligation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *value_type,
                    value: shifted,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::ExactIntegerShiftLeft {
            psi_operation,
            obligation,
            result,
            value_type,
            count_type,
            value,
            count,
        } => {
            let shifted = lower_exact_shift_left(
                &values,
                *result,
                *value_type,
                *count_type,
                *value,
                *count,
                *psi_operation,
                *obligation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *value_type,
                    value: shifted,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::WrappingIntegerAdd { .. }
        | AbstractOperation::ExactIntegerAdd { .. }
        | AbstractOperation::SaturatingIntegerAdd { .. }
        | AbstractOperation::WrappingIntegerSubtract { .. }
        | AbstractOperation::ExactIntegerSubtract { .. }
        | AbstractOperation::SaturatingIntegerSubtract { .. }
        | AbstractOperation::WrappingIntegerMultiply { .. }
        | AbstractOperation::ExactIntegerMultiply { .. }
        | AbstractOperation::SaturatingIntegerMultiply { .. } => {
            integer_arithmetic::lower_integer_arithmetic(operation, values, provenance)?
        }
        AbstractOperation::ExactIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let value = lower_conditional_integer_binary(
                &values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactDivide(*obligation),
                *psi_operation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *scalar_type,
                    value,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::ExactIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let value = lower_conditional_integer_binary(
                &values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::ExactRemainder(*obligation),
                *psi_operation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *scalar_type,
                    value,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::WrappingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let value = lower_conditional_integer_binary(
                &values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingDivide(*obligation),
                *psi_operation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *scalar_type,
                    value,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::WrappingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let value = lower_conditional_integer_binary(
                &values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::WrappingRemainder(*obligation),
                *psi_operation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *scalar_type,
                    value,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::SaturatingIntegerDivide {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let value = lower_conditional_integer_binary(
                &values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingDivide(*obligation),
                *psi_operation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *scalar_type,
                    value,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::SaturatingIntegerRemainder {
            psi_operation,
            obligation,
            result,
            scalar_type,
            left,
            right,
        } => {
            let value = lower_conditional_integer_binary(
                &values,
                *result,
                *scalar_type,
                *left,
                *right,
                IntegerBinaryKind::SaturatingRemainder(*obligation),
                *psi_operation,
            )?;
            insert_value(
                &mut values,
                *result,
                KnownScalar::Integer {
                    scalar_type: *scalar_type,
                    value,
                },
            )?;
            provenance.operations.push(*psi_operation);
        }
        AbstractOperation::Jump {
            psi_edge,
            bindings,
            trivial_affine_discards,
            ..
        } => {
            // This ownership-only edge work is deliberately erased after
            // Terminal verification (and optimizer admission when
            // selected); it has no target instruction.
            let _ = trivial_affine_discards;
            let transferred = bindings
                .iter()
                .map(|binding| {
                    let value = values
                        .get(&binding.argument)
                        .cloned()
                        .ok_or(LoweringError::UnknownValue(binding.argument))?;
                    if binding.scalar_type != value.scalar_type() {
                        return Err(LoweringError::ValueTypeMismatch(binding.parameter));
                    }
                    Ok((binding.parameter, value))
                })
                .collect::<Result<Vec<_>, _>>()?;
            for (parameter, value) in transferred {
                insert_value(&mut values, parameter, value)?;
            }
            provenance.edges.push(*psi_edge);
        }
        AbstractOperation::Conditional { .. } => {
            return Err(LoweringError::ConditionalControlFlowRequiresBlockLowering(
                function.machine,
            ));
        }
        AbstractOperation::Crash { .. }
        | AbstractOperation::Return { .. }
        | AbstractOperation::ReturnUnit { .. }
        | AbstractOperation::ReturnStructural { .. } => exit::lower_exit(
            operation,
            function,
            function_result,
            values,
            functions,
            structural_types,
            call_plan,
            target_structural_parameters,
            provenance,
            returned,
        )?,
    }
    Ok(())
}
