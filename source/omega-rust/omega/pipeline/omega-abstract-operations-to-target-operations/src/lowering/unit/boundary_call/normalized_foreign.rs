//! Normalized foreign scalar-call argument and result projection.

use super::*;

pub(super) fn lower_normalized_foreign_scalar_arguments_with_result(
    boundary: BoundaryMachineId,
    declaration: &psi_terminal::BoundaryMachineDeclaration,
    arguments: &[ValueId],
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    result_shape: Option<ValueShape>,
) -> Result<Vec<omega_target_operations::NormalizedForeignScalarArgument>, LoweringError> {
    let scalar_parameter_shapes = declaration
        .scalar_parameters
        .iter()
        .map(|parameter| {
            let ScalarType::Integer(integer_type) = parameter else {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            };
            if integer_type.carrier() != psi_core::IntegerCarrier::Fixed
                || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
            {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            let bytes = integer_type.bits().div_ceil(8);
            Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let signature = CallSignature {
        parameters: scalar_parameter_shapes.clone(),
        result: result_shape,
    };
    let validated = omega_calling_conventions::validate_boundary_entry_plan(
        boundary_entry_plan.clone(),
        &signature,
    )
    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    if arguments.len() != declaration.scalar_parameters.len()
        || validated.plan() != boundary_entry_plan
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    arguments
        .iter()
        .zip(&declaration.scalar_parameters)
        .zip(&scalar_parameter_shapes)
        .zip(&boundary_entry_plan.call.parameters)
        .enumerate()
        .map(
            |(parameter_index, (((source_value, parameter), shape), placement))| {
                let ScalarType::Integer(integer_type) = parameter else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let Some(known) = scalar_values.get(source_value).copied() else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let source = known.into_target_source(*source_value);
                let [
                    ValueLocation::Register {
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] = placement.locations.as_slice()
                else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                if known.scalar_type() != *integer_type
                    || placement.shape != *shape
                    || u16::try_from(shape.byte_size) != Ok(*byte_size)
                    || match source {
                        TargetUnitScalarArgumentSource::IntegerImmediate {
                            scalar_type,
                            value,
                            ..
                        } => psi_core::ScalarTerm::integer(scalar_type, value).is_err(),
                        TargetUnitScalarArgumentSource::Home(home) => home.shape != *shape,
                    }
                {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                }
                Ok(omega_target_operations::NormalizedForeignScalarArgument {
                    parameter_index: u32::try_from(parameter_index)
                        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?,
                    source,
                    placement: placement.clone(),
                })
            },
        )
        .collect()
}

#[cfg(test)]
pub(super) fn lower_normalized_foreign_scalar_arguments(
    boundary: BoundaryMachineId,
    declaration: &psi_terminal::BoundaryMachineDeclaration,
    arguments: &[ValueId],
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
) -> Result<Vec<omega_target_operations::NormalizedForeignScalarArgument>, LoweringError> {
    lower_normalized_foreign_scalar_arguments_with_result(
        boundary,
        declaration,
        arguments,
        boundary_entry_plan,
        scalar_values,
        None,
    )
}

pub(super) fn lower_normalized_foreign_scalar_result(
    boundary: BoundaryMachineId,
    declaration: &psi_terminal::BoundaryMachineDeclaration,
    defining_operation: OperationId,
    result: Option<omega_abstract_operations::AbstractResult>,
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
) -> Result<Option<TargetUnitScalarHomeRequirement>, LoweringError> {
    let (declaration_result, result) = match (declaration.result, result) {
        (None, None) => {
            if boundary_entry_plan.call.result.is_some() {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            return Ok(None);
        }
        (Some(ScalarType::Integer(declaration_result)), Some(result)) => {
            let ScalarType::Integer(result_type) = result.scalar_type else {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            };
            (declaration_result, (result.value, result_type))
        }
        _ => return Err(LoweringError::BoundaryRealizationMismatch(boundary)),
    };
    let (source_value, result_type) = result;
    let expected_type = IntegerType::new(IntegerSign::Signed, 32)
        .expect("signed i32 is a valid fixed integer type");
    let shape = ValueShape::integer(4, 4);
    let Some(placement) = boundary_entry_plan.call.result.as_ref() else {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    };
    if declaration_result != expected_type
        || result_type != expected_type
        || placement.shape != shape
        || !matches!(
            placement.locations.as_slice(),
            [ValueLocation::Register {
                value_byte_offset: 0,
                byte_size: 4,
                ..
            }]
        )
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    Ok(Some(TargetUnitScalarHomeRequirement {
        defining_operation,
        source_value,
        scalar_type: result_type,
        shape,
    }))
}
