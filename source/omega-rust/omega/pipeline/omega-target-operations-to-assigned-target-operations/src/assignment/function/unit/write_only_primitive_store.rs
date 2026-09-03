//! Independent assignment replay for a whole-root primitive store.

use crate::assignment::shared::*;
use omega_calling_conventions::ValuePlacement;
use omega_target_operations::TargetUnitBody;
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
    source: TargetUnitScalarArgumentSource,
    preceding_operations: &[TargetUnitOperation],
) -> Result<AssignedUnitOperation, AssignmentError> {
    let invalid = || AssignmentError::WriteOnlyPrimitiveStoreCustodyMismatch {
        machine,
        operation: psi_operation,
    };
    let parameter_index = usize::try_from(destination.position).map_err(|_| invalid())?;
    let parameter = body.parameters.get(parameter_index).ok_or_else(invalid)?;
    let (expected_scalar_type, expected_shape) = match source {
        TargetUnitScalarArgumentSource::IntegerImmediate { scalar_type, .. } => {
            let referent_shape =
                super::scalar_call::fixed_integer_shape(source.source_value(), scalar_type)
                    .map_err(|_| invalid())?;
            (
                ScalarType::Integer(scalar_type),
                ValueShape::borrowed_reference(referent_shape.byte_size, referent_shape.alignment),
            )
        }
        TargetUnitScalarArgumentSource::BooleanImmediate { .. } => {
            (ScalarType::Boolean, ValueShape::borrowed_reference(1, 1))
        }
        _ => return Err(invalid()),
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
    let assigned_source = super::scalar_call::assign_known_unit_scalar_source(
        source,
        preceding_operations,
        &BTreeMap::new(),
    )
    .ok_or_else(invalid)?;
    if assigned_source.scalar_type() != expected_scalar_type
        || !matches!(
            assigned_source,
            AssignedUnitScalarArgumentSource::IntegerImmediate { .. }
                | AssignedUnitScalarArgumentSource::BooleanImmediate { .. }
        )
    {
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
