//! Exact target lowering for runtime-indexed primitive storage projections.
//! The runtime `u64` index and the stored scalar resolve through the same
//! dominating-source maps every other Unit argument uses; the bounds
//! obligation and declared extent stay on the operation for replay.
use super::super::scalar_abi::fixed_native_scalar_shape;
use super::super::shared::*;
use crate::lowering::control_flow::scalar_sources::ScalarSources;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};

pub(in crate::lowering) fn lower_write_only_indexed_primitive_store(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    scalar_sources: &ScalarSources<'_>,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::WriteOnlyIndexedPrimitiveStore {
        psi_operation,
        destination,
        path,
        index,
        value,
        obligation,
    } = operation
    else {
        unreachable!("indexed store lowering receives only indexed primitive stores")
    };
    let invalid = || LoweringError::UnsupportedWriteOnlyIndexedPrimitiveStore {
        machine: function.machine,
        operation: *psi_operation,
    };
    if !function
        .structural_parameters
        .iter()
        .any(|parameter| parameter == destination)
        || destination.multiplicity == StructuralMultiplicity::Linear
        || (path.is_empty() && destination.multiplicity != StructuralMultiplicity::Unrestricted)
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
    if super::super::structural_layout::indexed_array_projection(
        destination.structural_type,
        path,
        structural_types,
    )
    .map(|(scalar, _)| scalar)
        != Some(value.scalar_type)
    {
        return Err(invalid());
    }
    let unsigned_64 = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
    if index.scalar_type != ScalarType::Integer(unsigned_64)
        || fixed_native_scalar_shape(value.scalar_type).is_none()
    {
        return Err(invalid());
    }
    let index_source = super::super::control_flow::scalar_sources::resolved_source(
        index.value,
        function,
        scalar_sources,
    )
    .map_err(|_| invalid())?;
    if index_source.scalar_type() != index.scalar_type {
        return Err(invalid());
    }
    let source = super::super::control_flow::scalar_sources::resolved_source(
        value.value,
        function,
        scalar_sources,
    )
    .map_err(|_| invalid())?;
    if source.scalar_type() != value.scalar_type {
        return Err(invalid());
    }
    let root_shape = super::super::structural_layout::structural_shape(
        destination.structural_type,
        structural_types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    let expected_shape = ValueShape::borrowed_reference(root_shape.byte_size, root_shape.alignment);
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
    operations.push(TargetUnitOperation::WriteOnlyIndexedPrimitiveStore {
        psi_operation: *psi_operation,
        destination: destination.clone(),
        path: path.clone(),
        index: index_source,
        destination_type: destination_type.clone(),
        destination_placement: target_parameter.placement.clone(),
        source,
        obligation: *obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
