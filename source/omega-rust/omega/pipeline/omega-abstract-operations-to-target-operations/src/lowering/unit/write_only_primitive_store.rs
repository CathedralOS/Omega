//! Exact target lowering for one whole-root non-observing primitive store.

use super::super::scalar_abi::fixed_native_integer_shape;
use super::super::shared::*;
use super::scalar_call::KnownUnitInteger;
use omega_target_operations::TargetUnitWriteOnlyPrimitiveStoreSource;
use psi_core::IeeeFloatValue;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_write_only_primitive_store(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    scalar_values: &BTreeMap<ValueId, KnownUnitInteger>,
    boolean_constants: &BTreeMap<ValueId, (OperationId, bool)>,
    ieee_float_constants: &BTreeMap<ValueId, (OperationId, IeeeFloatValue)>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination,
        value,
    } = operation
    else {
        unreachable!("whole-root store lowering receives only primitive stores")
    };
    let invalid = || LoweringError::UnsupportedWriteOnlyPrimitiveStore {
        machine: function.machine,
        operation: *psi_operation,
    };
    if !function
        .structural_parameters
        .iter()
        .any(|parameter| parameter == destination)
        || destination.multiplicity != StructuralMultiplicity::Unrestricted
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
    {
        return Err(invalid());
    }
    let destination_type = structural_types
        .get(&destination.structural_type)
        .copied()
        .ok_or_else(invalid)?;
    if destination_type.shape != StructuralTypeShape::PrimitiveScalar(value.scalar_type) {
        return Err(invalid());
    }
    let (expected_shape, source) = match value.scalar_type {
        ScalarType::Integer(integer_type) => {
            let referent_shape = fixed_native_integer_shape(integer_type).ok_or_else(invalid)?;
            let Some(known_value) = scalar_values.get(&value.value).copied() else {
                return Err(invalid());
            };
            if known_value.scalar_type() != integer_type {
                return Err(invalid());
            }
            let source = match known_value {
                KnownUnitInteger::Parameter {
                    parameter_index,
                    scalar_type,
                } => TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                    parameter_index,
                    source_value: value.value,
                    scalar_type: ScalarType::Integer(scalar_type),
                },
                KnownUnitInteger::Immediate {
                    defining_operation,
                    scalar_type,
                    value: immediate,
                } => TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                    defining_operation,
                    source_value: value.value,
                    scalar_type,
                    value: immediate,
                },
                KnownUnitInteger::Home(_) => return Err(invalid()),
            };
            (
                ValueShape::borrowed_reference(referent_shape.byte_size, referent_shape.alignment),
                source,
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
                TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
                    parameter_index: u32::try_from(parameter_index).map_err(|_| invalid())?,
                    source_value: value.value,
                    scalar_type: ScalarType::Boolean,
                }
            } else {
                let (defining_operation, immediate) = boolean_constants
                    .get(&value.value)
                    .copied()
                    .ok_or_else(invalid)?;
                TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
                    defining_operation,
                    source_value: value.value,
                    value: immediate,
                }
            };
            (ValueShape::borrowed_reference(1, 1), source)
        }
        ScalarType::IeeeFloat(format) => {
            let (defining_operation, immediate) = ieee_float_constants
                .get(&value.value)
                .copied()
                .ok_or_else(invalid)?;
            if immediate.format() != format {
                return Err(invalid());
            }
            let byte_size = match format {
                IeeeFloatFormat::Binary32 => 4,
                IeeeFloatFormat::Binary64 => 8,
            };
            (
                ValueShape::borrowed_reference(byte_size, byte_size),
                TargetUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
                    defining_operation,
                    source_value: value.value,
                    value: immediate,
                },
            )
        }
    };
    let target_parameter = parameters_by_place
        .get(&destination.place)
        .copied()
        .filter(|parameter| {
            parameter.structural_type == destination.structural_type
                && parameter.multiplicity == destination.multiplicity
                && parameter.access == destination.access
                && parameter.projected_qualifications == destination.projected_qualifications
                && parameter.shape == expected_shape
                && parameter.placement.shape == expected_shape
        })
        .ok_or_else(invalid)?;
    operations.push(TargetUnitOperation::WriteOnlyPrimitiveStore {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        destination_type: destination_type.clone(),
        destination_placement: target_parameter.placement.clone(),
        source,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
