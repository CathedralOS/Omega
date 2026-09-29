//! Exact projected scalar store lowering for an attached Unit body.

use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use calling_conventions::ValueShape;
use semantic_vocabulary::{PlaceId, ScalarType, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetStructuralParameter, TargetUnitOperation, TargetUnitScalarArgumentSource,
    TerminalPsiProvenance,
};
use terminal_psi::StructuralAccess;

use super::super::scalar::scalar_shape;
use super::super::scalar_abi::fixed_native_scalar_shape;
use super::super::structural_layout::{
    direct_boolean_field_offset, direct_integer_field_offset, direct_scalar_field_offset,
    runtime_projection,
};

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_field_store(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters_by_place: &BTreeMap<PlaceId, &TargetStructuralParameter>,
    structural_homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
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
    if !terminal_psi::is_structural_scalar_store_path(path) {
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let destination_storage = match destination {
        abstract_operations::StructuralScalarStoreDestination::Parameter(destination) => {
            if !matches!(
                destination.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ) {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
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
                    .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ))?;
                target_operations::TargetStructuralArgumentSource::Placement(
                    parameter.placement.clone(),
                )
            } else {
                let block = function
                    .block_entries
                    .iter()
                    .find(|block| {
                        block.structural_parameters.contains(destination)
                            && destination.access == StructuralAccess::MutableBorrow
                            && abstract_operations::control_flow::address_joins::is_address_join(
                                destination,
                                |identity| structural_types.get(&identity).copied(),
                            )
                    })
                    .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
                        function.machine,
                    ))?;
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block: block.block,
                    place: destination.place,
                }
            }
        }
        abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } => {
            let home = structural_homes
                .get(&result.place)
                .filter(|home| home.operation_result() == Some((*producer, result)))
                .ok_or(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ))?;
            if home.has_claims()
                || !home.qualifications().is_empty()
                || !home.projected_qualifications().is_empty()
                || !matches!(
                    home.multiplicity(),
                    terminal_psi::StructuralMultiplicity::Affine
                        | terminal_psi::StructuralMultiplicity::Unrestricted
                )
            {
                return Err(LoweringError::UnsupportedOperationInUnitFunction(
                    function.machine,
                ));
            }
            target_operations::TargetStructuralArgumentSource::StructuralHome {
                psi_operation: *producer,
            }
        }
    };
    let destination_shape = super::super::structural_layout::structural_shape(
        destination.structural_type(),
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
    let source_shape = scalar_shape(value.value, value.scalar_type, false)?;
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
        return Err(LoweringError::UnsupportedOperationInUnitFunction(
            function.machine,
        ));
    }
    let field_byte_size = source_shape.byte_size;
    // The carrier composes fields and literal or runtime elements in any
    // order: static steps fold into the offset, and each runtime element is
    // an `(index, stride)` run over its selector's dominating source. The
    // element's bound stays the store's verified obligation.
    let (carrier_type, _, carrier_byte_offset, runs) = runtime_projection(
        destination.structural_type(),
        path,
        structural_types,
        shape_cache,
        active,
    )?;
    let indices = crate::lowering::control_flow::scalar_sources::runtime_indices(
        runs,
        function,
        scalar_sources,
    )?;
    let scalar_byte_offset = match value.scalar_type {
        ScalarType::Boolean => direct_boolean_field_offset(carrier_type, *field, structural_types)?,
        ScalarType::Integer(integer_type) => {
            direct_integer_field_offset(carrier_type, *field, integer_type, structural_types)?
        }
        ScalarType::IeeeFloat(_) => {
            direct_scalar_field_offset(carrier_type, *field, value.scalar_type, structural_types)?
        }
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
        destination: destination.clone(),
        path: path.clone(),
        field: *field,
        destination_storage,
        field_byte_offset,
        source,
        indices,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
