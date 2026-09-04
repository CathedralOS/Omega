//! Independent assignment replay for a whole-root primitive store.

use crate::assignment::shared::*;
use omega_assigned_target_operations::AssignedUnitWriteOnlyPrimitiveStoreSource;
use omega_calling_conventions::ValuePlacement;
use omega_target_operations::{TargetUnitBody, TargetUnitWriteOnlyPrimitiveStoreSource};
use psi_core::ScalarType;
use psi_terminal::{StructuralParameterDeclaration, StructuralTypeDeclaration};

#[allow(clippy::too_many_arguments)]
pub(super) fn assign(
    machine: MachineId,
    body: &TargetUnitBody,
    psi_operation: OperationId,
    destination: &StructuralParameterDeclaration,
    destination_type: &StructuralTypeDeclaration,
    destination_placement: &ValuePlacement,
    source: TargetUnitWriteOnlyPrimitiveStoreSource,
    preceding_operations: &[TargetUnitOperation],
    preceding_assigned_operations: &[AssignedUnitOperation],
    target: NativeTarget,
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::WriteOnlyPrimitiveStoreCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    let parameter_index = usize::try_from(destination.position).map_err(|_| invalid())?;
    let parameter = body.parameters.get(parameter_index).ok_or_else(invalid)?;
    let (expected_scalar_type, expected_shape) = match source {
        TargetUnitWriteOnlyPrimitiveStoreSource::Parameter { scalar_type, .. } => {
            let shape = match scalar_type {
                ScalarType::Boolean => ValueShape::borrowed_reference(1, 1),
                ScalarType::Integer(integer) => {
                    let referent_shape =
                        super::scalar_call::fixed_integer_shape(source.source_value(), integer)
                            .map_err(|_| invalid())?;
                    ValueShape::borrowed_reference(
                        referent_shape.byte_size,
                        referent_shape.alignment,
                    )
                }
                ScalarType::IeeeFloat(_) => return Err(invalid()),
            };
            (scalar_type, shape)
        }
        TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate { scalar_type, .. } => {
            let referent_shape =
                super::scalar_call::fixed_integer_shape(source.source_value(), scalar_type)
                    .map_err(|_| invalid())?;
            (
                ScalarType::Integer(scalar_type),
                ValueShape::borrowed_reference(referent_shape.byte_size, referent_shape.alignment),
            )
        }
        TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate { .. } => {
            (ScalarType::Boolean, ValueShape::borrowed_reference(1, 1))
        }
        TargetUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate { value, .. } => {
            let byte_size = match value.format() {
                psi_core::IeeeFloatFormat::Binary32 => 4,
                psi_core::IeeeFloatFormat::Binary64 => 8,
            };
            (
                ScalarType::IeeeFloat(value.format()),
                ValueShape::borrowed_reference(byte_size, byte_size),
            )
        }
        TargetUnitWriteOnlyPrimitiveStoreSource::Home(home) => {
            let ScalarType::Integer(integer) = home.scalar_type else {
                return Err(invalid());
            };
            let referent_shape =
                super::scalar_call::fixed_integer_shape(home.source_value, integer)
                    .map_err(|_| invalid())?;
            if home.shape != referent_shape {
                return Err(invalid());
            }
            (
                home.scalar_type,
                ValueShape::borrowed_reference(referent_shape.byte_size, referent_shape.alignment),
            )
        }
    };
    if destination.is_self
        || destination.multiplicity != psi_terminal::StructuralMultiplicity::Unrestricted
        || !matches!(
            destination.access,
            psi_terminal::StructuralAccess::MutableBorrow
                | psi_terminal::StructuralAccess::WriteOnlyBorrow
        )
        || !destination.qualifications.is_empty()
        || !destination.projected_qualifications.is_empty()
        || destination_type.id != destination.structural_type
        || destination_type.shape
            != psi_terminal::StructuralTypeShape::PrimitiveScalar(expected_scalar_type)
        || !body
            .structural_types
            .iter()
            .any(|candidate| candidate == destination_type)
        || parameter.place != destination.place
        || parameter.structural_type != destination.structural_type
        || parameter.multiplicity != destination.multiplicity
        || parameter.access != destination.access
        || parameter.projected_qualifications != destination.projected_qualifications
        || parameter.shape != expected_shape
        || parameter.placement != *destination_placement
        || destination_placement.shape != expected_shape
    {
        return Err(invalid());
    }
    let definition_matches = preceding_operations
        .iter()
        .filter(|operation| match (source, operation) {
            (
                TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                    defining_operation,
                    source_value,
                    scalar_type,
                    value,
                },
                TargetUnitOperation::IntegerConstant {
                    psi_operation,
                    result,
                    scalar_type: retained_type,
                    value: retained_value,
                },
            ) => {
                *psi_operation == defining_operation
                    && *result == source_value
                    && *retained_type == scalar_type
                    && *retained_value == value
            }
            (
                TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
                    defining_operation,
                    source_value,
                    value,
                },
                TargetUnitOperation::BooleanConstant {
                    psi_operation,
                    result,
                    value: retained_value,
                },
            ) => {
                *psi_operation == defining_operation
                    && *result == source_value
                    && *retained_value == value
            }
            (
                TargetUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
                    defining_operation,
                    source_value,
                    value,
                },
                TargetUnitOperation::IeeeFloatConstant {
                    psi_operation,
                    result,
                    value: retained_value,
                },
            ) => {
                *psi_operation == defining_operation
                    && *result == source_value
                    && *retained_value == value
            }
            _ => false,
        })
        .count();
    if !matches!(
        source,
        TargetUnitWriteOnlyPrimitiveStoreSource::Parameter { .. }
            | TargetUnitWriteOnlyPrimitiveStoreSource::Home(_)
    ) && definition_matches != 1
    {
        return Err(invalid());
    }
    let assigned_source = match source {
        TargetUnitWriteOnlyPrimitiveStoreSource::Parameter {
            parameter_index,
            source_value,
            scalar_type,
        } => {
            let parameter_index_usize = usize::try_from(parameter_index).map_err(|_| invalid())?;
            let parameter = body
                .scalar_parameters
                .get(parameter_index_usize)
                .filter(|parameter| {
                    parameter.value == source_value && parameter.scalar_type == scalar_type
                })
                .ok_or_else(invalid)?;
            if body.call_plan.parameters.get(parameter_index_usize) != Some(&parameter.placement) {
                return Err(invalid());
            }
            let location = match parameter.placement.locations.as_slice() {
                [
                    ValueLocation::Register {
                        register,
                        value_byte_offset: 0,
                        byte_size,
                    },
                ] if *byte_size == parameter.placement.shape.byte_size => {
                    crate::assignment::placement::require_register_architecture(
                        source_value,
                        *register,
                        target.architecture,
                    )?;
                    AssignedScalarLocation::Register(*register)
                }
                [
                    ValueLocation::Stack {
                        stack_byte_offset,
                        value_byte_offset: 0,
                        byte_size,
                        ..
                    },
                ] if *byte_size == parameter.placement.shape.byte_size => {
                    AssignedScalarLocation::IncomingStack {
                        byte_offset: *stack_byte_offset,
                    }
                }
                _ => return Err(invalid()),
            };
            AssignedUnitWriteOnlyPrimitiveStoreSource::Parameter {
                parameter_index,
                source_value,
                scalar_type,
                location,
            }
        }
        TargetUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
            defining_operation,
            source_value,
            scalar_type,
            value,
        } => {
            if psi_core::ScalarTerm::integer(scalar_type, value).is_err() {
                return Err(invalid());
            }
            AssignedUnitWriteOnlyPrimitiveStoreSource::IntegerImmediate {
                defining_operation,
                source_value,
                scalar_type,
                value,
            }
        }
        TargetUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        } => AssignedUnitWriteOnlyPrimitiveStoreSource::BooleanImmediate {
            defining_operation,
            source_value,
            value,
        },
        TargetUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
        } => AssignedUnitWriteOnlyPrimitiveStoreSource::IeeeFloatImmediate {
            defining_operation,
            source_value,
            value,
        },
        TargetUnitWriteOnlyPrimitiveStoreSource::Home(home) => {
            let target_matches = preceding_operations
                .iter()
                .filter(|operation| {
                    matches!(
                        operation,
                        TargetUnitOperation::ScalarCall { result_home, .. }
                            if *result_home == home
                    )
                })
                .count();
            let assigned = preceding_assigned_operations
                .iter()
                .filter_map(|operation| match operation {
                    AssignedUnitOperation::ScalarCall { result_home, .. }
                        if result_home.defining_operation == home.defining_operation
                            && result_home.source_value == home.source_value
                            && result_home.scalar_type == home.scalar_type
                            && result_home.shape == home.shape =>
                    {
                        Some(*result_home)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let [assigned] = assigned.as_slice() else {
                return Err(invalid());
            };
            if target_matches != 1 {
                return Err(invalid());
            }
            AssignedUnitWriteOnlyPrimitiveStoreSource::Home(*assigned)
        }
    };
    if assigned_source.scalar_type() != expected_scalar_type {
        return Err(invalid());
    }
    Ok(AssignedUnitOperation::WriteOnlyPrimitiveStore {
        psi_operation,
        destination: destination.clone(),
        destination_type: destination_type.clone(),
        destination_placement: destination_placement.clone(),
        source: assigned_source,
    })
}
