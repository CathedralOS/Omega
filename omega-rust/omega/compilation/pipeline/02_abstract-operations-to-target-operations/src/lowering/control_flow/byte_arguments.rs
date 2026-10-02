//! Shared byte arguments present live owner storage as a call-local descriptor.
//! Storage origin chooses an address, not an admission family: parameters,
//! state arrivals and operation homes all retain their own length word and
//! inline bytes. The verified abstract call owns predicate discharge; source
//! qualifications are not a requirement to impose on an unqualified view.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::PreparedFunctionSignature;
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
        || argument.path.iter().any(|segment| {
            !matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        })
        || declaration.access != StructuralAccess::SharedBorrow
        || !types
            .get(&declaration.structural_type)
            .is_some_and(|declared| {
                matches!(
                    declared.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                        access: None | Some(StructuralAccess::SharedBorrow),
                    })
                )
            })
    {
        return Ok(None);
    }
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
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
            let source = if let Some(parameter) = prepared
                .parameters
                .iter()
                .find(|parameter| parameter.place == argument.place)
            {
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
                    .ok_or_else(unsupported)?;
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
    let geometry = if argument.path.is_empty() {
        types.get(&root_type).and_then(|root| match root.shape {
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }) => {
                Some((0, capacity))
            }
            _ => None,
        })
    } else {
        crate::lowering::structural_layout::bounded_byte_field_geometry(
            root_type,
            &argument.path,
            types,
            &mut shapes,
            &mut active,
        )?
    };
    let Some((offset, capacity)) = geometry else {
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
        return Err(unsupported());
    }
    let root_shape = crate::lowering::structural_layout::structural_shape(
        root_type,
        types,
        &mut shapes,
        &mut active,
    )?;
    if !matches!(
        access,
        StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || multiplicity != StructuralMultiplicity::Unrestricted
        || !projected.is_empty()
        || declaration.is_self
        || declaration.multiplicity != StructuralMultiplicity::Unrestricted
        || !declaration.projected_qualifications.is_empty()
        || destination.shape != ValueShape::borrowed_reference(16, 8)
        || destination.placement.shape != destination.shape
        || u64::from(offset)
            .checked_add(8)
            .and_then(|end| end.checked_add(capacity))
            .is_none_or(|end| end > u64::from(root_shape.byte_size))
    {
        return Err(unsupported());
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
        indices: Vec::new(),
        source,
        destination: destination.placement.clone(),
    }))
}
