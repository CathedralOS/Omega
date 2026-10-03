//! Exact mutable byte descriptors and once-evaluated scalar write operands.

use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

/// Keep replacement as one ordered operation. Capacity is a destination bound,
/// not permission to read that many bytes from a shorter immutable source.
pub(super) fn replace(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let (psi_operation, destination, source, length, obligation, field_destination) =
        match operation {
            AbstractOperation::ByteSequenceStore {
                psi_operation,
                destination,
                source,
                length,
                obligation,
            } => {
                let parameter = function
                    .arrival_parameter(*destination)
                    .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
                if !matches!(
                    parameter.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.projected_qualifications.is_empty()
                    || !structural_types
                        .get(&parameter.structural_type)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                                )
                            )
                        })
                    || function
                        .entry_claims
                        .iter()
                        .any(|claim| claim.input == *destination)
                {
                    return Err(LoweringError::unsupported_control_flow(function.machine));
                }
                (psi_operation, destination, source, length, obligation, None)
            }
            AbstractOperation::StructuralByteSequenceFieldStore {
                psi_operation,
                destination,
                path,
                field,
                source,
                length,
                obligation,
            } => (
                psi_operation,
                destination,
                source,
                length,
                obligation,
                Some((
                    writable_field(
                        function,
                        structural_types,
                        prepared,
                        live,
                        *destination,
                        path,
                        *field,
                    )?,
                    *field,
                )),
            ),
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
    if source == destination
        || live.lengths.get(length) != Some(source)
        || !(prepared
            .parameters
            .iter()
            .any(|parameter| parameter.place == *source)
            || live.views.contains_key(source)
            || live.block_views.contains(source))
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let length_value = *live
        .integers
        .get(length)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if length_value.scalar_type()
        != IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    operations.push(if let Some((destination, field)) = field_destination {
        TargetUnitOperation::StructuralByteSequenceFieldStore {
            psi_operation: *psi_operation,
            destination,
            field,
            source: *source,
            length: length_value.into_target_source(*length),
            obligation: *obligation,
        }
    } else {
        TargetUnitOperation::ByteSequenceStore {
            psi_operation: *psi_operation,
            destination: *destination,
            source: *source,
            length: length_value.into_target_source(*length),
            obligation: *obligation,
        }
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

fn writable_field(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    destination: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Result<terminal_psi::StructuralArgument, LoweringError> {
    writable_byte_field(
        function,
        structural_types,
        prepared,
        live,
        destination,
        path,
        field,
        |carrier| {
            matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        },
    )
}

/// Replace a shared view field from a readable descriptor value. Construction
/// and replacement share storage availability: a copied descriptor is an
/// ordinary value home, not a new backing allocation. Independent abstract
/// replay owns the source loan's lifetime and the destination's predicates.
pub(super) fn replace_view_descriptor(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let unsupported = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::StructuralByteViewFieldStore {
        psi_operation,
        destination,
        path,
        field,
        source,
        ..
    } = operation
    else {
        return Err(unsupported());
    };
    let destination_argument = writable_byte_field(
        function,
        structural_types,
        prepared,
        live,
        *destination,
        path,
        *field,
        |carrier| {
            carrier
                == terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                }
        },
    )?;
    // The source is an already available two-word descriptor. Its backing
    // lifetime is independently replayed on the current optimization graph;
    // native lowering must not infer that lifetime from a producer family.
    let source_type = live.structural_observation_type(function, *source);
    if source == destination
        || !source_type.is_some_and(|identity| {
            structural_types
                .get(&identity)
                .is_some_and(|declaration| declaration.shape.is_borrowed_byte_view())
        })
    {
        return Err(unsupported());
    }
    let indices = super::primitive_storage::field_indices(
        function,
        structural_types,
        prepared,
        live,
        &destination_argument,
    )?;
    operations.push(TargetUnitOperation::StructuralByteViewFieldStore {
        psi_operation: *psi_operation,
        destination: destination_argument,
        field: *field,
        source: *source,
        indices,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

fn writable_byte_field(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    destination: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
    carrier_admitted: impl Fn(terminal_psi::ByteSequenceCarrier) -> bool,
) -> Result<terminal_psi::StructuralArgument, LoweringError> {
    let reference_destination = super::references::writable_referent_destination(
        function,
        live,
        structural_types,
        destination,
        path,
    );
    if reference_destination.is_none()
        && super::references::check_projected_access(
            function,
            live,
            structural_types,
            destination,
            path,
            Some(field),
        )
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (root_type, access) = if let Some(reference) = &reference_destination {
        (
            reference.result.structural_type,
            StructuralAccess::MutableBorrow,
        )
    } else {
        super::primitive_storage::field_root(
            function,
            structural_types,
            prepared,
            live,
            destination,
        )?
    };
    if !matches!(
        access,
        StructuralAccess::Owned
            | StructuralAccess::MutableBorrow
            | StructuralAccess::WriteOnlyBorrow
    ) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let root = destination;
    let (projection_root, projection_path) = reference_destination
        .as_ref()
        .map(|reference| (reference.referent, reference.path.as_slice()))
        .unwrap_or((root_type, path));
    if !terminal_psi::is_structural_scalar_store_path(projection_path)
        || function
            .entry_claims
            .iter()
            .any(|claim| claim.input == root)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let carrier = if projection_path.is_empty() {
        projection_root
    } else {
        crate::lowering::structural_layout::runtime_projection(
            projection_root,
            projection_path,
            structural_types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?
        .0
    };
    let StructuralTypeShape::Record { fields } = &structural_types
        .get(&carrier)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
        .shape
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let mut matching = fields.iter().filter(|candidate| candidate.id == field);
    let selected = matching
        .next()
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if matching.next().is_some()
        || selected.relevance.is_erased()
        || !selected
            .field_type
            .byte_sequence_carrier()
            .is_some_and(carrier_admitted)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    Ok(terminal_psi::StructuralArgument {
        place: root,
        access,
        path: path.to_vec(),
    })
}

pub(super) fn replace_byte(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::StructuralByteSequenceFieldByteStore {
        psi_operation,
        destination,
        path,
        field,
        index,
        value,
        length,
        obligation,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let destination_argument = writable_field(
        function,
        structural_types,
        prepared,
        live,
        *destination,
        path,
        *field,
    )?;
    // Current-IR validation proves all-path freshness. Retain that same exact
    // field observation and dominating scalar definitions in target custody.
    if !function.operations.iter().any(|operation| matches!(operation,
        AbstractOperation::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field, result, .. }
        if source == destination && terminal_semantics::structural_paths_select_same_location(measured_path, path) && measured_field == field && result.value == *length))
    { return Err(LoweringError::unsupported_control_flow(function.machine)); }
    let index_value = *live
        .integers
        .get(index)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    let byte_value = *live
        .integers
        .get(value)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    let length_value = *live
        .integers
        .get(length)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    for (scalar_type, bits) in [
        (index_value.scalar_type(), 64),
        (byte_value.scalar_type(), 8),
        (length_value.scalar_type(), 64),
    ] {
        if scalar_type.sign() != IntegerSign::Unsigned
            || scalar_type.bits() != bits
            || scalar_type.is_address()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    operations.push(TargetUnitOperation::StructuralByteSequenceFieldByteStore {
        psi_operation: *psi_operation,
        destination: destination_argument,
        field: *field,
        index: index_value.into_target_source(*index),
        value: byte_value.into_target_source(*value),
        length: *length,
        obligation: *obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::ByteSequenceWrite {
        psi_operation,
        destination,
        index,
        value,
        length,
        obligation,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    if live.lengths.get(length) != Some(destination)
        || !(prepared
            .parameters
            .iter()
            .any(|parameter| parameter.place == *destination)
            || live.block_views.contains(destination)
            || live.views.contains_key(destination)
            || live.structural_homes.contains_key(destination))
        || super::references::check_root_access(live, *destination)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let values = super::observations::scalar_values(live, &prepared.scalar_parameters)?;
    let (view, access) = crate::lowering::scalar::byte_views::resolve_byte_view(
        function,
        structural_types,
        &prepared.parameters,
        crate::lowering::scalar::element_views::ElementViewBackings::new(
            &live.structural_homes,
            &live.address_joins,
        ),
        &values,
        &live.lengths,
        *destination,
        &mut Vec::new(),
    )?;
    if !matches!(
        access,
        StructuralAccess::MutableBorrow | StructuralAccess::Owned
    ) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let index_value = *live
        .integers
        .get(index)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    let byte_value = *live
        .integers
        .get(value)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    for (scalar_type, bits) in [
        (index_value.scalar_type(), 64),
        (byte_value.scalar_type(), 8),
    ] {
        if scalar_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
            || scalar_type.bits() != bits
            || scalar_type.is_address()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    operations.push(TargetUnitOperation::ByteSequenceWrite {
        psi_operation: *psi_operation,
        destination: *destination,
        view,
        index: index_value.into_target_source(*index),
        value: byte_value.into_target_source(*value),
        length: *length,
        obligation: *obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
