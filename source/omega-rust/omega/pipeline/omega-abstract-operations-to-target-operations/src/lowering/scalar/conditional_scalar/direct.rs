//! Calls, boolean production, and scalar comparisons.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn try_lower_direct_scalar(
    operation: &AbstractOperation,
    machine: MachineId,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    provenance: &mut Vec<psi_core::OperationId>,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    structural_parameters: &[TargetStructuralParameter],
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
) -> Result<bool, LoweringError> {
    if let AbstractOperation::Call {
        psi_operation,
        result,
        scalar_type,
        callee,
        arguments,
    } = operation
    {
        let value = lower_call(
            *psi_operation,
            *result,
            *scalar_type,
            *callee,
            arguments,
            values,
            target,
            functions,
        )?;
        insert_value(values, *result, value)?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::BooleanConstant {
        psi_operation,
        result,
        value,
    } = operation
    {
        insert_value(values, *result, KnownScalar::Boolean(*value))?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::BooleanStructuralField {
        psi_operation,
        result,
        source,
        field,
    } = operation
    {
        let parameter = structural_parameters
            .iter()
            .find(|parameter| parameter.place == *source)
            .ok_or(LoweringError::UnsupportedOperationInScalarFunction(machine))?;
        let field_byte_offset =
            direct_boolean_field_offset(parameter.structural_type, *field, structural_types)?;
        insert_value(
            values,
            *result,
            KnownScalar::BooleanRuntime(TargetBooleanExpression::StructuralField {
                psi_operation: *psi_operation,
                source_value: *result,
                source: *source,
                field: *field,
                source_placement: parameter.placement.clone(),
                field_byte_offset,
            }),
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::BooleanNot {
        psi_operation,
        result,
        operand,
    } = operation
    {
        let operand = values
            .get(operand)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*operand))?;
        insert_value(
            values,
            *result,
            negate_boolean(operand, *psi_operation, *result)?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::BooleanEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let left_value = values
            .get(left)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*left))?;
        let right_value = values
            .get(right)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*right))?;
        insert_value(
            values,
            *result,
            equal_boolean(left_value, right_value, *psi_operation, *result)?,
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::IntegerEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let left_value = values
            .get(left)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*left))?;
        let right_value = values
            .get(right)
            .cloned()
            .ok_or(LoweringError::UnknownValue(*right))?;
        insert_value(
            values,
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
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::IntegerLessThan {
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
    } = operation
    {
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
            values,
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
        provenance.push(*psi_operation);
        return Ok(true);
    }
    Ok(false)
}
