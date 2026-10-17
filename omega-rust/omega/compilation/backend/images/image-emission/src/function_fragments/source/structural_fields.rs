//! Publication joins field observations to their exact ordinary graph operations.
//! Mandatory object/source replay derives layout and validates the selected load,
//! including the original referent or owned input's current local home. No second
//! field-offset calculator or legacy expression-tree realization belongs here.

use abstract_operations::{AbstractFunction, AbstractOperation, AbstractStructuralCallResult};
use semantic_vocabulary::ScalarType;
use target_operations::{TargetFunction, TargetUnitOperation};
use terminal_psi::{StructuralArgument, live_field_by_id};

#[cfg(test)]
mod tests;

/// Access, geometry and bounds are independently replayed by source::admit's
/// mandatory current-graph validation. Publication binds the emitted occurrence
/// to the same named selector operands rather than discovering another producer catalog.
pub(super) fn retained_selectors(
    path: &[terminal_psi::StructuralPathSegment],
    indices: &[target_operations::TargetStructuralRuntimeIndex],
) -> bool {
    let mut selectors = path.iter().filter_map(|segment| segment.runtime_index());
    indices.iter().all(|retained| {
        selectors
            .next()
            .is_some_and(|(index, _)| index == retained.operand.source_value())
    }) && selectors.next().is_none()
}

/// Bind a consumed place's declaration to the same target storage producer.
/// Permission follows mandatory graph replay, not the physical home category.
fn retained_field_root(
    function: &AbstractFunction,
    target: &TargetFunction,
    place: semantic_vocabulary::PlaceId,
) -> bool {
    if let Some(parameter) = function.arrival_parameter(place) {
        if let Some(position) = function
            .structural_parameters
            .iter()
            .position(|row| row == parameter)
        {
            return target.graph.parameters.get(position).is_some_and(|row| {
                row.place == place
                    && row.structural_type == parameter.structural_type
                    && row.access == parameter.access
                    && row.multiplicity == parameter.multiplicity
                    && row.projected_qualifications == parameter.projected_qualifications
            });
        }
        return function.block_entries.iter().any(|block| {
            block.structural_parameters.contains(parameter)
                && target.graph.blocks.iter().any(|retained| {
                    retained.block == block.block
                        && retained
                            .structural_parameters
                            .get(parameter.position as usize)
                            == Some(parameter)
                })
        });
    }
    let mut declarations = function
        .operations
        .iter()
        .filter_map(|operation| operation.structural_result())
        .filter(|(_, result)| result.place == place);
    let Some((producer, declaration)) = declarations.next() else {
        return false;
    };
    if declarations.next().is_some() {
        return false;
    }
    let mut retained = target.graph.operations().filter(|operation| {
        if let Some((identity, home)) = operation.structural_result_home() {
            return identity == producer
                && home.operation_result() == Some((producer, declaration));
        }
        match operation {
            TargetUnitOperation::EstablishElementView {
                result,
                view: target_operations::TargetElementView::Established { psi_operation, .. },
            }
            | TargetUnitOperation::ElementViewSubslice {
                result,
                view: target_operations::TargetElementView::Subslice { psi_operation, .. },
            } => *psi_operation == producer && result == declaration,
            _ => false,
        }
    });
    retained.next().is_some() && retained.next().is_none()
}

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
    let mut writes = target.graph.operations().filter(|candidate| {
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
        }
        | AbstractOperation::StructuralByteSequenceFieldLength {
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
    let mut reads = target.graph.operations().filter(|candidate| {
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
        indices,
        ..
    }) = reads.next()
    else {
        return false;
    };
    retained_field_root(function, target, *source)
        && retained_result == result
        && retained_source.place == *source
        && retained_source.path == *path
        && retained_source.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
        && retained_selectors(path, indices)
        && retained_field == field
        && retained_length == length
        && retained_obligation == obligation
        && retained_index.source_value() == *index
        && matches!(retained_index.scalar_type(), ScalarType::Integer(integer)
            if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64))
        && reads.next().is_none()
}

fn field_root_type(
    function: &AbstractFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let mut roots = function
        .arrival_parameter(place)
        .map(|parameter| parameter.structural_type)
        .into_iter()
        .chain(function.operations.iter().filter_map(|operation| {
            let (_, result) = operation.structural_result()?;
            (result.place == place).then_some(result.structural_type)
        }));
    let root = roots.next()?;
    roots.next().is_none().then_some(root)
}

fn loads_backing_pointer(
    function: &AbstractFunction,
    target: &TargetFunction,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> bool {
    matches!(
        path.first(),
        Some(terminal_psi::StructuralPathSegment::Referent)
    ) || field_root_type(function, place).is_some_and(|root| {
        target.graph.structural_types.iter().any(|row| {
            row.id == root
                && matches!(
                    row.shape,
                    terminal_psi::StructuralTypeShape::ElementView { .. }
                )
        })
    })
}

/// Whether an indexed field read follows a view field's descriptor pointer
/// rather than addressing inline bounded-owned bytes. Descriptor layout is
/// independent of access; mandatory source replay owns read permission. Resolve
/// the carrier from the exact root declaration, never from a footprint shape.
fn reads_through_descriptor(
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
    field_root_type(function, *source).is_some_and(|root| {
        matches!(
            field_carrier(target, root, path, *field),
            Some(terminal_psi::ByteSequenceCarrier::BorrowedView { .. })
        )
    })
}

/// The declared byte carrier below the operation's retained projection.
fn field_carrier(
    target: &TargetFunction,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<terminal_psi::ByteSequenceCarrier> {
    let mut carrier = root;
    for segment in path {
        let declaration =
            terminal_psi::find_structural_type(&target.graph.structural_types, carrier)?;
        carrier = match (segment, &declaration.shape) {
            (
                terminal_psi::StructuralPathSegment::RuntimeIndex { .. },
                terminal_psi::StructuralTypeShape::FixedArray { element, .. }
                | terminal_psi::StructuralTypeShape::ElementView { element, .. },
            ) => *element,
            (
                terminal_psi::StructuralPathSegment::Referent,
                terminal_psi::StructuralTypeShape::Reference { referent, .. },
            ) => *referent,
            _ => {
                terminal_semantics::runtime_structural_path_tip(
                    target.graph.structural_types.iter(),
                    carrier,
                    std::slice::from_ref(segment),
                )?
                .id
            }
        };
    }
    let terminal_psi::StructuralTypeShape::Record { fields, .. } =
        &terminal_psi::find_structural_type(&target.graph.structural_types, carrier)?.shape
    else {
        return None;
    };
    live_field_by_id(fields, field)?
        .field_type
        .byte_sequence_carrier()
}

/// The footprint rows an indexed field byte read publishes: its source root,
/// one byte, and its own selector, length and obligation. A read through a
/// view field's descriptor first loads that descriptor's pointer word
/// from the same root.
pub(super) fn indexed_read_footprint_retained(
    function: &AbstractFunction,
    target: &TargetFunction,
    operation: &AbstractOperation,
    accesses: &[selected_instructions::SelectedMemoryAccess],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let AbstractOperation::StructuralByteSequenceFieldRead {
        psi_operation,
        source,
        path,
        index,
        length,
        obligation,
        ..
    } = operation
    else {
        return false;
    };
    let effects = accesses
        .iter()
        .filter(|access| access.origin == Origin::Operation(*psi_operation))
        .collect::<Vec<_>>();
    let mut cursor = effects.as_slice();
    // A descriptor may already retain its backing pointer in a register.
    // Account for a reload when present; either realization publishes the
    // same field access rows below.
    let mut consumed_backing = false;
    if loads_backing_pointer(function, target, *source, path)
        && let Some((backing, rest)) = cursor.split_first()
        && backing.place == *source
        && backing.byte_count == 8
        && backing.role == Role::ReadPlace
    {
        cursor = rest;
        consumed_backing = matches!(
            path.first(),
            Some(terminal_psi::StructuralPathSegment::Referent)
        );
    }
    if reads_through_descriptor(function, operation, target) {
        let count = path
            .iter()
            .filter_map(|segment| segment.runtime_index())
            .count()
            .max(1);
        if cursor.len() < count || !projected_field_rows(&cursor[..count], path, *source, false) {
            return false;
        }
        cursor = &cursor[count..];
    } else {
        // Each `Referent` hop publishes the pointer load it reads through —
        // one 8-byte root access per hop before the terminal byte read.
        let hops = path
            .iter()
            .filter(|segment| matches!(segment, terminal_psi::StructuralPathSegment::Referent))
            .count()
            .saturating_sub(usize::from(consumed_backing));
        if cursor.len() < hops
            || !cursor[..hops].iter().all(|hop| {
                hop.place == *source && hop.byte_count == 8 && hop.role == Role::ReadPlace
            })
        {
            return false;
        }
        cursor = &cursor[hops..];
    }
    // The indexed-read target row above retains the exact selector, extent
    // and accepted obligation. A returned descriptor's memory effect instead
    // names completed call origins: the carrier's home is not its backing
    // allocation, so it cannot supply an inline one-byte footprint. These
    // opaque effects interfere with every store; their producer labels grant
    // neither memory authority nor disjointness. Physical address validation
    // remains with the retained selected operation, not these effect labels.
    if !cursor.is_empty()
        && cursor.iter().all(|read| {
            let Role::ReadReturnedView { producer } = read.role else {
                return false;
            };
            read.byte_offset == 0
                && read.byte_count == 0
                && target.graph.operations().any(|operation| {
                    matches!(operation, TargetUnitOperation::Call {
                        psi_operation,
                        result: target_operations::TargetCallResult::Structural { result, .. },
                        ..
                    } if *psi_operation == producer && result.place == read.place)
                })
        })
    {
        return true;
    }
    let [read] = cursor else {
        return false;
    };
    read.place == *source
        && read.byte_count == 1
        && matches!(read.role, Role::ReadByteSequence { index: actual_index, length: actual_length, obligation: actual_obligation, .. }
            if actual_index == *index && actual_length == *length && actual_obligation == *obligation)
}

/// Each dynamic path element publishes its own accepted bound on the same
/// load/store instruction. Physical encoding validation owns the address and
/// width; this publication check retains the bound's operation occurrence.
fn projected_field_rows(
    rows: &[&selected_instructions::SelectedMemoryAccess],
    path: &[terminal_psi::StructuralPathSegment],
    place: semantic_vocabulary::PlaceId,
    writing: bool,
) -> bool {
    use selected_instructions::SelectedMemoryAccessRole as Role;
    let selectors = path
        .iter()
        .filter_map(|segment| segment.runtime_index())
        .collect::<Vec<_>>();
    let Some(first) = rows.first() else {
        return false;
    };
    if rows.len() != selectors.len().max(1)
        || rows.iter().any(|row| {
            row.place != place
                || row.byte_count != 8
                || row.byte_offset != first.byte_offset
                || row.instruction != first.instruction
        })
    {
        return false;
    }
    if selectors.is_empty() {
        return first.role
            == if writing {
                Role::WritePlace
            } else {
                Role::ReadPlace
            };
    }
    rows.iter()
        .zip(selectors)
        .all(|(row, (index, obligation))| match (writing, row.role) {
            (
                false,
                Role::ReadIndexedPrimitive {
                    index: actual_index,
                    obligation: actual_obligation,
                    ..
                },
            )
            | (
                true,
                Role::WriteIndexedStructural {
                    index: actual_index,
                    obligation: actual_obligation,
                    ..
                },
            ) => actual_index == index && actual_obligation == obligation,
            _ => false,
        })
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
        ..
    } = operation
    else {
        return false;
    };
    if source == destination || !retained_field_root(function, target, *destination) {
        return false;
    }
    let mut replacements = target
        .graph
        .operations()
        .filter_map(|operation| match operation {
            TargetUnitOperation::StructuralByteViewFieldStore {
                psi_operation: identity,
                destination,
                field,
                source,
                indices,
            } if identity == psi_operation => Some((destination, field, source, indices)),
            _ => None,
        });
    matches!(replacements.next(), Some((actual_destination, actual_field, actual_source, indices))
        if actual_destination.place == *destination && actual_destination.path == *path
            && matches!(actual_destination.access, terminal_psi::StructuralAccess::Owned | terminal_psi::StructuralAccess::MutableBorrow | terminal_psi::StructuralAccess::WriteOnlyBorrow)
            && retained_selectors(path, indices) && actual_field == field
            && actual_source == source)
        && replacements.next().is_none()
}

/// The four footprint rows of a descriptor replacement: the literal
/// descriptor's pointer and extent words read in order, then the same two
/// words written at the field's descriptor in the destination root.
pub(super) fn view_replacement_footprints_retained(
    function: &AbstractFunction,
    target: &TargetFunction,
    operation: &AbstractOperation,
    accesses: &[selected_instructions::SelectedMemoryAccess],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let AbstractOperation::StructuralByteViewFieldStore {
        psi_operation,
        source,
        destination,
        path,
        ..
    } = operation
    else {
        return false;
    };
    let effects = accesses
        .iter()
        .filter(|access| access.origin == Origin::Operation(*psi_operation))
        .collect::<Vec<_>>();
    let mut cursor = effects.as_slice();
    // A descriptor may already retain its backing pointer in a register.
    // Mandatory selection replay decides whether a reload is needed; either
    // realization still publishes the same strict field access rows below.
    if loads_backing_pointer(function, target, *destination, path)
        && let Some((backing, rest)) = cursor.split_first()
        && backing.place == *destination
        && backing.byte_count == 8
        && backing.role == Role::ReadPlace
    {
        cursor = rest;
    }
    let count = path
        .iter()
        .filter_map(|segment| segment.runtime_index())
        .count()
        .max(1);
    if cursor.len() != 2 + 2 * count {
        return false;
    }
    let pointer = cursor[0];
    let extent = cursor[1];
    let pointer_writes = &cursor[2..2 + count];
    let extent_writes = &cursor[2 + count..];
    pointer.place == *source
        && pointer.byte_offset == 0
        && pointer.byte_count == 8
        && pointer.role == Role::ReadPlace
        && extent.place == *source
        && extent.byte_offset == 8
        && extent.byte_count == 8
        && extent.role == Role::ReadPlace
        && projected_field_rows(pointer_writes, path, *destination, true)
        && projected_field_rows(extent_writes, path, *destination, true)
        && pointer_writes[0].byte_offset.checked_add(8) == Some(extent_writes[0].byte_offset)
        && pointer.instruction < extent.instruction
        && extent.instruction < pointer_writes[0].instruction
        && pointer_writes[0].instruction < extent_writes[0].instruction
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
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            )
        {
            return false;
        }
        let mut matching = target.graph.operations().filter(|operation| matches!(operation,
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
        .operations()
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
    let (abstract_operations::byte_sequence_store_pair_pattern!(
        psi_operation,
        destination: _,
        source,
        length,
        obligation,
        ..
    )) = operation
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
    // Entry and state arrivals, concatenations and bounded-owner call results
    // use the same `{live length, inline backing}` layout as bounded fields.
    // Borrowing an owner preserves that layout; source access is admitted
    // independently. Plain descriptors account for their backing from offset zero.
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
                        && declaration.shape.is_bounded_owned_byte_sequence()
                })
        }
        _ => false,
    }) || function
        .arrival_parameter(*source)
        .is_some_and(|parameter| {
            parameter.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && parameter.projected_qualifications.is_empty()
                && structural_types.iter().any(|declaration| {
                    declaration.id == parameter.structural_type
                        && declaration.shape.is_bounded_owned_byte_sequence()
                })
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
        // Mandatory graph replay reconstructs bounded-field geometry and u64
        // metadata typing. Publication rejoins this exact observation, not a
        // content read or a synthesized whole-view descriptor.
        let mut reads = target
            .graph
            .operations()
            .filter_map(|operation| match operation {
                TargetUnitOperation::StructuralByteSequenceFieldLength {
                    psi_operation: identity,
                    result,
                    source,
                    field,
                    indices,
                } if identity == psi_operation => Some((result, source, field, indices)),
                _ => None,
            });
        return retained_field_root(function, target, *source)
            && matches!(reads.next(), Some((actual_result, actual_source, actual_field, indices))
            if actual_result == result && actual_source.place == *source && actual_source.path == *path
                && actual_field == field && retained_selectors(path, indices))
            && reads.next().is_none();
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
    }) = terminal_psi::find_structural_type(target.graph.structural_types.as_slice(), carrier)
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
        // Observation retains the carrier's declaration; it does not mint a
        // separate shared loan or erase the existing mutable permission.
        carrier = *referent;
        access = *reference_access;
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
                let Some(declaration) = terminal_psi::find_structural_type(
                    target.graph.structural_types.as_slice(),
                    carrier,
                ) else {
                    return false;
                };
                let Some(fields) = declaration.shape.direct_fields() else {
                    return false;
                };
                let Some(selected) = live_field_by_id(fields, *field) else {
                    return false;
                };
                let Some(child) = selected.field_type.as_structural() else {
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
                let Some(declaration) = terminal_psi::find_structural_type(
                    target.graph.structural_types.as_slice(),
                    carrier,
                ) else {
                    return false;
                };
                let element = match &declaration.shape {
                    terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                        if index >= length {
                            return false;
                        }
                        *element
                    }
                    terminal_psi::StructuralTypeShape::ElementView { element, .. }
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
        .operations()
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
    ) && let Some((access, _)) = read_access(function, target, place, purpose)
    {
        // Whole field replacement retains the authenticated owned home,
        // whether produced here or received at a state edge. Mandatory graph
        // replay checks the selected field's access, geometry and predicates;
        // publication must not impose another outer-shape or producer roster.
        // Indexed-byte stores keep their separate borrowed-destination rule.
        let owned_home =
            purpose == AccessPurpose::ByteLength && access == terminal_psi::StructuralAccess::Owned;
        return (owned_home
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
    // Reference-result leaf custody currently comes from a private machine's
    // structural result declaration. Public requirement slots do not publish
    // that contract, so a dynamic target cannot stand in for this direct proof.
    if !matches!(callee, abstract_operations::AbstractCallTarget::Machine(_)) {
        return None;
    }
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
    let terminal_psi::StructuralTypeShape::Reference { referent, .. } =
        terminal_psi::find_structural_type(
            target.graph.structural_types.as_slice(),
            result.structural_type,
        )?
        .shape
    else {
        return None;
    };
    if !target
        .graph
        .structural_types
        .iter()
        .any(|declaration| declaration.id == referent && declaration.is_record_shape())
    {
        return None;
    }
    let mut retained = target
        .graph
        .operations()
        .filter_map(|operation| match operation {
            TargetUnitOperation::Call {
                psi_operation,
                target: actual_target,
                result:
                    target_operations::TargetCallResult::Structural {
                        result: actual,
                        result_home: Some(home),
                        reference_results,
                        ..
                    },
                ..
            } if *psi_operation == producer
                && super::call_target_matches(callee, *actual_target)
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
        || leaf.origins.is_empty()
        || leaf.origins.windows(2).any(|pair| pair[0] >= pair[1])
        || leaf.origins.iter().any(|origin| {
            let Some((access, root_type)) = read_access(function, target, origin.root, purpose)
            else {
                return true;
            };
            if !matches!(
                access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) {
                return true;
            }
            if origin.path.is_empty() {
                return root_type != referent;
            }
            terminal_semantics::captured_reference_origin_type(
                target.graph.structural_types.iter(),
                root_type,
                &origin.path,
            )
            .is_none_or(|selected| selected != referent)
        })
    {
        return None;
    }
    Some(StructuralArgument {
        place,
        access: terminal_psi::StructuralAccess::MutableBorrow,
        path: path.to_vec(),
    })
}

/// Join a store to its declared root and exact target operation. Descriptor
/// establishment, construction and call results all use the same store; the
/// producing operation's spelling cannot change its publication contract.
/// A retained logical store is not necessarily one fixed memory-footprint row:
/// encoded cells use an address and runtime codec. Mandatory graph and physical
/// replay authenticate that encoding, including every pointer/selector step.
pub(super) fn scalar_store_retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
    let AbstractOperation::StructuralScalarFieldStore {
        psi_operation,
        destination,
        path,
        field,
        value,
        ..
    } = operation
    else {
        return false;
    };
    let place = destination.place();
    if !retained_field_root(function, target, place) {
        return false;
    }
    let mut stores = target.graph.operations().filter(|operation| {
        matches!(operation,
            TargetUnitOperation::StructuralScalarFieldStore {
                psi_operation: retained, destination: retained_destination,
                path: retained_path, field: retained_field, source, indices, ..
            } if retained == psi_operation && retained_destination == destination
                && retained_path == path && retained_field == field
                && source.source_value() == value.value
                && source.scalar_type() == value.scalar_type
                && retained_selectors(path, indices)
        )
    });
    stores.next().is_some() && stores.next().is_none()
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

/// Owned field observations retain the complete obligation on the carrier.
/// Reference/view shells keep their separate referent and descriptor rules.
fn owned_observation_storage(
    types: &[terminal_psi::StructuralTypeDeclaration],
    structural_type: semantic_vocabulary::StructuralTypeId,
) -> bool {
    types.iter().any(|declaration| {
        declaration.id == structural_type && (declaration.shape).is_composite_shape()
    })
}

/// Access policy belongs to the declared storage, not whether its address
/// arrived through the entry ABI or a state edge. Their transport identities
/// are independently checked by `read_access` below.
fn parameter_access(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    types: &[terminal_psi::StructuralTypeDeclaration],
    purpose: AccessPurpose,
) -> bool {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
    let owned_composite = parameter.access == StructuralAccess::Owned
        && owned_observation_storage(types, parameter.structural_type);
    if (parameter.multiplicity == StructuralMultiplicity::Linear
        && (!owned_composite || purpose == AccessPurpose::ByteOwnerReplacement))
        || (parameter.access == StructuralAccess::WriteOnlyBorrow
            && purpose == AccessPurpose::Field)
    {
        return false;
    }
    // A field or length observation neither consumes nor establishes a
    // membership: a qualified owner, borrow or descriptor keeps its whole and
    // projected rosters unchanged, and the exact declaration join in
    // read_access binds that evidence after mandatory semantic/graph replay.
    // Only whole-owner replacement, which rewrites the bytes a predicate was
    // proved over, keeps the qualification fence below.
    if purpose != AccessPurpose::ByteOwnerReplacement {
        return true;
    }
    if !parameter.projected_qualifications.is_empty() {
        return false;
    }
    types.iter().any(|declaration| {
        declaration.id == parameter.structural_type
            && declaration.shape.is_bounded_owned_byte_sequence()
    })
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
            || !parameter_access(parameter, &target.graph.structural_types, purpose)
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
        for (position, parameter) in block
            .structural_parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| parameter.place == place)
        {
            // Access follows the declared parameter, independently of whether
            // native transport carries a referent address or a descriptor.
            // Mandatory source/selection replay checks that transport; here
            // publication rejoins the exact block declaration and permissions.
            if block.block == function.entry
                || parameter.position as usize != position
                || !parameter_access(parameter, &target.graph.structural_types, purpose)
                || !target.graph.blocks.iter().any(|candidate| {
                    candidate.block == block.block
                        && candidate.structural_parameters.get(position) == Some(parameter)
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
                terminal_psi::nonlinear_multiplicity_pair_pattern!() => {
                    StructuralAccess::SharedBorrow
                }
                _ => return None,
            };
            let retained = target.graph.operations().any(|candidate| {
                let actual = match candidate {
                    TargetUnitOperation::EstablishElementView { result, .. }
                    | TargetUnitOperation::ElementViewSubslice { result, .. } => result,
                    _ => return false,
                };
                actual == result
            });
            if result.multiplicity == StructuralMultiplicity::Linear
                || !result.qualifications.is_empty()
                || !terminal_semantics::view_descriptor_projected_qualifications(
                    target.graph.structural_types.iter(),
                    result.structural_type,
                    &result.projected_qualifications,
                )
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
        let Some((producer, result)) = operation.structural_result() else {
            continue;
        };
        if result.place != place {
            continue;
        }
        // Metadata reads observe the same established home as scalar fields.
        // Whole-byte replacement retains its exact writable owned home and
        // predicate obligations independently of the observation contract.
        // An observation carries the existing claim roster unchanged. Exact
        // source/home equality below rejoins that roster; construction and
        // current semantic replay, not this read, validate its authority.
        // Borrowed carriers keep their separate claim-free contract.
        let retained_custody = if purpose == AccessPurpose::ByteOwnerReplacement {
            result.multiplicity == StructuralMultiplicity::Unrestricted
                && result.claims.is_empty()
                && result.projected_qualifications.is_empty()
                && target.graph.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                        && declaration.shape.is_bounded_owned_byte_sequence()
                })
        } else {
            owned_observation_storage(&target.graph.structural_types, result.structural_type)
                || (result.multiplicity != StructuralMultiplicity::Linear
                    && result.claims.is_empty()
                    && result.qualifications.is_empty()
                    && terminal_semantics::view_descriptor_projected_qualifications(
                        target.graph.structural_types.iter(),
                        result.structural_type,
                        &result.projected_qualifications,
                    ))
        };
        if !retained_custody
            || !target.graph.operations().any(|operation| {
                // Primitive locals and descriptor results need not carry an
                // aggregate home. The observation binds the exact producer and
                // declaration; each establishment's own audit checks storage.
                operation.structural_result() == Some((producer, result))
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
