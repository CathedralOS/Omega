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
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == *destination)
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
                        live,
                        *destination,
                        path,
                        *field,
                        live.structural_homes.get(destination),
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
    live: &LiveDefinitions,
    destination: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
    owned_home: Option<&target_operations::TargetStructuralHomeRequirement>,
) -> Result<terminal_psi::StructuralArgument, LoweringError> {
    writable_byte_field(
        function,
        structural_types,
        live,
        destination,
        path,
        field,
        owned_home,
        |carrier| {
            matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        },
    )
}

/// Replace a shared view field's descriptor with an established literal
/// view's. The verifier admitted only literal sources; this lowering keeps
/// that join rather than relocating an arbitrary live view into the field.
pub(super) fn replace_view_descriptor(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
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
    } = operation
    else {
        return Err(unsupported());
    };
    let destination_argument = writable_byte_field(
        function,
        structural_types,
        live,
        *destination,
        path,
        *field,
        None,
        |carrier| {
            carrier
                == terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(StructuralAccess::SharedBorrow),
                }
        },
    )?;
    let literal = live.views.get(source).is_some_and(|(producer, _)| {
        function.operations.iter().any(|operation| {
            matches!(operation, AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation, place, ..
            } if psi_operation == producer && place.id == *source)
        })
    });
    if source == destination || !literal {
        return Err(unsupported());
    }
    operations.push(TargetUnitOperation::StructuralByteViewFieldStore {
        psi_operation: *psi_operation,
        destination: destination_argument,
        field: *field,
        source: *source,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

fn writable_byte_field(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    live: &LiveDefinitions,
    destination: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
    owned_home: Option<&target_operations::TargetStructuralHomeRequirement>,
    carrier_admitted: impl Fn(terminal_psi::ByteSequenceCarrier) -> bool,
) -> Result<terminal_psi::StructuralArgument, LoweringError> {
    let (root, root_type, access, path) = if let Some(home) = owned_home.filter(|home| {
        structural_types
            .get(&home.structural_type())
            .is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::Record { .. })
            })
    }) {
        let (_, result) = home
            .operation_result()
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if result.place != destination
            || result.multiplicity != StructuralMultiplicity::Unrestricted
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.qualification_establishments.is_empty()
            || !result.claims.is_empty()
            || !structural_types
                .get(&result.structural_type)
                .is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::Record { .. })
                })
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        // The live home is the actual dominating result, not a borrowed
        // parameter invented from its type. Selection addresses this home.
        (
            destination,
            result.structural_type,
            StructuralAccess::Owned,
            path.to_vec(),
        )
    } else {
        let (parameter, path) = if let Some(parameter) = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == destination)
        {
            (parameter.clone(), path.to_vec())
        } else {
            let resolved = super::references::writable_referent_destination(
                function,
                live,
                structural_types,
                destination,
                path,
            )
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            (resolved.parameter, resolved.path)
        };
        if !matches!(
            parameter.access,
            StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
        ) || parameter.multiplicity == StructuralMultiplicity::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        (
            parameter.place,
            parameter.structural_type,
            parameter.access,
            path,
        )
    };
    if !terminal_psi::is_bounded_structural_scalar_store_path(&path)
        || function
            .entry_claims
            .iter()
            .any(|claim| claim.input == root)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let carrier = if path.is_empty() {
        root_type
    } else {
        crate::lowering::structural_layout::resolve_structural_projection_path(
            root_type,
            &path,
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
        path,
    })
}

pub(super) fn replace_byte(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
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
        live,
        *destination,
        path,
        *field,
        None,
    )?;
    // Current-IR validation proves all-path freshness. Retain that same exact
    // field observation and dominating scalar definitions in target custody.
    if !function.operations.iter().any(|operation| matches!(operation,
        AbstractOperation::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field, result, .. }
        if source == destination && measured_path == path && measured_field == field && result.value == *length))
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
            || live.structural_homes.contains_key(destination))
        || super::references::check_root_access(live, *destination)
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let view = if live.structural_homes.contains_key(destination) {
        let (producer, structural_type) = live
            .views
            .get(destination)
            .copied()
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
        if !function.operations.iter().any(|operation| {
            matches!(operation,
            AbstractOperation::EstablishByteSequenceConcatenation { psi_operation, result, .. }
                if *psi_operation == producer && result.place == *destination
                    && result.structural_type == structural_type)
        }) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        target_operations::TargetByteView::Owned {
            psi_operation: producer,
            place: *destination,
            structural_type,
        }
    } else {
        crate::lowering::scalar::byte_views::mutable_parameter_view(
            function,
            structural_types,
            &prepared.parameters,
            *destination,
        )?
        .1
    };
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
