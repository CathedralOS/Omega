//! Normalized foreign scalar-call argument and result projection.

use super::super::super::scalar_abi::fixed_native_integer_shape;
use super::*;

pub(super) fn lower_normalized_foreign_scalar_arguments_with_result(
    boundary: BoundaryMachineId,
    declaration: &psi_terminal::BoundaryMachineDeclaration,
    arguments: &[ValueId],
    boundary_entry_plan: &omega_calling_conventions::BoundaryEntryPlan,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    result_shape: Option<ValueShape>,
    native_callback: Option<&omega_target_operations::TargetNativeCallbackArgument>,
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
    let callback_ordinal = native_callback
        .map(|callback| usize::try_from(callback.application.native_ordinal))
        .transpose()
        .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    let signature = CallSignature {
        parameters: if native_callback.is_some() {
            boundary_entry_plan
                .call
                .parameters
                .iter()
                .map(|placement| placement.shape)
                .collect()
        } else {
            scalar_parameter_shapes.clone()
        },
        result: result_shape,
    };
    let validated = match native_callback {
        Some(callback) => {
            let callback_placement = callback_ordinal
                .and_then(|ordinal| boundary_entry_plan.call.parameters.get(ordinal));
            if callback.registrar_boundary_entry_plan != *boundary_entry_plan
                || callback.application.shape != callback.application.placement.shape
                || callback_placement != Some(&callback.application.placement)
            {
                return Err(LoweringError::InvalidNativeCallbackArgument(
                    callback.terminal_operation,
                ));
            }
            omega_calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
                boundary_entry_plan.clone(),
                &signature,
                &callback.registrar_context,
            )
        }
        None => omega_calling_conventions::validate_boundary_entry_plan(
            boundary_entry_plan.clone(),
            &signature,
        ),
    }
    .map_err(|_| LoweringError::BoundaryRealizationMismatch(boundary))?;
    if arguments.len() != declaration.scalar_parameters.len()
        || validated.plan() != boundary_entry_plan
        || boundary_entry_plan.call.parameters.len()
            != scalar_parameter_shapes.len() + usize::from(native_callback.is_some())
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    arguments
        .iter()
        .zip(&declaration.scalar_parameters)
        .zip(&scalar_parameter_shapes)
        .enumerate()
        .map(
            |(semantic_parameter_index, ((source_value, parameter), shape))| {
                let parameter_index = semantic_parameter_index
                    + usize::from(
                        callback_ordinal.is_some_and(|ordinal| semantic_parameter_index >= ordinal),
                    );
                let placement = boundary_entry_plan
                    .call
                    .parameters
                    .get(parameter_index)
                    .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
                let ScalarType::Integer(integer_type) = parameter else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let Some(known) = scalar_values.get(source_value).copied() else {
                    return Err(LoweringError::BoundaryRealizationMismatch(boundary));
                };
                let source = known.into_target_source(*source_value);
                let placed_byte_size = match placement.locations.as_slice() {
                    [
                        ValueLocation::Register {
                            value_byte_offset: 0,
                            byte_size,
                            ..
                        },
                    ]
                    | [
                        ValueLocation::Stack {
                            value_byte_offset: 0,
                            byte_size,
                            ..
                        },
                    ] => *byte_size,
                    _ => return Err(LoweringError::BoundaryRealizationMismatch(boundary)),
                };
                if known.scalar_type() != *integer_type
                    || placement.shape != *shape
                    || shape.byte_size != placed_byte_size
                    || match source {
                        TargetUnitScalarArgumentSource::Parameter { .. } => true,
                        TargetUnitScalarArgumentSource::IntegerImmediate {
                            scalar_type,
                            value,
                            ..
                        } => psi_core::ScalarTerm::integer(scalar_type, value).is_err(),
                        TargetUnitScalarArgumentSource::BooleanImmediate { .. } => true,
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
    let (declaration_result, result) = match (&declaration.result, result) {
        (psi_terminal::BoundaryMachineResult::Unit, None) => {
            if boundary_entry_plan.call.result.is_some() {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            }
            return Ok(None);
        }
        (
            psi_terminal::BoundaryMachineResult::Scalar(ScalarType::Integer(declaration_result)),
            Some(result),
        ) => {
            let ScalarType::Integer(result_type) = result.scalar_type else {
                return Err(LoweringError::BoundaryRealizationMismatch(boundary));
            };
            (*declaration_result, (result.value, result_type))
        }
        _ => return Err(LoweringError::BoundaryRealizationMismatch(boundary)),
    };
    let (source_value, result_type) = result;
    let shape = fixed_native_integer_shape(result_type)
        .ok_or(LoweringError::BoundaryRealizationMismatch(boundary))?;
    let Some(placement) = boundary_entry_plan.call.result.as_ref() else {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    };
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
    if declaration_result != result_type
        || placement.shape != shape
        || *byte_size != shape.byte_size
    {
        return Err(LoweringError::BoundaryRealizationMismatch(boundary));
    }
    Ok(Some(TargetUnitScalarHomeRequirement {
        defining_operation,
        source_value,
        scalar_type: ScalarType::Integer(result_type),
        shape,
    }))
}
