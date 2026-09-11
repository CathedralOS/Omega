//! Exact projected integer store and structural-scalar call lowering for an
//! attached Unit body.

mod dynamic_arguments;

use super::super::scalar::scalar_shape;
use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::super::structural_layout::{
    direct_boolean_field_offset, direct_integer_field_offset, direct_scalar_field_offset,
    resolve_structural_field_path, resolve_structural_projection_path, structural_shape,
};
use super::scalar_call::{KnownUnitInteger, insert_known_unit_integer};
pub(in crate::lowering) use dynamic_arguments::{
    lower_dynamic_argument_scalar_call, lower_dynamic_argument_unit_call,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_field_store(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    boolean_constants: &BTreeMap<ValueId, (OperationId, bool)>,
    ieee_float_constants: &BTreeMap<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>,
    shape_cache: &mut BTreeMap<StructuralTypeId, ValueShape>,
    active: &mut BTreeSet<StructuralTypeId>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        path,
        field,
        value,
    } = operation
    else {
        unreachable!("projected field-store lowering receives only field stores")
    };
    if !function
        .structural_parameters
        .iter()
        .any(|parameter| parameter == destination)
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || !terminal_psi::is_bounded_structural_scalar_store_path(path)
    {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let target_parameter = parameters_by_place
        .get(&destination.place)
        .copied()
        .filter(|parameter| {
            parameter.structural_type == destination.structural_type
                && parameter.multiplicity == destination.multiplicity
                && parameter.access == destination.access
                && parameter.projected_qualifications == destination.projected_qualifications
        })
        .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ))?;
    if function.structural_parameters.len() != 1 {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let (source, field_byte_size) = match value.scalar_type {
        ScalarType::Integer(integer_type) => {
            let known_value = scalar_values
                .get(&value.value)
                .copied()
                .ok_or(LoweringError::UnknownValue(value.value))?;
            let exact_source = match known_value {
                KnownUnitInteger::BlockParameter {
                    block,
                    value: source,
                    scalar_type,
                } => {
                    source == value.value
                        && scalar_type == integer_type
                        && function.block_entries.iter().any(|entry| {
                            entry.block == block
                                && entry.parameters.iter().any(|parameter| {
                                    parameter.value == source
                                        && parameter.scalar_type == value.scalar_type
                                })
                        })
                }
                KnownUnitInteger::Parameter {
                    parameter_index,
                    scalar_type,
                } => {
                    let parameter_index = usize::try_from(parameter_index).ok();
                    scalar_type == integer_type
                        && parameter_index
                            .and_then(|index| function.parameters.get(index))
                            .is_some_and(|parameter| {
                                parameter.value == value.value
                                    && parameter.scalar_type == value.scalar_type
                            })
                }
                KnownUnitInteger::Immediate { scalar_type, .. } => scalar_type == integer_type,
                KnownUnitInteger::Home(home) => {
                    home.scalar_type == value.scalar_type
                        && home.source_value == value.value
                        && home.shape
                            == fixed_native_integer_shape(integer_type).ok_or(
                                LoweringError::UnsupportedOperationInUnitFunction(function.machine),
                            )?
                }
            };
            if !exact_source {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
            (
                known_value.into_target_source(value.value),
                integer_type.bits().div_ceil(8),
            )
        }
        ScalarType::Boolean => {
            let source = if let Some((parameter_index, _)) = function
                .parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| {
                    parameter.value == value.value && parameter.scalar_type == ScalarType::Boolean
                }) {
                TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| {
                        LoweringError::UnitFunctionHasScalarParameters(function.machine)
                    })?,
                    source_value: value.value,
                    scalar_type: ScalarType::Boolean,
                }
            } else {
                let (defining_operation, immediate) = boolean_constants
                    .get(&value.value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(value.value))?;
                TargetUnitScalarArgumentSource::BooleanImmediate {
                    defining_operation,
                    source_value: value.value,
                    value: immediate,
                }
            };
            (source, 1)
        }
        ScalarType::IeeeFloat(format) => {
            let source = if let Some((parameter_index, _)) = function
                .parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| {
                    parameter.value == value.value && parameter.scalar_type == value.scalar_type
                }) {
                TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| {
                        LoweringError::UnitFunctionHasScalarParameters(function.machine)
                    })?,
                    source_value: value.value,
                    scalar_type: value.scalar_type,
                }
            } else {
                let (defining_operation, immediate) = ieee_float_constants
                    .get(&value.value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(value.value))?;
                if immediate.format() != format {
                    return Err(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ));
                }
                TargetUnitScalarArgumentSource::IeeeFloatImmediate {
                    defining_operation,
                    source_value: value.value,
                    value: immediate,
                }
            };
            (
                source,
                match format {
                    IeeeFloatFormat::Binary32 => 4,
                    IeeeFloatFormat::Binary64 => 8,
                },
            )
        }
    };
    let (carrier_type, carrier_byte_offset) = if path.is_empty() {
        structural_shape(
            destination.structural_type,
            structural_types,
            shape_cache,
            active,
        )?;
        (destination.structural_type, 0)
    } else {
        let (carrier_type, _, carrier_byte_offset) = resolve_structural_projection_path(
            destination.structural_type,
            path,
            structural_types,
            shape_cache,
            active,
        )?;
        (carrier_type, carrier_byte_offset)
    };
    let scalar_byte_offset = match value.scalar_type {
        ScalarType::Boolean => direct_boolean_field_offset(carrier_type, *field, structural_types)?,
        ScalarType::Integer(integer_type) => {
            direct_integer_field_offset(carrier_type, *field, integer_type, structural_types)?
        }
        ScalarType::IeeeFloat(_) => {
            direct_scalar_field_offset(carrier_type, *field, value.scalar_type, structural_types)?
        }
    };
    let field_byte_offset = carrier_byte_offset
        .checked_add(scalar_byte_offset)
        .filter(|offset| {
            offset
                .checked_add(u32::from(field_byte_size))
                .is_some_and(|end| end <= u32::from(target_parameter.shape.byte_size))
        })
        .ok_or(LoweringError::StructuralTypeTooLarge(
            destination.structural_type,
        ))?;
    operations.push(TargetUnitOperation::StructuralScalarFieldStore {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        field: *field,
        destination_placement: target_parameter.placement.clone(),
        field_byte_offset,
        source,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
