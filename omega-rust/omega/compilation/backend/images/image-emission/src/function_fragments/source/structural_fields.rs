//! Publication joins field observations to their exact ordinary graph operations.
//! Mandatory object/source replay derives layout and validates the selected load,
//! including the original referent or owned input's current local home. No second
//! field-offset calculator or legacy expression-tree realization belongs here.

use abstract_operations::{AbstractFunction, AbstractOperation, AbstractStructuralCallResult};
use semantic_vocabulary::ScalarType;
use target_operations::{TargetFunction, TargetUnitOperation};
use terminal_psi::StructuralArgument;

#[cfg(test)]
mod tests;

/// Join the exact indexed-write occurrence without replacing its structural
/// subject with a byte offset. Mandatory replay checks layout and scalar homes.
pub(super) fn indexed_store_retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
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
        return false;
    };
    let Some((access, _)) = read_access(function, target, *destination, AccessPurpose::ByteLength)
    else {
        return false;
    };
    if !matches!(
        access,
        terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    ) {
        return false;
    }
    let expected_destination = StructuralArgument {
        place: *destination,
        access,
        path: path.clone(),
    };
    let mut writes = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|candidate| {
            matches!(candidate,
        TargetUnitOperation::StructuralByteSequenceFieldByteStore { psi_operation: retained, .. }
            if retained == psi_operation)
        });
    let Some(TargetUnitOperation::StructuralByteSequenceFieldByteStore {
        destination: retained_destination,
        field: retained_field,
        index: retained_index,
        value: retained_value,
        length: retained_length,
        obligation: retained_obligation,
        ..
    }) = writes.next()
    else {
        return false;
    };
    *retained_destination == expected_destination
        && retained_field == field
        && retained_length == length
        && retained_obligation == obligation
        && retained_index.source_value() == *index
        && retained_value.source_value() == *value
        && matches!(retained_index.scalar_type(), ScalarType::Integer(integer)
            if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64))
        && matches!(retained_value.scalar_type(), ScalarType::Integer(integer)
            if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8))
        && writes.next().is_none()
}

/// The footprint rows a primitive read or store, or a scalar field store,
/// publishes. A static projection keeps its one exact place row, which the
/// caller checks. A projection through runtime-selected elements publishes
/// exactly one row per element, in path order, on the accessed root: each
/// names that element's selector and obligation (and, for a write, the
/// stored value). Mandatory source/selection replay independently
/// reconstructs the scaled address, the strides, and the accepted
/// certificates rather than trusting this roster.
pub(super) fn runtime_footprint_retained(
    operation: &AbstractOperation,
    accesses: &[selected_instructions::SelectedMemoryAccess],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let (psi_operation, place, written) = match operation {
        AbstractOperation::PrimitiveScalarRead {
            psi_operation,
            source,
            ..
        } => (*psi_operation, *source, None),
        AbstractOperation::WriteOnlyPrimitiveStore {
            psi_operation,
            destination,
            value,
            ..
        } => (*psi_operation, destination.place(), Some(value.value)),
        AbstractOperation::StructuralScalarFieldStore {
            psi_operation,
            destination,
            value,
            ..
        } => (*psi_operation, destination.place(), Some(value.value)),
        _ => return false,
    };
    let elements = operation.runtime_indices();
    // The indexed-access rows this operation publishes: one per runtime
    // element. An auxiliary row the addressing realizes beside them — a view
    // carrier's backing-pointer read of its own descriptor — shares the
    // operation's origin but is not the element access.
    let rows = accesses
        .iter()
        .filter(|access| {
            access.origin == Origin::Operation(psi_operation)
                && matches!(
                    access.role,
                    Role::ReadIndexedPrimitive { .. }
                        | Role::WriteIndexedPrimitive { .. }
                        | Role::WriteIndexedStructural { .. }
                )
        })
        .collect::<Vec<_>>();
    if elements.is_empty() {
        return true;
    }
    rows.len() == elements.len()
        && rows
            .iter()
            .zip(&elements)
            .all(|(row, (index, obligation))| {
                row.place == place
                    && match (row.role, written) {
                        (
                            Role::ReadIndexedPrimitive {
                                index: retained_index,
                                obligation: retained_obligation,
                                ..
                            },
                            None,
                        ) => retained_index == *index && retained_obligation == *obligation,
                        (
                            Role::WriteIndexedPrimitive {
                                index: retained_index,
                                value: retained_value,
                                obligation: retained_obligation,
                                ..
                            },
                            Some(value),
                        ) => {
                            retained_index == *index
                                && retained_value == value
                                && retained_obligation == *obligation
                        }
                        _ => false,
                    }
            })
}

/// Exactly one one-byte write accounts for this effect. A read or metadata
/// publication with the same origin is not part of indexed replacement.
/// Mandatory source/selection replay independently reconstructs the additive
/// payload offset and the accepted fact, rather than trusting this roster.
pub(super) fn indexed_store_footprint_retained(
    operation: &AbstractOperation,
    accesses: &[selected_instructions::SelectedMemoryAccess],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let AbstractOperation::StructuralByteSequenceFieldByteStore {
        psi_operation,
        destination,
        index,
        value,
        length,
        obligation,
        ..
    } = operation
    else {
        return false;
    };
    let mut effects = accesses
        .iter()
        .filter(|access| access.origin == Origin::Operation(*psi_operation));
    let Some(write) = effects.next() else {
        return false;
    };
    write.place == *destination
        && write.byte_count == 1
        && matches!(write.role, Role::WriteByteSequence {
            index: retained_index,
            value: retained_value,
            length: retained_length,
            obligation: retained_obligation,
            ..
        } if retained_index == *index && retained_value == *value
            && retained_length == *length && retained_obligation == *obligation)
        && effects.next().is_none()
}

/// Join the exact indexed-read occurrence without replacing its structural
/// subject with a byte offset. Mandatory replay checks layout, the payload
/// address and scalar homes; a read needs a shared or mutable borrow.
pub(super) fn indexed_read_retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
    let AbstractOperation::StructuralByteSequenceFieldRead {
        psi_operation,
        result,
        source,
        path,
        field,
        index,
        length,
        obligation,
    } = operation
    else {
        return false;
    };
    let Some((access, _)) = read_access(function, target, *source, AccessPurpose::Field) else {
        return false;
    };
    if !matches!(
        access,
        terminal_psi::StructuralAccess::Owned
            | terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
    ) {
        return false;
    }
    let expected_source = StructuralArgument {
        place: *source,
        access,
        path: path.clone(),
    };
    let mut reads = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|candidate| {
            matches!(candidate,
        TargetUnitOperation::StructuralByteSequenceFieldRead { psi_operation: retained, .. }
            if retained == psi_operation)
        });
    let Some(TargetUnitOperation::StructuralByteSequenceFieldRead {
        result: retained_result,
        source: retained_source,
        field: retained_field,
        index: retained_index,
        length: retained_length,
        obligation: retained_obligation,
        ..
    }) = reads.next()
    else {
        return false;
    };
    retained_result == result
        && *retained_source == expected_source
        && retained_field == field
        && retained_length == length
        && retained_obligation == obligation
        && retained_index.source_value() == *index
        && matches!(retained_index.scalar_type(), ScalarType::Integer(integer)
            if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64))
        && reads.next().is_none()
}

/// Whether an indexed field read follows a shared view field's descriptor
/// pointer rather than addressing inline bounded-owned bytes. The carrier is
/// resolved from the exact root declaration, never from a footprint shape.
pub(super) fn reads_through_descriptor(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
    let AbstractOperation::StructuralByteSequenceFieldRead {
        source,
        path,
        field,
        ..
    } = operation
    else {
        return false;
    };
    read_access(function, target, *source, AccessPurpose::Field).is_some_and(|(_, root)| {
        field_carrier(target, root, path, *field)
            == Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            })
    })
}

/// The declared byte carrier of `field` below `root` along a static path.
fn field_carrier(
    target: &TargetFunction,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<terminal_psi::ByteSequenceCarrier> {
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &terminal_semantics::runtime_structural_path_tip(
            target.graph.structural_types.iter(),
            root,
            path,
        )?
        .shape
    else {
        return None;
    };
    fields
        .iter()
        .find(|candidate| candidate.id == field && !candidate.relevance.is_erased())?
        .field_type
        .byte_sequence_carrier()
}

/// The footprint rows an indexed field byte read publishes: its source root,
/// one byte, and its own selector, length and obligation. A read through a
/// shared view field's descriptor first loads that descriptor's pointer word
/// from the same root.
pub(super) fn indexed_read_footprint_retained(
    through_descriptor: bool,
    operation: &AbstractOperation,
    accesses: &[selected_instructions::SelectedMemoryAccess],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let AbstractOperation::StructuralByteSequenceFieldRead {
        psi_operation,
        source,
        index,
        length,
        obligation,
        ..
    } = operation
    else {
        return false;
    };
    let mut effects = accesses
        .iter()
        .filter(|access| access.origin == Origin::Operation(*psi_operation));
    if through_descriptor {
        let Some(pointer) = effects.next() else {
            return false;
        };
        if pointer.place != *source || pointer.byte_count != 8 || pointer.role != Role::ReadPlace {
            return false;
        }
    }
    let Some(read) = effects.next() else {
        return false;
    };
    read.place == *source
        && read.byte_count == 1
        && matches!(read.role, Role::ReadByteSequence {
            index: retained_index,
            length: retained_length,
            obligation: retained_obligation,
            ..
        } if retained_index == *index && retained_length == *length
            && retained_obligation == *obligation)
        && effects.next().is_none()
}

/// Account for one descriptor replacement's exact graph occurrence: the
/// exclusive destination, field and literal source the target retained.
pub(super) fn view_replacement_retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
    let AbstractOperation::StructuralByteViewFieldStore {
        psi_operation,
        destination,
        path,
        field,
        source,
    } = operation
    else {
        return false;
    };
    let Some(expected_destination) =
        write_destination(function, target, *destination, path, AccessPurpose::Field)
    else {
        return false;
    };
    if source == destination {
        return false;
    }
    let mut replacements = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation {
            TargetUnitOperation::StructuralByteViewFieldStore {
                psi_operation: identity,
                destination,
                field,
                source,
            } if identity == psi_operation => Some((destination, field, source)),
            _ => None,
        });
    matches!(replacements.next(), Some((actual_destination, actual_field, actual_source))
        if *actual_destination == expected_destination && actual_field == field
            && actual_source == source)
        && replacements.next().is_none()
}

/// The four footprint rows of a descriptor replacement: the literal
/// descriptor's pointer and extent words read in order, then the same two
/// words written at the field's descriptor in the destination root.
pub(super) fn view_replacement_footprints_retained(
    operation: &AbstractOperation,
    accesses: &[selected_instructions::SelectedMemoryAccess],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let AbstractOperation::StructuralByteViewFieldStore {
        psi_operation,
        source,
        ..
    } = operation
    else {
        return false;
    };
    let effects = accesses
        .iter()
        .filter(|access| access.origin == Origin::Operation(*psi_operation))
        .collect::<Vec<_>>();
    let [pointer, extent, pointer_write, extent_write] = effects.as_slice() else {
        return false;
    };
    pointer.place == *source
        && pointer.byte_offset == 0
        && pointer.byte_count == 8
        && pointer.role == Role::ReadPlace
        && extent.place == *source
        && extent.byte_offset == 8
        && extent.byte_count == 8
        && extent.role == Role::ReadPlace
        && pointer_write.place == extent_write.place
        && pointer_write.byte_count == 8
        && extent_write.byte_count == 8
        && pointer_write.role == Role::WritePlace
        && extent_write.role == Role::WritePlace
        && pointer_write.byte_offset.checked_add(8) == Some(extent_write.byte_offset)
        && pointer.instruction < extent.instruction
        && extent.instruction < pointer_write.instruction
        && pointer_write.instruction < extent_write.instruction
}

/// Account for replacement's exact graph occurrence. Mandatory source/selection
/// replay owns capacity evidence, original backing, layout and copy correctness.
pub(super) fn replacement_retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
    if let AbstractOperation::ByteSequenceStore {
        psi_operation,
        destination,
        source,
        length,
        obligation,
    } = operation
    {
        let Some((access, _)) = read_access(
            function,
            target,
            *destination,
            AccessPurpose::ByteOwnerReplacement,
        ) else {
            return false;
        };
        if destination == source
            || !matches!(
                access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
        {
            return false;
        }
        let mut matching = target.graph.blocks.iter().flat_map(|block| &block.operations).filter(|operation| matches!(operation,
            TargetUnitOperation::ByteSequenceStore { psi_operation: retained, .. } if retained == psi_operation));
        return matches!(matching.next(), Some(TargetUnitOperation::ByteSequenceStore { destination: actual_destination, source: actual_source, length: actual_length, obligation: actual_obligation, .. })
            if actual_destination == destination && actual_source == source && actual_length.source_value() == *length && actual_obligation == obligation)
            && matching.next().is_none();
    }
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
        return false;
    };
    let Some(expected_destination) = write_destination(
        function,
        target,
        *destination,
        path,
        AccessPurpose::ByteLength,
    ) else {
        return false;
    };
    if source == destination {
        return false;
    }
    let mut replacements = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation {
            TargetUnitOperation::StructuralByteSequenceFieldStore {
                psi_operation: identity,
                destination,
                field,
                source,
                length,
                obligation,
            } if identity == psi_operation => {
                Some((destination, field, source, length, obligation))
            }
            _ => None,
        });
    matches!(replacements.next(), Some((actual_destination, actual_field, actual_source, actual_length, actual_obligation))
        if *actual_destination == expected_destination && actual_field == field
            && actual_source == source && actual_obligation == obligation
            && actual_length.source_value() == *length
            && matches!(actual_length.scalar_type(), ScalarType::Integer(integer)
                if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)))
        && replacements.next().is_none()
}

/// The runtime copy has two exact span rows; metadata publication follows it.
/// These rows account for the effect, not a substitute for instruction replay.
pub(super) fn replacement_footprints_retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: Option<&TargetFunction>,
    accesses: &[selected_instructions::SelectedMemoryAccess],
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let (AbstractOperation::ByteSequenceStore {
        psi_operation,
        destination: _,
        source,
        length,
        obligation,
    }
    | AbstractOperation::StructuralByteSequenceFieldStore {
        psi_operation,
        destination: _,
        source,
        length,
        obligation,
        ..
    }) = operation
    else {
        return false;
    };
    let write_place = match operation {
        AbstractOperation::ByteSequenceStore { destination, .. } => *destination,
        AbstractOperation::StructuralByteSequenceFieldStore {
            destination, path, ..
        } => {
            let Some(target) = target else {
                return false;
            };
            match write_destination(
                function,
                target,
                *destination,
                path,
                AccessPurpose::ByteLength,
            ) {
                Some(destination) => destination.place,
                None => return false,
            }
        }
        _ => return false,
    };
    let mut reads = accesses.iter().filter(|access| {
        access.origin == Origin::Operation(*psi_operation)
            && matches!(access.role, Role::ReadByteSpan { .. })
    });
    let mut writes = accesses.iter().filter(|access| {
        access.origin == Origin::Operation(*psi_operation)
            && matches!(access.role, Role::WriteByteSpan { .. })
    });
    let mut metadata = accesses.iter().filter(|access| {
        access.origin == Origin::Operation(*psi_operation) && access.role == Role::WritePlace
    });
    let (Some(read), Some(write), Some(metadata_write)) =
        (reads.next(), writes.next(), metadata.next())
    else {
        return false;
    };
    if matches!(operation, AbstractOperation::StructuralByteSequenceFieldStore { path, .. }
        if matches!(path.first(), Some(terminal_psi::StructuralPathSegment::Referent)))
        && !pointer_read_before(accesses, *psi_operation, write_place, read.instruction)
    {
        return false;
    }
    let Role::ReadByteSpan {
        length: read_length,
        obligation: read_obligation,
        accepted_fact,
    } = read.role
    else {
        return false;
    };
    // Owned parameters, concatenations and ordinary bounded-owner call results use the
    // same `{live length, inline backing}` layout as bounded fields. Borrowed
    // descriptor sources account for their external backing from offset zero.
    let source_byte_offset = if function.operations.iter().any(|operation| match operation {
        AbstractOperation::EstablishByteSequenceConcatenation { result, .. } => {
            result.place == *source
        }
        AbstractOperation::CallStructural {
            result: AbstractStructuralCallResult::Structural { result, .. },
            ..
        } => {
            result.place == *source
                && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && result.projected_qualifications.is_empty()
                && result.qualification_establishments.is_empty()
                && result.claims.is_empty()
                && structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            )
                        )
                })
        }
        _ => false,
    }) || function.structural_parameters.iter().any(|parameter| {
        parameter.place == *source
            && abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter_in(
                parameter,
                structural_types,
            )
    }) {
        8
    } else {
        0
    };
    read.place == *source
        && read.byte_count == 0
        && read.byte_offset == source_byte_offset
        && read_length == *length
        && read_obligation == *obligation
        && write.place == write_place
        && write.byte_count == 0
        && write.role
            == Role::WriteByteSpan {
                length: *length,
                obligation: *obligation,
                accepted_fact,
            }
        && read.instruction == write.instruction
        && metadata_write.place == write_place
        && metadata_write.byte_count == 8
        && metadata_write.byte_offset.checked_add(8) == Some(write.byte_offset)
        && metadata_write.instruction > write.instruction
        && reads.next().is_none()
        && writes.next().is_none()
        && metadata.next().is_none()
}

pub(super) fn retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
    if let AbstractOperation::StructuralByteSequenceFieldLength {
        psi_operation,
        result,
        source,
        path,
        field,
    } = operation
    {
        let Some((access, _)) = read_access(function, target, *source, AccessPurpose::ByteLength)
        else {
            return false;
        };
        let expected_source = StructuralArgument {
            place: *source,
            access,
            path: path.clone(),
        };
        // Mandatory graph replay reconstructs bounded-field geometry and u64
        // metadata typing. Publication rejoins this exact observation, not a
        // content read or a synthesized whole-view descriptor.
        let mut reads = target
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation {
                TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation: identity,
                    result,
                    source,
                    field,
                } if identity == psi_operation => Some((result, source, field)),
                _ => None,
            });
        return reads.next() == Some((result, &expected_source, field)) && reads.next().is_none();
    }
    if let AbstractOperation::PrimitiveScalarRead {
        psi_operation,
        result,
        source,
        path,
    } = operation
    {
        let Some((_, root)) = read_access(function, target, *source, AccessPurpose::Field) else {
            return false;
        };
        // A read may end at a scalar field of a runtime-selected record
        // element; the leaf is rebuilt from declarations, never from a
        // retained offset.
        if path.is_empty()
            || terminal_semantics::primitive_read_projection_type(
                target.graph.structural_types.iter(),
                root,
                path,
            ) != Some(result.scalar_type)
        {
            return false;
        }
        // As with record-field reads, publication retains the exact graph
        // subject, and each runtime element's traversal names that element's
        // selector. Mandatory source/selection replay independently checks the
        // computed offset, strides and load on the original root; do not
        // duplicate its layout calculator or accept a matching byte offset as
        // path identity.
        let selectors = operation
            .runtime_indices()
            .into_iter()
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let mut reads = target
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| match operation {
                TargetUnitOperation::PrimitiveScalarRead {
                    psi_operation: identity,
                    result,
                    source,
                    path,
                    indices,
                } if identity == psi_operation => Some((
                    result,
                    source,
                    path,
                    indices
                        .iter()
                        .map(|index| index.operand.source_value())
                        .collect::<Vec<_>>(),
                )),
                _ => None,
            });
        return reads.next() == Some((result, source, path, selectors)) && reads.next().is_none();
    }
    let AbstractOperation::StructuralScalarFieldRead {
        psi_operation: identity,
        result,
        source: place,
        path,
        field,
    } = operation
    else {
        return false;
    };
    let (identity, result, place, field) = (*identity, *result, *place, *field);
    let Some((mut access, mut carrier)) =
        read_access(function, target, place, AccessPurpose::Field)
    else {
        return false;
    };
    let mut root_path = Vec::new();
    if let Some(terminal_psi::StructuralTypeShape::Reference {
        referent,
        access: reference_access,
    }) = target
        .graph
        .structural_types
        .iter()
        .find(|declaration| declaration.id == carrier)
        .map(|declaration| &declaration.shape)
    {
        if access != terminal_psi::StructuralAccess::Owned
            || !matches!(
                reference_access,
                terminal_psi::StructuralAccess::SharedBorrow
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
        {
            return false;
        }
        // Rejoin the pointer's exact producer home above; source/graph replay
        // checks live custody. Origin alternatives must not select its address.
        carrier = *referent;
        access = terminal_psi::StructuralAccess::SharedBorrow;
        root_path.push(terminal_psi::StructuralPathSegment::Referent);
    }
    // A final `Case` step names the payload overlay the field belongs to. It
    // adds no runtime segment; the published row must carry that same case.
    let (carrier_path, case) = match path.split_last() {
        Some((semantic_vocabulary::CanonicalStructuralPathSegment::Case(case), prefix)) => {
            (prefix, Some(*case))
        }
        _ => (path.as_slice(), None),
    };
    let mut runtime_path = Vec::with_capacity(carrier_path.len());
    for (position, segment) in carrier_path.iter().enumerate() {
        match segment {
            semantic_vocabulary::CanonicalStructuralPathSegment::Field(field) => {
                let Some(declaration) = target
                    .graph
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == carrier)
                else {
                    return false;
                };
                let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape
                else {
                    return false;
                };
                let Some(selected) = fields
                    .iter()
                    .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())
                else {
                    return false;
                };
                let terminal_psi::StructuralFieldType::Structural(child) = selected.field_type
                else {
                    return false;
                };
                runtime_path.push(terminal_psi::StructuralPathSegment::Field(
                    selected.identity.clone(),
                ));
                carrier = child;
            }
            // Fixed-array indices compose with later record fields and replay
            // each declared bound. Element views keep the terminal-index rule:
            // their bound comes from the separately proved live extent.
            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index) => {
                let Some(declaration) = target
                    .graph
                    .structural_types
                    .iter()
                    .find(|declaration| declaration.id == carrier)
                else {
                    return false;
                };
                let element = match &declaration.shape {
                    terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                        if index >= length {
                            return false;
                        }
                        *element
                    }
                    terminal_psi::StructuralTypeShape::ElementView { element }
                        if position + 1 == carrier_path.len() =>
                    {
                        *element
                    }
                    _ => return false,
                };
                runtime_path.push(terminal_psi::StructuralPathSegment::FixedIndex(*index));
                carrier = element;
            }
            _ => return false,
        }
    }
    root_path.extend(runtime_path.iter().cloned());
    let expected_source = StructuralArgument {
        place,
        access,
        path: root_path,
    };
    let mut reads = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation {
            TargetUnitOperation::StructuralScalarFieldRead {
                psi_operation,
                result,
                source,
                case,
                field,
            } if *psi_operation == identity => Some((result, source, case, field)),
            _ => None,
        });
    matches!(reads.next(), Some((actual_result, source, actual_case, actual_field))
        if *actual_result == result && *source == expected_source && *actual_case == case
            && *actual_field == field)
        && reads.next().is_none()
}

/// Rejoin original storage declarations without manufacturing a parameter for
/// an operation result. Mandatory graph replay checks pointwise availability.
#[derive(Clone, Copy, PartialEq, Eq)]
enum AccessPurpose {
    Field,
    ByteLength,
    ByteOwnerReplacement,
}

/// Retain the actual pointer carrier for a write through a returned reference.
/// Every candidate constrains custody; none chooses the runtime address.
fn write_destination(
    function: &AbstractFunction,
    target: &TargetFunction,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    purpose: AccessPurpose,
) -> Option<StructuralArgument> {
    if !matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) && let Some((access, structural_type)) = read_access(function, target, place, purpose)
    {
        // Whole byte-field replacement may mutate an existing copy-record
        // home. read_access has already rejoined its exact target producer;
        // this adds no owned-parameter or indexed-store write authority.
        let owned_record = purpose == AccessPurpose::ByteLength
            && access == terminal_psi::StructuralAccess::Owned
            && target.graph.structural_types.iter().any(|declaration| {
                declaration.id == structural_type
                    && matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Record { .. }
                    )
            })
            && function.operations.iter().any(|operation| {
                let result = match operation {
                    AbstractOperation::EstablishRecord { result, .. }
                    | AbstractOperation::EstablishZeroedStructural { result, .. }
                    | AbstractOperation::StructuralLeafCopy { result, .. }
                    | AbstractOperation::CallStructural {
                        result: AbstractStructuralCallResult::Structural { result, .. },
                        ..
                    } => result,
                    _ => return false,
                };
                result.place == place
                    && result.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                    && result.qualification_establishments.is_empty()
            });
        return (owned_record
            || matches!(
                access,
                terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ))
        .then(|| StructuralArgument {
            place,
            access,
            path: path.to_vec(),
        });
    }
    let (terminal_psi::StructuralPathSegment::Referent, _) = path.split_first()? else {
        return None;
    };
    let mut producers = function
        .operations
        .iter()
        .filter_map(|operation| match operation {
            AbstractOperation::CallStructural {
                psi_operation,
                callee,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if result.place == place => Some((*psi_operation, *callee, result)),
            _ => None,
        });
    let (producer, callee, result) = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    if result.multiplicity != terminal_psi::StructuralMultiplicity::Affine
        || !result.qualification_establishments.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !target.graph.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    terminal_psi::StructuralTypeShape::Reference {
                        access: terminal_psi::StructuralAccess::MutableBorrow,
                        ..
                    }
                )
        })
    {
        return None;
    }
    let terminal_psi::StructuralTypeShape::Reference { referent, .. } = target
        .graph
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)?
        .shape
    else {
        return None;
    };
    if !target.graph.structural_types.iter().any(|declaration| {
        declaration.id == referent
            && matches!(
                declaration.shape,
                terminal_psi::StructuralTypeShape::Record { .. }
            )
    }) {
        return None;
    }
    let mut retained = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation {
            TargetUnitOperation::Call {
                psi_operation,
                callee: actual_callee,
                result:
                    target_operations::TargetCallResult::Structural {
                        result: actual,
                        result_home: Some(home),
                        reference_results,
                        ..
                    },
                ..
            } if *psi_operation == producer
                && *actual_callee == callee
                && actual == result
                && home.operation_result() == Some((producer, result))
                && home.layout
                    == target_operations::TargetStructuralHomeLayout::Aggregate(
                        calling_conventions::ValueShape::integer(8, 8),
                    ) =>
            {
                Some(reference_results)
            }
            _ => None,
        });
    let [leaf] = retained.next()?.as_slice() else {
        return None;
    };
    if retained.next().is_some()
        || !leaf.path.is_empty()
        || leaf.root_paths.is_empty()
        || leaf.root_paths.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return None;
    }
    let (access, root_type) = read_access(function, target, leaf.root, purpose)?;
    if !matches!(
        access,
        terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    ) || leaf.root_paths.iter().any(|path| {
        if path.is_empty() {
            return root_type != referent;
        }
        crate::object_artifact::replay::structural::condition_layout::replay_structural_projection(
            root_type,
            path,
            &target.graph.structural_types,
        )
        .is_none_or(|(selected, _, _)| selected != referent)
    }) {
        return None;
    }
    Some(StructuralArgument {
        place,
        access: terminal_psi::StructuralAccess::MutableBorrow,
        path: path.to_vec(),
    })
}

pub(super) fn scalar_store_place(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> Option<semantic_vocabulary::PlaceId> {
    let AbstractOperation::StructuralScalarFieldStore {
        destination, path, ..
    } = operation
    else {
        return None;
    };
    if !matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        return Some(destination.place());
    }
    let abstract_operations::StructuralScalarStoreDestination::OwnedResult { producer, result } =
        destination
    else {
        return None;
    };
    if !function.operations.iter().any(|operation| matches!(operation,
        AbstractOperation::CallStructural { psi_operation, result: AbstractStructuralCallResult::Structural { result: actual, .. }, .. }
        if psi_operation == producer && actual == result)) { return None; }
    write_destination(
        function,
        target,
        destination.place(),
        path,
        AccessPurpose::Field,
    )
    .map(|destination| destination.place)
}

fn pointer_read_before(
    accesses: &[selected_instructions::SelectedMemoryAccess],
    operation: semantic_vocabulary::OperationId,
    carrier: semantic_vocabulary::PlaceId,
    before: selected_instructions::SelectedInstructionId,
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let mut reads = accesses.iter().filter(|access| {
        access.origin == Origin::Operation(operation)
            && access.role == Role::ReadPlace
            && access.place == carrier
    });
    matches!(reads.next(), Some(read) if read.byte_offset == 0 && read.byte_count == 8
        && read.instruction < before)
        && reads.next().is_none()
}

pub(super) fn scalar_store_footprints_retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
    accesses: &[selected_instructions::SelectedMemoryAccess],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        path,
        ..
    } = operation
    else {
        return false;
    };
    let Some(place) = scalar_store_place(function, operation, target) else {
        return false;
    };
    let mut writes = accesses.iter().filter(|access| {
        access.origin == Origin::Operation(*psi_operation)
            && access.place == place
            && access.role == Role::WritePlace
    });
    let Some(write) = writes.next() else {
        return false;
    };
    if matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) {
        writes.next().is_none()
            && pointer_read_before(accesses, *psi_operation, place, write.instruction)
    } else {
        true
    }
}

fn read_access(
    function: &AbstractFunction,
    target: &TargetFunction,
    place: semantic_vocabulary::PlaceId,
    purpose: AccessPurpose,
) -> Option<(
    terminal_psi::StructuralAccess,
    semantic_vocabulary::StructuralTypeId,
)> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    let observes_byte_length = purpose != AccessPurpose::Field;
    let mut access = None;
    for (position, parameter) in function
        .structural_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.place == place)
    {
        let retained = target.graph.parameters.get(position)?;
        if parameter.position as usize != position
            || !(matches!(
                parameter.access,
                StructuralAccess::Owned | StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
                || (parameter.access == StructuralAccess::WriteOnlyBorrow && observes_byte_length))
            || parameter.multiplicity == StructuralMultiplicity::Linear
            // Whole-owner replacement carries the declared predicate roster;
            // mandatory object/source replay checks its exact preservation and
            // the source proof. Other field observations retain their fence.
            || (!parameter.qualifications.is_empty()
                && purpose != AccessPurpose::ByteOwnerReplacement)
            || (purpose == AccessPurpose::ByteOwnerReplacement
                && !target.graph.structural_types.iter().any(|declaration| {
                    declaration.id == parameter.structural_type
                        && matches!(declaration.shape,
                            terminal_psi::StructuralTypeShape::ByteSequence(
                                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                            ))
                }))
            || !parameter.projected_qualifications.is_empty()
            || retained.place != place
            || retained.structural_type != parameter.structural_type
            || retained.access != parameter.access
            || retained.multiplicity != parameter.multiplicity
            || retained.projected_qualifications != parameter.projected_qualifications
            || access
                .replace((parameter.access, parameter.structural_type))
                .is_some()
        {
            return None;
        }
    }
    for block in &function.block_entries {
        for parameter in block
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.place == place)
        {
            // A block loan observes the original pointer retained by its
            // exact graph declaration, just as an owned arrival observes its
            // own home. Source/selection replay checks that pointer transport.
            if purpose == AccessPurpose::ByteOwnerReplacement
                || !(parameter.access == StructuralAccess::Owned
                    || abstract_operations::control_flow::address_joins::is_address_join_in(
                        parameter,
                        &target.graph.structural_types,
                    ))
                || parameter.multiplicity == StructuralMultiplicity::Linear
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || !target.graph.blocks.iter().any(|candidate| {
                    candidate.block == block.block
                        && candidate
                            .structural_parameters
                            .iter()
                            .any(|retained| retained == parameter)
                })
                || access
                    .replace((parameter.access, parameter.structural_type))
                    .is_some()
            {
                return None;
            }
        }
    }
    for operation in &function.operations {
        // An element view's descriptor is the lending carrier: reads through
        // its place borrow the view, so the minted access follows the view
        // result's multiplicity rather than an owned home.
        if let AbstractOperation::EstablishElementView { result, .. }
        | AbstractOperation::ElementViewSubslice { result, .. } = operation
        {
            if result.place != place || observes_byte_length {
                continue;
            }
            // An element read borrows the view's live extent however the
            // view itself was lent: legalization spells the argument
            // SharedBorrow for either multiplicity.
            let view_access = match result.multiplicity {
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine => {
                    StructuralAccess::SharedBorrow
                }
                _ => return None,
            };
            let retained = target
                .graph
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|candidate| {
                    let actual = match candidate {
                        TargetUnitOperation::EstablishElementView { result, .. }
                        | TargetUnitOperation::ElementViewSubslice { result, .. } => result,
                        _ => return false,
                    };
                    actual == result
                });
            if result.multiplicity == StructuralMultiplicity::Linear
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || !result.claims.is_empty()
                || !retained
                || access
                    .replace((view_access, result.structural_type))
                    .is_some()
            {
                return None;
            }
            continue;
        }
        let (producer, result) = match operation {
            AbstractOperation::EstablishReference {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::EstablishRecord {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::EstablishZeroedStructural {
                psi_operation,
                result,
            }
            | AbstractOperation::StructuralLeafCopy {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::MoveStructuralField {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            // A scalar array literal's home is the same owned arrival the
            // record arms admit: element reads borrow that place, and the
            // retained row carries its `result_home` home declaration.
            | AbstractOperation::EstablishScalarArray {
                psi_operation,
                result,
                ..
            } if result.place == place => (*psi_operation, result),
            _ => continue,
        };
        // Metadata reads observe the same established home as scalar fields.
        // This does not authorize replacing an owned byte carrier: mutation
        // keeps its separately checked borrowed-destination contract.
        if purpose == AccessPurpose::ByteOwnerReplacement
            || result.multiplicity == StructuralMultiplicity::Linear
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !target
                .graph
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    let home = match operation {
                        TargetUnitOperation::EstablishReference { result_home, .. }
                        | TargetUnitOperation::EstablishRecord { result_home, .. }
                        | TargetUnitOperation::EstablishZeroedStructural { result_home, .. }
                        | TargetUnitOperation::StructuralLeafCopy { result_home, .. }
                        | TargetUnitOperation::EstablishScalarArray { result_home, .. }
                        | TargetUnitOperation::Call {
                            result:
                                target_operations::TargetCallResult::Structural {
                                    result_home: Some(result_home),
                                    ..
                                },
                            ..
                        } => result_home,
                        _ => return false,
                    };
                    home.operation_result()
                        .is_some_and(|(actual, declaration)| {
                            actual == producer && declaration == result
                        })
                })
            || access
                .replace((StructuralAccess::Owned, result.structural_type))
                .is_some()
        {
            return None;
        }
    }
    access
}
