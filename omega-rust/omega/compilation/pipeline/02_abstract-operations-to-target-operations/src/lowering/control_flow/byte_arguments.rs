//! Shared byte arguments present live owner storage as a call-local descriptor.
//! Storage origin chooses an address, not an admission family: parameters,
//! state arrivals and operation homes all retain their own length word and
//! inline bytes. The verified abstract call owns predicate discharge; source
//! qualifications are not a requirement to impose on an unqualified view.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::PreparedFunctionSignature;
use crate::lowering::structural_layout::reference_projection_regions;
use crate::lowering::structural_layout::runtime_storage_projection;
use crate::lowering::structural_layout::structural_shape;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::AbstractFunction;
use calling_conventions::ValueShape;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralArgument, TargetStructuralArgumentSource, TargetStructuralHomeOrigin,
    TargetStructuralParameter,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeShape,
};

pub(super) fn shared_owner_view(
    argument: &StructuralArgument,
    declaration: &StructuralParameterDeclaration,
    destination: &TargetStructuralParameter,
    function: &AbstractFunction,
    prepared: &PreparedFunctionSignature,
    live: &LiveDefinitions,
    types: &StructuralTypeLookup<'_>,
) -> Result<Option<TargetStructuralArgument>, LoweringError> {
    if argument.access != StructuralAccess::SharedBorrow
        // Pointer crossings and explicit windows have different physical
        // presentations, owned by their existing call adapters.
        || argument.path.iter().any(|segment| matches!(segment,
            terminal_psi::StructuralPathSegment::Referent
                | terminal_psi::StructuralPathSegment::FixedRange { .. }
        ))
        || declaration.access != StructuralAccess::SharedBorrow
        || !types
            .get(&declaration.structural_type)
            .is_some_and(|declared| declared.shape.is_shared_readable_byte_view())
    {
        return Ok(None);
    }
    let (root_type, access, multiplicity, projected, source) =
        if let Some(home) = live.structural_homes.get(&argument.place) {
            let source = match &home.origin {
                TargetStructuralHomeOrigin::OperationResult { operation, .. } => {
                    TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: *operation,
                    }
                }
                TargetStructuralHomeOrigin::BlockParameter { block, declaration } => {
                    TargetStructuralArgumentSource::BlockParameter {
                        block: *block,
                        place: declaration.place,
                    }
                }
            };
            (
                home.structural_type(),
                home.access(),
                home.multiplicity(),
                home.projected_qualifications(),
                source,
            )
        } else if let Some(arrival) = function.arrival_parameter(argument.place) {
            let source = if let Some(parameter) = prepared.parameter(argument.place) {
                parameter.placement.clone().into()
            } else if live.address_joins.contains(&argument.place) {
                let block = function
                    .block_entries
                    .iter()
                    .find(|block| {
                        block
                            .structural_parameters
                            .iter()
                            .any(|parameter| parameter == arrival)
                    })
                    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
                TargetStructuralArgumentSource::BlockParameter {
                    block: block.block,
                    place: arrival.place,
                }
            } else {
                return Ok(None);
            };
            (
                arrival.structural_type,
                arrival.access,
                arrival.multiplicity,
                arrival.projected_qualifications.as_slice(),
                source,
            )
        } else {
            return Ok(None);
        };
    let mut shapes = BTreeMap::new();
    let mut active = BTreeSet::new();
    // An element view crosses its backing pointer even without an explicit
    // Referent marker. This adapter stages a descriptor over contiguous owner
    // storage; crossing-aware call transport owns other address regions.
    if !reference_projection_regions(root_type, &argument.path, types)
        .ok_or(LoweringError::UnsupportedStructuralReference(root_type))?
        .pointer_slots
        .is_empty()
    {
        return Ok(None);
    }
    let (selected, _, offset, runs) =
        runtime_storage_projection(root_type, &argument.path, types, &mut shapes, &mut active)?;
    let Some(selected_shape) = selected.shape(types) else {
        return Ok(None);
    };
    let StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) =
        *selected_shape
    else {
        return Ok(None);
    };
    // Inspect extra custody only after this operand is an owner presentation;
    // unrelated descriptors keep their own existing admission route.
    if live
        .structural_homes
        .get(&argument.place)
        .is_some_and(|home| {
            home.has_claims()
                || home
                    .operation_result()
                    .is_some_and(|(_, result)| !result.qualification_establishments.is_empty())
        })
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let root_shape = structural_shape(root_type, types, &mut shapes, &mut active)?;
    if !matches!(
        access,
        StructuralAccess::Owned | terminal_psi::borrowed_access_pair_pattern!()
    ) || multiplicity != StructuralMultiplicity::Unrestricted
        || !projected.is_empty()
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.projected_qualifications.is_empty()
        || destination.shape != ValueShape::borrowed_reference(16, 8)
        || destination.placement.shape != destination.shape
        || u64::from(offset)
            .checked_add(8)
            .and_then(|end| end.checked_add(capacity))
            .is_none_or(|end| end > u64::from(root_shape.byte_size))
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(Some(TargetStructuralArgument {
        place: argument.place,
        access: argument.access,
        path: argument.path.clone(),
        root_structural_type: root_type,
        structural_type: declaration.structural_type,
        shape: destination.shape,
        source_byte_offset: offset,
        fixed_array_length: None,
        element_stride: None,
        indices: super::scalar_sources::runtime_indices(
            runs,
            function,
            &super::scalar_sources::ScalarSources::from(live),
        )?,
        source,
        destination: destination.placement.clone(),
    }))
}
