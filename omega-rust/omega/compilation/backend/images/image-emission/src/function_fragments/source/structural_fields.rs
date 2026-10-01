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
        terminal_psi::StructuralAccess::SharedBorrow
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

/// The one footprint row an indexed field byte read publishes: its source
/// root, one byte, and its own selector, length and obligation.
pub(super) fn indexed_read_footprint_retained(
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
    // A read spelled on a live `&` carrier names no storage of its own: the
    // producing call's reference roster re-bases it onto the referent root,
    // and the runtime path walks the root's declaration through that prefix.
    let (arg_place, mut root_path, access, mut carrier) = if let Some((access, carrier)) =
        read_access(function, target, place, AccessPurpose::Field)
    {
        (place, Vec::new(), access, carrier)
    } else {
        let Some((root_place, root_path)) = reference_root(target, place) else {
            return false;
        };
        let Some((access, carrier)) =
            read_access(function, target, root_place, AccessPurpose::Field)
        else {
            return false;
        };
        (root_place, root_path, access, carrier)
    };
    for segment in &root_path {
        let terminal_psi::StructuralPathSegment::Field(field_identity) = segment else {
            return false;
        };
        let Some(declaration) = target
            .graph
            .structural_types
            .iter()
            .find(|declaration| declaration.id == carrier)
        else {
            return false;
        };
        let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
            return false;
        };
        let Some(selected) = fields.iter().find(|candidate| {
            candidate.identity == *field_identity && !candidate.relevance.is_erased()
        }) else {
            return false;
        };
        let terminal_psi::StructuralFieldType::Structural(child) = selected.field_type else {
            return false;
        };
        carrier = child;
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
            // The bounded carrier grammar ends with at most one literal
            // index: the fixed-array element is the record owning the
            // observed field. The bound is rechecked against the declared
            // extent rather than trusted from the producer.
            // An element view contributes its own live extent as the index
            // bound; the producer's establishment already discharged it, so
            // only the declared element joins the carrier walk.
            semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(index)
                if position + 1 == carrier_path.len() =>
            {
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
                    terminal_psi::StructuralTypeShape::ElementView { element } => *element,
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
        place: arg_place,
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

/// The referent root a live `&` carrier resolves to: the producing call's
/// published reference roster names the whole-carrier leaf (`path` empty) by
/// root place and the referent's path beneath it.
fn reference_root(
    target: &TargetFunction,
    place: semantic_vocabulary::PlaceId,
) -> Option<(
    semantic_vocabulary::PlaceId,
    Vec<terminal_psi::StructuralPathSegment>,
)> {
    let mut calls = target
        .graph
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation {
            TargetUnitOperation::Call {
                result:
                    target_operations::TargetCallResult::Structural {
                        result,
                        result_home: None,
                        reference_results,
                        ..
                    },
                ..
            } if result.place == place => Some(reference_results),
            _ => None,
        });
    let leaves = calls.next()?;
    if calls.next().is_some() {
        return None;
    }
    let mut roots = leaves.iter().filter(|leaf| leaf.path.is_empty());
    let root = roots.next()?;
    (roots.next().is_none()).then(|| (root.root, root.root_path.clone()))
}

/// Resolve a write spelled on a bare returned reference onto the exact root
/// already retained by its call-result custody roster. The carrier remains
/// zero-storage permission metadata; publication never treats it as a home or
/// descriptor.
fn write_destination(
    function: &AbstractFunction,
    target: &TargetFunction,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    purpose: AccessPurpose,
) -> Option<StructuralArgument> {
    if let Some((access, _)) = read_access(function, target, place, purpose) {
        return matches!(
            access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        )
        .then(|| StructuralArgument {
            place,
            access,
            path: path.to_vec(),
        });
    }
    let (terminal_psi::StructuralPathSegment::Referent, tail) = path.split_first()? else {
        return None;
    };
    let result = function
        .operations
        .iter()
        .find_map(|operation| match operation {
            AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if result.place == place => Some(result),
            _ => None,
        })?;
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
    let (root, mut root_path) = reference_root(target, place)?;
    let (access, _) = read_access(function, target, root, purpose)?;
    if !matches!(
        access,
        terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    ) {
        return None;
    }
    root_path.extend_from_slice(tail);
    Some(StructuralArgument {
        place: root,
        access,
        path: root_path,
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
    write_destination(
        function,
        target,
        destination.place(),
        path,
        AccessPurpose::Field,
    )
    .map(|destination| destination.place)
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
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            ) || (parameter.access == StructuralAccess::Owned && !observes_byte_length)
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
            if observes_byte_length
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
            AbstractOperation::EstablishRecord {
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
                        TargetUnitOperation::EstablishRecord { result_home, .. }
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
