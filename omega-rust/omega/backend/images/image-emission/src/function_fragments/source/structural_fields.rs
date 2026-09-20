//! Publication joins field observations to their exact ordinary graph operations.
//! Mandatory object/source replay derives layout and validates the selected load,
//! including the original referent or owned input's current local home. No second
//! field-offset calculator or legacy expression-tree realization belongs here.

use abstract_operations::{AbstractFunction, AbstractOperation, AbstractResult};
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
    let Some((access, _)) = read_access(function, target, *destination, true) else {
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

/// Account for replacement's exact graph occurrence. Mandatory source/selection
/// replay owns capacity evidence, original backing, layout and copy correctness.
pub(super) fn replacement_retained(
    function: &AbstractFunction,
    operation: &AbstractOperation,
    target: &TargetFunction,
) -> bool {
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
    let Some((access, _)) = read_access(function, target, *destination, true) else {
        return false;
    };
    if !matches!(
        access,
        terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow
    ) || source == destination
    {
        return false;
    }
    let expected_destination = StructuralArgument {
        place: *destination,
        access,
        path: path.clone(),
    };
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
    operation: &AbstractOperation,
    accesses: &[selected_instructions::SelectedMemoryAccess],
) -> bool {
    use selected_instructions::{
        SelectedMemoryAccessOrigin as Origin, SelectedMemoryAccessRole as Role,
    };
    let AbstractOperation::StructuralByteSequenceFieldStore {
        psi_operation,
        destination,
        source,
        length,
        obligation,
        ..
    } = operation
    else {
        return false;
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
    read.place == *source
        && read.byte_count == 0
        && read.byte_offset == 0
        && read_length == *length
        && read_obligation == *obligation
        && write.place == *destination
        && write.byte_count == 0
        && write.role
            == Role::WriteByteSpan {
                length: *length,
                obligation: *obligation,
                accepted_fact,
            }
        && read.instruction == write.instruction
        && metadata_write.place == *destination
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
        let Some((access, _)) = read_access(function, target, *source, true) else {
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
        let Some((_, root)) = read_access(function, target, *source, false) else {
            return false;
        };
        if path.is_empty()
            || terminal_semantics::primitive_place_type(
                target.graph.structural_types.iter(),
                root,
                path,
            ) != Some(result.scalar_type)
        {
            return false;
        }
        // As with record-field reads, publication retains the exact graph
        // subject. Mandatory source/selection replay independently checks the
        // computed offset and load on the original root; do not duplicate its
        // layout calculator or accept a matching byte offset as path identity.
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
                } if identity == psi_operation => Some((result, source, path)),
                _ => None,
            });
        return reads.next() == Some((result, source, path)) && reads.next().is_none();
    }
    let (identity, result, place, path, field) = match operation {
        AbstractOperation::IntegerStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (*psi_operation, *result, *source, path, *field),
        AbstractOperation::BooleanStructuralField {
            psi_operation,
            result,
            source,
            path,
            field,
        } => (
            *psi_operation,
            AbstractResult {
                value: *result,
                scalar_type: ScalarType::Boolean,
            },
            *source,
            path,
            *field,
        ),
        _ => return false,
    };
    let Some((access, mut carrier)) = read_access(function, target, place, false) else {
        return false;
    };
    let mut runtime_path = Vec::with_capacity(path.len());
    for segment in path {
        let semantic_vocabulary::CanonicalStructuralPathSegment::Field(field) = segment else {
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
        let Some(selected) = fields
            .iter()
            .find(|candidate| candidate.id == *field && !candidate.relevance.is_erased())
        else {
            return false;
        };
        let terminal_psi::StructuralFieldType::Structural(child) = selected.field_type else {
            return false;
        };
        runtime_path.push(terminal_psi::StructuralPathSegment::Field(
            selected.identity.clone(),
        ));
        carrier = child;
    }
    let expected_source = StructuralArgument {
        place,
        access,
        path: runtime_path,
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
                field,
            } if *psi_operation == identity => Some((result, source, field)),
            _ => None,
        });
    matches!(reads.next(), Some((actual_result, source, actual_field))
        if *actual_result == result && *source == expected_source && *actual_field == field)
        && reads.next().is_none()
}

/// Rejoin original storage declarations without manufacturing a parameter for
/// an operation result. Mandatory graph replay checks pointwise availability.
fn read_access(
    function: &AbstractFunction,
    target: &TargetFunction,
    place: semantic_vocabulary::PlaceId,
    observes_byte_length: bool,
) -> Option<(
    terminal_psi::StructuralAccess,
    semantic_vocabulary::StructuralTypeId,
)> {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity};
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
            || !parameter.qualifications.is_empty()
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
            if observes_byte_length
                || parameter.access != StructuralAccess::Owned
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
        let (producer, result) = match operation {
            AbstractOperation::EstablishRecord {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                result,
                ..
            } if result.place == place => (*psi_operation, result),
            _ => continue,
        };
        if observes_byte_length
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
