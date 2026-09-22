//! Exact mutable byte descriptors and once-evaluated scalar write operands.

use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{
    StructuralAccess, StructuralFieldType, StructuralMultiplicity, StructuralTypeShape,
};

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
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::StructuralByteSequenceFieldStore {
        psi_operation,
        destination,
        path,
        field,
        source,
        length,
        obligation,
    } = operation
    else {
        return Err(invalid());
    };
    let destination_argument =
        writable_field(function, structural_types, *destination, path, *field)?;
    if source == destination
        || live.lengths.get(length) != Some(source)
        || !(prepared
            .parameters
            .iter()
            .any(|parameter| parameter.place == *source)
            || live.views.contains_key(source)
            || live.block_views.contains(source))
    {
        return Err(invalid());
    }
    let length_value = *live.integers.get(length).ok_or_else(invalid)?;
    if length_value.scalar_type()
        != IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::StructuralByteSequenceFieldStore {
        psi_operation: *psi_operation,
        destination: destination_argument,
        field: *field,
        source: *source,
        length: length_value.into_target_source(*length),
        obligation: *obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}

fn writable_field(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    destination: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Result<terminal_psi::StructuralArgument, LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let parameter = function
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == destination)
        .ok_or_else(invalid)?;
    if !matches!(
        parameter.access,
        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
    ) || parameter.multiplicity == StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || !terminal_psi::is_bounded_structural_scalar_store_path(path)
        || function
            .entry_claims
            .iter()
            .any(|claim| claim.input == destination)
    {
        return Err(invalid());
    }
    let carrier = if path.is_empty() {
        parameter.structural_type
    } else {
        crate::lowering::structural_layout::resolve_structural_projection_path(
            parameter.structural_type,
            path,
            structural_types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?
        .0
    };
    let StructuralTypeShape::Record { fields } =
        &structural_types.get(&carrier).ok_or_else(invalid)?.shape
    else {
        return Err(invalid());
    };
    let mut matching = fields.iter().filter(|candidate| candidate.id == field);
    let selected = matching.next().ok_or_else(invalid)?;
    if matching.next().is_some()
        || selected.relevance.is_erased()
        || !matches!(
            selected.field_type,
            StructuralFieldType::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        )
    {
        return Err(invalid());
    }
    Ok(terminal_psi::StructuralArgument {
        place: destination,
        access: parameter.access,
        path: path.to_vec(),
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
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
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
        return Err(invalid());
    };
    let destination_argument =
        writable_field(function, structural_types, *destination, path, *field)?;
    // Current-IR validation proves all-path freshness. Retain that same exact
    // field observation and dominating scalar definitions in target custody.
    if !function.operations.iter().any(|operation| matches!(operation,
        AbstractOperation::StructuralByteSequenceFieldLength { source, path: measured_path, field: measured_field, result, .. }
        if source == destination && measured_path == path && measured_field == field && result.value == *length))
    { return Err(invalid()); }
    let index_value = *live.integers.get(index).ok_or_else(invalid)?;
    let byte_value = *live.integers.get(value).ok_or_else(invalid)?;
    let length_value = *live.integers.get(length).ok_or_else(invalid)?;
    for (scalar_type, bits) in [
        (index_value.scalar_type(), 64),
        (byte_value.scalar_type(), 8),
        (length_value.scalar_type(), 64),
    ] {
        if scalar_type.sign() != IntegerSign::Unsigned
            || scalar_type.bits() != bits
            || scalar_type.is_address()
        {
            return Err(invalid());
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
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::ByteSequenceWrite {
        psi_operation,
        destination,
        index,
        value,
        length,
        obligation,
    } = operation
    else {
        return Err(invalid());
    };
    if live.lengths.get(length) != Some(destination)
        || !(prepared
            .parameters
            .iter()
            .any(|parameter| parameter.place == *destination)
            || live.block_views.contains(destination))
    {
        return Err(invalid());
    }
    let (destination, view) = crate::lowering::scalar::byte_views::mutable_parameter_view(
        function,
        structural_types,
        &prepared.parameters,
        *destination,
    )?;
    let index_value = *live.integers.get(index).ok_or_else(invalid)?;
    let byte_value = *live.integers.get(value).ok_or_else(invalid)?;
    for (scalar_type, bits) in [
        (index_value.scalar_type(), 64),
        (byte_value.scalar_type(), 8),
    ] {
        if scalar_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
            || scalar_type.bits() != bits
            || scalar_type.is_address()
        {
            return Err(invalid());
        }
    }
    operations.push(TargetUnitOperation::ByteSequenceWrite {
        psi_operation: *psi_operation,
        destination,
        view,
        index: index_value.into_target_source(*index),
        value: byte_value.into_target_source(*value),
        length: *length,
        obligation: *obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
