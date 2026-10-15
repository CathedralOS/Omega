//! Exact projected scalar store lowering for an attached Unit body.

use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarArgumentSource,
    TerminalPsiProvenance,
};
use terminal_psi::StructuralAccess;

use super::super::scalar::scalar_shape;
use super::super::scalar_abi::fixed_native_scalar_shape;
use super::super::structural_layout::{
    direct_scalar_field_geometry, runtime_projection, static_field_geometry,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_field_store(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    structural_homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
    reference_destination: Option<crate::lowering::control_flow::references::ReferentProjection>,
    scalar_sources: &crate::lowering::control_flow::scalar_sources::ScalarSources<'_>,
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
        ..
    } = operation
    else {
        unreachable!("projected field-store lowering receives only field stores")
    };
    let unsupported = |reason| LoweringError::InvalidStructuralValueInput {
        machine: function.machine,
        place: destination.place(),
        reason,
    };
    let preserves = |root,
                     whole: &[semantic_vocabulary::StructuralDomainId],
                     projected: &[terminal_psi::StructuralPathQualification]| {
        terminal_semantics::projected_field_declaration(
            structural_types.values().copied(),
            root,
            path,
            *field,
        )
        .is_some_and(|selected| {
            let mut written = path.clone();
            written.push(terminal_psi::StructuralPathSegment::Field(
                selected.identity.clone(),
            ));
            terminal_semantics::structural_memberships_disjoint_from_write(
                whole, projected, &written,
            )
        })
    };
    let (whole, projected) = match destination {
        abstract_operations::StructuralScalarStoreDestination::Parameter(parameter)
        | abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
            parameter,
            ..
        } => (
            parameter.qualifications.as_slice(),
            parameter.projected_qualifications.as_slice(),
        ),
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { result, .. } => (
            result.qualifications.as_slice(),
            result.projected_qualifications.as_slice(),
        ),
    };
    // Resolve the field through its typed crossing, but preserve memberships
    // in the original carrier's coordinates, including every Referent step.
    if !preserves(destination.structural_type(), whole, projected) {
        return Err(unsupported(
            "scalar field write overlaps a standing carrier membership",
        ));
    }
    let (destination, path, destination_storage) = match destination {
        abstract_operations::StructuralScalarStoreDestination::Parameter(destination) => {
            if function.arrival_parameter(destination.place) != Some(destination)
                || !matches!(
                    destination.access,
                    StructuralAccess::Owned
                        | StructuralAccess::MutableBorrow
                        | StructuralAccess::WriteOnlyBorrow
                )
            {
                return Err(unsupported(
                    "scalar field parameter access or unaffected membership does not match its declaration",
                ));
            }
            if function.structural_parameters.contains(destination) {
                let parameter = parameters_by_place
                    .get(&destination.place)
                    .copied()
                    .filter(|parameter| {
                        parameter.structural_type == destination.structural_type
                            && parameter.multiplicity == destination.multiplicity
                            && parameter.access == destination.access
                            && parameter.projected_qualifications
                                == destination.projected_qualifications
                    })
                    .ok_or(unsupported(
                        "scalar field parameter has no matching native placement",
                    ))?;
                (
                    abstract_operations::StructuralScalarStoreDestination::Parameter(
                        destination.clone(),
                    ),
                    path.clone(),
                    target_operations::TargetStructuralArgumentSource::Placement(
                        parameter.placement.clone(),
                    ),
                )
            } else {
                let block = function
                    .block_entries
                    .iter()
                    .find(|block| {
                        block.structural_parameters.contains(destination)
                            && destination.access == StructuralAccess::MutableBorrow
                    })
                    .ok_or(unsupported(
                        "scalar field parameter has no matching block declaration",
                    ))?;
                (
                    abstract_operations::StructuralScalarStoreDestination::Parameter(
                        destination.clone(),
                    ),
                    path.clone(),
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: block.block,
                        place: destination.place,
                    },
                )
            }
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } => {
            if let Some(resolved) = reference_destination.as_ref() {
                if resolved.carrier_type != result.structural_type
                    || !structural_homes
                        .get(&result.place)
                        .is_some_and(|home| home.operation_result() == Some((*producer, result)))
                {
                    return Err(unsupported(
                        "returned reference has no matching declared carrier home",
                    ));
                }
                (
                    destination.clone(),
                    path.clone(),
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: *producer,
                    },
                )
            } else {
                let home = structural_homes
                    .get(&result.place)
                    .filter(|home| home.operation_result() == Some((*producer, result)))
                    .ok_or(unsupported(
                        "scalar field result has no matching storage home",
                    ))?;
                (
                    destination.clone(),
                    path.clone(),
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation: *producer,
                    },
                )
            }
        }
        // An owned arrival writes its own block home, the one every incoming
        // edge moved its whole source into; it never writes a predecessor's
        // producer home and never becomes a loan of one.
        abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
            block,
            parameter,
        } => {
            super::owned_store_home::block_home(structural_homes, *block, parameter)
                .ok_or_else(|| unsupported("owned scalar arrival has no matching storage home"))?;
            (
                destination.clone(),
                path.clone(),
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block: *block,
                    place: parameter.place,
                },
            )
        }
    };
    // Geometry after a reference crossing is relative to the loaded pointer,
    // not the carrier's eight-byte home. Retain the original destination and
    // crossing on the target row so receiving replay must check that load.
    let (projection_root, projection_path) = reference_destination
        .as_ref()
        .map(|reference| (reference.referent, reference.path.as_slice()))
        .unwrap_or((destination.structural_type(), path.as_slice()));
    if !terminal_psi::is_structural_scalar_store_path(projection_path) {
        return Err(unsupported(
            "scalar field destination is not an admitted scalar projection",
        ));
    }
    let destination_shape = super::super::structural_layout::structural_shape(
        projection_root,
        structural_types,
        shape_cache,
        active,
    )?;
    // The retained destination and its exact storage origin select the
    // write. Additional borrowed inputs do not change that storage authority.
    // A field consumes the same dominating scalar source as a call or edge:
    // Boolean and IEEE results can have homes or block parameters just as
    // integers do. Do not rediscover a narrower producer family by type.
    let source = crate::lowering::control_flow::scalar_sources::resolved_source(
        value.value,
        function,
        scalar_sources,
    )?;
    scalar_shape(value.value, value.scalar_type, false)?;
    let exact_source = match source {
        TargetUnitScalarArgumentSource::BlockParameter(parameter) => {
            function.block_entries.iter().any(|entry| {
                entry.block == parameter.block
                    && entry.parameters.iter().any(|declared| {
                        declared.value == parameter.value
                            && declared.scalar_type == parameter.scalar_type
                    })
            })
        }
        TargetUnitScalarArgumentSource::Parameter {
            parameter_index, ..
        } => usize::try_from(parameter_index)
            .ok()
            .and_then(|position| function.parameters.get(position))
            .is_some_and(|parameter| {
                parameter.value == value.value && parameter.scalar_type == value.scalar_type
            }),
        TargetUnitScalarArgumentSource::Home(home) => {
            Some(home.shape) == fixed_native_scalar_shape(value.scalar_type)
        }
        TargetUnitScalarArgumentSource::IntegerImmediate { .. }
        | TargetUnitScalarArgumentSource::BooleanImmediate { .. }
        | TargetUnitScalarArgumentSource::IeeeFloatImmediate { .. } => true,
    };
    if !exact_source
        || source.source_value() != value.value
        || source.scalar_type() != value.scalar_type
    {
        return Err(unsupported(
            "scalar field source does not match its exact dominating value",
        ));
    }
    // The carrier composes fields and literal or runtime elements in any
    // order: static steps fold into the offset, and each runtime element is
    // an `(index, stride)` run over its selector's dominating source. The
    // element's bound stays the store's verified obligation.
    let (carrier_path, namespace) = match projection_path.split_last() {
        Some((terminal_psi::StructuralPathSegment::Case(_), prefix)) => {
            (prefix, &projection_path[projection_path.len() - 1..])
        }
        _ => (projection_path, &[][..]),
    };
    let (carrier_type, _, carrier_byte_offset, runs) = runtime_projection(
        projection_root,
        carrier_path,
        structural_types,
        shape_cache,
        active,
    )?;
    let indices = crate::lowering::control_flow::scalar_sources::runtime_indices(
        runs,
        function,
        scalar_sources,
    )?;
    let (scalar_byte_offset, field_byte_size) = if namespace.is_empty() {
        direct_scalar_field_geometry(carrier_type, *field, value.scalar_type, structural_types)?
    } else {
        static_field_geometry(
            carrier_type,
            namespace,
            *field,
            value.scalar_type,
            structural_types,
        )?
    };
    // The descriptor-size ceiling bounds a store inside its destination's
    // own storage. A view destination only stores through the loan: its
    // referent's extent is the runtime bound, and the descriptor's 16-byte
    // base+extent carrier is not the write's frame.
    let destination_is_view = matches!(
        structural_types
            .get(&destination.structural_type())
            .map(|declaration| &declaration.shape),
        Some(
            terminal_psi::StructuralTypeShape::ElementView { .. }
                | terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
        )
    );
    let field_byte_offset = carrier_byte_offset
        .checked_add(scalar_byte_offset)
        .filter(|offset| {
            offset
                .checked_add(field_byte_size)
                .is_some_and(|end| destination_is_view || end <= destination_shape.byte_size)
        })
        .ok_or(LoweringError::StructuralTypeTooLarge(
            destination.structural_type(),
        ))?;
    operations.push(TargetUnitOperation::StructuralScalarFieldStore {
        psi_operation: *psi_operation,
        destination,
        path,
        field: *field,
        destination_storage,
        field_byte_offset,
        source,
        indices,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
