//! Operations that read or write retained storage: references, byte-sequence
//! fields and views, primitive and structural scalar stores, field reads,
//! case membership and dynamic descriptors.

use super::{structural_place_type, validate_structural_path};
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::{IntegerSign, ScalarType, StructuralPlaceKind};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalMachine, TerminalModule, is_bounded_structural_scalar_store_path,
};

/// Resolve only the reference shell's declared geometry. Origin, live loan
/// authority and the referent's owned-content restrictions remain independent
/// verifier judgments; canonical encoding does not grant a storage permission.
fn reference_store_carrier<'path>(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: semantic_vocabulary::PlaceId,
    path: &'path [terminal_psi::StructuralPathSegment],
) -> Option<(
    semantic_vocabulary::StructuralTypeId,
    &'path [terminal_psi::StructuralPathSegment],
)> {
    let (terminal_psi::StructuralPathSegment::Referent, suffix) = path.split_first()? else {
        return None;
    };
    let structural_type = structural_place_type(machine, destination)?;
    let parameter = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == destination);
    let exact = if let Some(parameter) = parameter {
        parameter.multiplicity == StructuralMultiplicity::Affine
            && parameter.qualifications.is_empty()
            && parameter.projected_qualifications.is_empty()
    } else {
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|operation| operation.result.structural())
            .any(|result| {
                result.place == destination
                    && result.structural_type == structural_type
                    && result.multiplicity == StructuralMultiplicity::Affine
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            })
    };
    if !exact {
        return None;
    }
    let StructuralTypeShape::Reference {
        referent,
        access: terminal_psi::StructuralAccess::MutableBorrow,
    } = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)?
        .shape
    else {
        return None;
    };
    module
        .structural_types
        .iter()
        .any(|declaration| {
            declaration.id == referent
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        })
        .then_some((referent, suffix))
}

/// Canonical formation retains an existing owned record's producer and type.
/// It does not prove establishment or dominance; receiving verification checks
/// those independently before a store can mutate the home.
fn owned_record_store_carrier(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let StructuralPlaceKind::OperationResult {
        producer,
        structural_type,
    } = machine
        .structural_places
        .iter()
        .find(|place| place.id == destination)?
        .kind
    else {
        return None;
    };
    let mut producers = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == producer);
    let result = producers.next()?.result.structural()?;
    (producers.next().is_none()
        && result.place == destination
        && result.structural_type == structural_type
        && result.multiplicity == StructuralMultiplicity::Unrestricted
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && module.structural_types.iter().any(|declaration| {
            declaration.id == structural_type
                && matches!(declaration.shape, StructuralTypeShape::Record { .. })
        }))
    .then_some(structural_type)
}

pub(super) fn validate_establish_reference(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishReference { source } = &operation.kind else {
        unreachable!("dispatched validate_establish_reference")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("reference establishment requires a structural result");
    };
    let Some(StructuralTypeShape::Reference { access, .. }) = module
        .structural_types
        .iter()
        .find(|row| row.id == result.structural_type)
        .map(|row| &row.shape)
    else {
        return malformed("reference establishment requires a reference carrier type");
    };
    let Some(source_type) = structural_place_type(machine, source.place) else {
        return malformed("reference establishment source is unknown");
    };
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || *access != source.access
        || source.access == terminal_psi::StructuralAccess::Owned
        || !terminal_semantics::reference_source_compatible(
            module.structural_types.iter(),
            validate_structural_path(module, source_type, &source.path)?,
            result.structural_type,
        )
    {
        return malformed("reference establishment has inconsistent custody");
    }
    Ok(())
}

pub(super) fn validate_release_reference(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ReleaseReference { source } = &operation.kind else {
        unreachable!("dispatched validate_release_reference")
    };
    if operation.result != OperationResult::Unit
        || !structural_place_type(machine, *source).is_some_and(|source_type| {
            module.structural_types.iter().any(|row| {
                row.id == source_type && matches!(row.shape, StructuralTypeShape::Reference { .. })
            })
        })
    {
        return malformed("reference release requires a reference carrier and Unit result");
    }
    Ok(())
}

/// Canonical storage shape shared by scalar and descriptor field replacement.
/// Exact result/place linkage is checked by foundation validation; current
/// authority and selector bounds are independently replayed by the verifier.
fn field_store_parent(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Result<semantic_vocabulary::StructuralTypeId, CodecError> {
    let reference = reference_store_carrier(module, machine, destination, path);
    let destination_type = if let Some((referent, _)) = reference {
        referent
    } else if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == destination)
    {
        if !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return malformed("structural field store has invalid destination custody");
        }
        parameter.structural_type
    } else {
        let Some(result) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|producer| producer.result.structural())
            .find(|result| result.place == destination)
        else {
            return malformed("structural field store destination has no record home");
        };
        if !matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return malformed("structural field store has invalid local custody");
        }
        // Foundation validation also checks the exact producer/place binding;
        // ordered availability and whole affine liveness belong to verification.
        result.structural_type
    };
    let carrier_path = reference.map_or(path, |(_, suffix)| suffix);
    if !terminal_psi::is_structural_scalar_store_path(carrier_path)
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == destination)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == destination)
    {
        return malformed("structural field store has invalid destination custody");
    }
    validate_structural_path(module, destination_type, carrier_path)
}

/// Canonical shape only: a Unit store of a shared borrowed-view field below a
/// current declared storage carrier, sourced from a declared literal place. Lifetime authority, the literal's single
/// establishment and predicate implication remain verifier judgments.
pub(super) fn validate_structural_byte_view_field_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StructuralByteViewFieldStore {
        destination,
        path,
        field,
        source,
        ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_structural_byte_view_field_store")
    };
    if operation.result != OperationResult::Unit || destination == source {
        return malformed("byte view field store requires Unit and a distinct source");
    }
    let parent_type = field_store_parent(module, machine, *destination, path)?;
    let view_field = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)
        .is_some_and(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().any(|candidate| {
                candidate.id == *field
                    && !candidate.relevance.is_erased()
                    && candidate.field_type.byte_sequence_carrier()
                        == Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                        })
            }),
            _ => false,
        });
    if !view_field {
        return malformed("byte view field store does not select a shared view field");
    }
    // Encoding authenticates the descriptor carrier only. The independent
    // verifier reconstructs availability, readable access and backing lifetime.
    if !structural_place_type(machine, *source).is_some_and(|source_type| {
        module.structural_types.iter().any(|declaration| {
            declaration.id == source_type && declaration.shape.is_borrowed_byte_view()
        })
    }) {
        return malformed("byte view field store source has no descriptor carrier");
    }
    Ok(())
}

pub(super) fn validate_structural_byte_sequence_field_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StructuralByteSequenceFieldStore {
        destination,
        path,
        field,
        source,
        length,
        ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_structural_byte_sequence_field_store")
    };
    if operation.result != OperationResult::Unit {
        return malformed("byte field store requires Unit");
    }
    let reference = reference_store_carrier(module, machine, *destination, path);
    let (destination_type, carrier_path) = if let Some(reference) = reference {
        reference
    } else if let Some(structural_type) = owned_record_store_carrier(module, machine, *destination)
    {
        (structural_type, path.as_slice())
    } else {
        let Some(parameter) = terminal_semantics::arrival_parameter(machine, *destination) else {
            return malformed("byte field store destination is not arrival storage");
        };
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return malformed("byte field store has invalid destination custody");
        }
        (parameter.structural_type, path.as_slice())
    };
    if !is_bounded_structural_scalar_store_path(carrier_path)
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == *destination || claim.input == *source)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == *destination || claim.input.root == *source)
    {
        return malformed("byte field store has invalid destination custody");
    }
    let parent_type = validate_structural_path(module, destination_type, carrier_path)?;
    let bounded_field = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)
        .is_some_and(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().any(|candidate| {
                candidate.id == *field
                    && !candidate.relevance.is_erased()
                    && matches!(
                        candidate.field_type.byte_sequence_carrier(),
                        Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
                    )
            }),
            _ => false,
        });
    if !bounded_field {
        return malformed("byte field store does not select a bounded byte field");
    }
    let Some(source_place) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *source)
    else {
        return malformed("byte field store source is unknown");
    };
    let mut owned_source = false;
    let source_type = match source_place.kind {
        StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } => structural_type,
        StructuralPlaceKind::Parameter { position, is_self } => {
            let Some(parameter) = machine.structural_parameters.iter().find(|parameter| {
                parameter.place == *source
                    && parameter.position == position
                    && parameter.is_self == is_self
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.projected_qualifications.is_empty()
                    && ((parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                        && parameter.qualifications.is_empty())
                        || (parameter.access == terminal_psi::StructuralAccess::Owned
                            && terminal_semantics::byte_parameter_predicates(module, parameter)
                                .is_some()))
            }) else {
                return malformed("byte field store source is not an immutable whole view");
            };
            // Ordinary owned byte arguments retain their declared predicates
            // and live length. The carrier check below distinguishes bounded
            // ownership from a borrowed descriptor; receiving verification
            // still proves availability, destination predicates and capacity.
            owned_source = parameter.access == terminal_psi::StructuralAccess::Owned;
            parameter.structural_type
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            let Some(parameter) = machine
                .blocks
                .iter()
                .find(|candidate| candidate.id == block)
                .and_then(|block| block.structural_parameters.get(position as usize))
                .filter(|parameter| {
                    parameter.place == *source
                        && parameter.position == position
                        && !parameter.is_self
                        && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty()
                })
            else {
                return malformed("byte field store block source is not an immutable whole view");
            };
            parameter.structural_type
        }
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let mut producers = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|operation| operation.id == producer);
            let Some(producer) = producers.next() else {
                return malformed("byte field store source has no producer");
            };
            // Normal completion imports the exact declared byte owner, not a
            // borrowed descriptor. Receiving verification still checks callee
            // contents, predicate preservation, capacity and dominance.
            let owned_call =
                terminal_semantics::byte_owned_call_result(module, machine, *source).is_some();
            if producers.next().is_some()
                || !(owned_call
                    || matches!(
                        producer.kind,
                        OperationKind::ByteSequenceSubslice { .. }
                            | OperationKind::EstablishByteSequenceConcatenation { .. }
                    ))
                || !producer.result.structural().is_some_and(|result| {
                    result.place == *source
                        && result.structural_type == structural_type
                        && result.multiplicity == StructuralMultiplicity::Unrestricted
                        && (owned_call || result.qualifications.is_empty())
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                })
            {
                return malformed("byte field store source has inexact producer custody");
            }
            owned_source = owned_call
                || matches!(
                    producer.kind,
                    OperationKind::EstablishByteSequenceConcatenation { .. }
                );
            structural_type
        }
        _ => return malformed("byte field store source is not an immutable whole view"),
    };
    if !module.structural_types.iter().any(|declaration| declaration.id == source_type
        && match declaration.shape {
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView { .. }) => !owned_source,
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }) => owned_source,
            _ => false,
        })
        || !machine.blocks.iter().flat_map(|block| &block.operations).any(|candidate| {
            matches!(candidate.kind, OperationKind::ByteSequenceLength { source: measured } if measured == *source)
                && candidate.result.scalar_ref().is_some_and(|result| result.id == *length
                    && matches!(result.scalar_type, ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64))
        })
    {
        return malformed("byte field store source or exact length is invalid");
    }
    Ok(())
}

pub(super) fn validate_structural_byte_sequence_field_length(
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StructuralByteSequenceFieldLength { .. } = &operation.kind else {
        unreachable!("dispatched validate_structural_byte_sequence_field_length")
    };
    if operation.result.scalar().is_none_or(|result| {
        !matches!(result.scalar_type, ScalarType::Integer(integer)
            if integer.sign() == IntegerSign::Unsigned && integer.bits() == 64)
    }) {
        return malformed("byte field length requires an unsigned 64-bit scalar result");
    }
    // Independent module validation checks the exact bounded field,
    // borrowed parameter custody, and absence of live claims.
    Ok(())
}

pub(super) fn validate_structural_byte_sequence_field_read(
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StructuralByteSequenceFieldRead { .. } = &operation.kind else {
        unreachable!("dispatched validate_structural_byte_sequence_field_read")
    };
    let expected = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"),
    );
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return malformed("byte field read requires an unsigned 8-bit scalar result");
    }
    // Independent module validation reconstructs exact current length
    // provenance, scalar operand types, custody, and index bounds.
    Ok(())
}

pub(super) fn validate_byte_sequence_subslice(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ByteSequenceSubslice { .. } = &operation.kind else {
        unreachable!("dispatched validate_byte_sequence_subslice")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("byte-sequence subslice requires a structural result");
    };
    if !matches!(
        result.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !module.structural_types.iter().any(|row| {
            row.id == result.structural_type
                && matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
                )
        }) || !machine.structural_places.iter().any(|row| {
        row.id == result.place
            && matches!(row.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == operation.id && structural_type == result.structural_type)
    }) {
        return malformed("byte-sequence subslice requires its exact borrowed result place");
    }
    // Shape admission does not grant access. Independent module validation
    // reconstructs the source loan and forbids a shared-to-mutable result.
    Ok(())
}

/// The structural type one place carries, read through the machine's declared
/// parameter rows and produced-place table.
fn place_structural_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    if let Some(parameter) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    machine.structural_places.iter().find_map(|row| {
        if row.id != place {
            return None;
        }
        match row.kind {
            StructuralPlaceKind::OperationResult {
                structural_type, ..
            }
            | StructuralPlaceKind::ByteSequenceLiteral {
                structural_type, ..
            } => Some(structural_type),
            _ => None,
        }
    })
}

/// The shared result-place shape for element-view producers: an exact
/// unqualified `ElementView` structural result owned by this operation,
/// spelling the custody multiplicity its loan class declares (`Unrestricted`
/// for shared views, `Affine` for the exclusive mutable loan).
fn element_view_result_place(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    result: &terminal_psi::StructuralOperationResult,
    multiplicity: StructuralMultiplicity,
) -> bool {
    result.multiplicity == multiplicity
        && result.qualifications.is_empty()
        && result.projected_qualifications.is_empty()
        && result.claims.is_empty()
        && module.structural_types.iter().any(|row| {
            row.id == result.structural_type
                && (matches!(row.shape, StructuralTypeShape::ElementView { .. })
                    // A `&[u8]` view is the same establishment over `u8`
                    // elements; the byte carrier keeps no element field.
                    || row.shape.is_borrowed_byte_view())
        }) && machine.structural_places.iter().any(|row| {
        row.id == result.place
            && matches!(row.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                    if producer == operation.id && structural_type == result.structural_type)
    })
}

pub(super) fn validate_establish_element_view(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishElementView { .. } = &operation.kind else {
        unreachable!("dispatched validate_establish_element_view")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("element-view establishment requires a structural result");
    };
    if !element_view_result_place(
        module,
        machine,
        operation,
        result,
        StructuralMultiplicity::Unrestricted,
    ) {
        return malformed("element-view establishment requires its exact borrowed result place");
    }
    // Full module validation independently checks source access, the
    // collection's declared element type, custody, and establishment order.
    Ok(())
}

pub(super) fn validate_establish_mutable_element_view(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishMutableElementView { .. } = &operation.kind else {
        unreachable!("dispatched validate_establish_mutable_element_view")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("mutable element-view establishment requires a structural result");
    };
    if !element_view_result_place(
        module,
        machine,
        operation,
        result,
        StructuralMultiplicity::Affine,
    ) {
        return malformed(
            "mutable element-view establishment requires its exact borrowed result place",
        );
    }
    // Full module validation independently checks the mutable source access,
    // the collection's declared element type, custody, and establishment
    // order.
    Ok(())
}

pub(super) fn validate_element_view_subslice(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ElementViewSubslice { .. } = &operation.kind else {
        unreachable!("dispatched validate_element_view_subslice")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("element-view subslice requires a structural result");
    };
    if !matches!(
        result.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !element_view_result_place(module, machine, operation, result, result.multiplicity)
    {
        return malformed("element-view subslice requires its exact borrowed result place");
    }
    // Mutable range holders are affine, as mutable whole views are. Their
    // source authority, bounds and lifetime remain independent verifier work.
    Ok(())
}

pub(super) fn validate_element_view_length(operation: &Operation) -> Result<(), CodecError> {
    let OperationKind::ElementViewLength { .. } = &operation.kind else {
        unreachable!("dispatched validate_element_view_length")
    };
    let expected = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
    );
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return malformed("element-view length requires an unsigned 64-bit scalar result");
    }
    // Full module validation independently checks source custody and the
    // result's exact extent provenance.
    Ok(())
}

pub(super) fn validate_element_view_read(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ElementViewRead { source, path, .. } = &operation.kind else {
        unreachable!("dispatched validate_element_view_read")
    };
    let Some(source_type) = place_structural_type(machine, *source) else {
        return malformed("element-view read has an undeclared source place");
    };
    let Some(element) =
        module
            .structural_types
            .iter()
            .find_map(|row| match (row.id == source_type, &row.shape) {
                (true, StructuralTypeShape::ElementView { element }) => Some(*element),
                _ => None,
            })
    else {
        return malformed("element-view read requires an element-view source");
    };
    let Some(expected) =
        terminal_semantics::element_view_leaf_scalar(&module.structural_types, element, path)
    else {
        return malformed("element-view read requires a scalar element leaf");
    };
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return malformed("element-view read requires the element leaf's scalar result");
    }
    // Full module validation independently checks length provenance, custody,
    // operand types and dominance before encoding or after decoding.
    Ok(())
}

pub(super) fn validate_structural_case_membership(operation: &Operation) -> Result<(), CodecError> {
    let OperationKind::StructuralCaseMembership { .. } = &operation.kind else {
        unreachable!("dispatched validate_structural_case_membership")
    };
    if operation.result.scalar().is_none_or(|result| {
        result.scalar_type != ScalarType::Boolean || !result.qualifications.is_empty()
    }) {
        return malformed("case membership requires an unqualified Boolean result");
    }
    // Full module validation independently checks the source's nominal
    // owner, readable access, establishment order and live custody.
    Ok(())
}

pub(super) fn validate_structural_leaf_copy(
    module: &TerminalModule,
    operation: &Operation,
) -> Result<(), CodecError> {
    let is_case_copy = matches!(
        &operation.kind,
        OperationKind::StructuralCaseLeafCopy { .. }
    );
    let (OperationKind::StructuralLeafCopy { .. } | OperationKind::StructuralCaseLeafCopy { .. }) =
        &operation.kind
    else {
        unreachable!("dispatched validate_structural_leaf_copy")
    };
    if operation.result.structural().is_none_or(|result| {
        // A whole byte snapshot retains primitive declaration qualifications;
        // it does not establish a new domain or change a projected leaf.
        let qualified_byte_copy = matches!(&operation.kind,
            OperationKind::StructuralLeafCopy { path, .. } if path.is_empty())
            && result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.qualification_establishments.is_empty()
            && module.structural_types.iter().any(|declaration| {
                declaration.id == result.structural_type
                    && matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
                    )
            })
            && terminal_semantics::byte_qualification_predicates_in(
                module.structural_domains.iter(),
                result.structural_type,
                &result.qualifications,
            )
            .is_some();
        (result.multiplicity != StructuralMultiplicity::Unrestricted
            // A case-payload copy may instead mint the transfer's `Affine`
            // result shape: a `SharedBorrow` reference descriptor relocating
            // a proven payload's loan, or a plain leaf's owned copy the
            // successor takes custody of.
            && !(is_case_copy
                && result.multiplicity == StructuralMultiplicity::Affine
                && module.structural_types.iter().any(|declaration| {
                    declaration.id == result.structural_type
                })))
            || (!result.qualifications.is_empty() && !qualified_byte_copy)
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
    }) {
        return malformed("leaf copy has invalid multiplicity or qualification custody");
    }
    // Full module validation independently checks the source's readable
    // access, the resolved leaf type, custody and liveness.
    Ok(())
}

pub(super) fn validate_byte_sequence_read(operation: &Operation) -> Result<(), CodecError> {
    let OperationKind::ByteSequenceRead { .. } = &operation.kind else {
        unreachable!("dispatched validate_byte_sequence_read")
    };
    let expected = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"),
    );
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return malformed("byte-sequence read requires an unsigned 8-bit scalar result");
    }
    // Full module validation checks direct length provenance, custody,
    // operand types and dominance before encoding or after decoding.
    Ok(())
}

pub(super) fn validate_byte_sequence_length(operation: &Operation) -> Result<(), CodecError> {
    let OperationKind::ByteSequenceLength { .. } = &operation.kind else {
        unreachable!("dispatched validate_byte_sequence_length")
    };
    let expected = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
    );
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return malformed("byte-sequence length requires an unsigned 64-bit scalar result");
    }
    // The independent module verifier checks exact source custody and
    // literal establishment before encode or after decode.
    Ok(())
}

pub(super) fn validate_write_only_primitive_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::WriteOnlyPrimitiveStore {
        destination,
        value,
        path,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_write_only_primitive_store")
    };
    if operation.result != OperationResult::Unit {
        return malformed("write-only primitive store declares a non-Unit result");
    }
    // Entry and block parameters carry the same storage authority. A path
    // selects inside an aggregate; it does not select a different parameter
    // lookup or permit an undeclared destination.
    let projected = !path.is_empty();
    let destination_type = if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == *destination)
    {
        let declaration_matches = machine.structural_places.iter().any(|place| {
            if place.id != *destination {
                return false;
            }
            match place.kind {
                StructuralPlaceKind::Parameter { position, is_self } => {
                    position == parameter.position
                        && is_self == parameter.is_self
                        && machine.structural_parameters.contains(parameter)
                }
                StructuralPlaceKind::BlockParameter { block, position } => {
                    position == parameter.position
                        && !parameter.is_self
                        && machine
                            .blocks
                            .iter()
                            .find(|candidate| candidate.id == block)
                            .and_then(|block| block.structural_parameters.get(position as usize))
                            == Some(parameter)
                }
                _ => false,
            }
        });
        let writable = matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || (projected
            && parameter.access == terminal_psi::StructuralAccess::Owned);
        let multiplicity = parameter.multiplicity == StructuralMultiplicity::Unrestricted
            || (projected && parameter.multiplicity == StructuralMultiplicity::Affine);
        if !writable
            || !multiplicity
            || !declaration_matches
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == *destination)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination)
        {
            return malformed("write-only primitive store has invalid destination custody");
        }
        parameter.structural_type
    } else {
        let Some(result) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|producer| {
                projected || matches!(producer.kind, OperationKind::EstablishPrimitiveLocal { .. })
            })
            .filter_map(|producer| producer.result.structural())
            .find(|result| {
                result.place == *destination
                    && result.claims.is_empty()
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && (result.multiplicity == StructuralMultiplicity::Unrestricted
                        || (projected && result.multiplicity == StructuralMultiplicity::Affine))
            })
        else {
            return malformed(
                "primitive store destination is neither a parameter nor an initialized local",
            );
        };
        // Each producer validates its exact operation-result declaration;
        // independent verification checks dominance and reference custody.
        if machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == *destination)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination)
        {
            return malformed("primitive local store cannot carry entry claims");
        }
        result.structural_type
    };
    let Some(expected) = terminal_semantics::primitive_projection_type(
        module.structural_types.iter(),
        destination_type,
        path,
    ) else {
        return malformed("write-only primitive store requires a primitive-scalar root");
    };
    let actual = machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|candidate| candidate.result.scalar_ref())
        }))
        .find(|declaration| declaration.id == *value)
        .map(|declaration| declaration.scalar_type);
    if actual != Some(expected) {
        return malformed("write-only primitive store value type does not match referent");
    }
    Ok(())
}

pub(super) fn validate_structural_scalar_field_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StructuralScalarFieldStore {
        destination,
        path,
        field,
        value,
        range_obligation,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_structural_scalar_field_store")
    };
    if operation.result != OperationResult::Unit {
        return malformed("structural scalar field store declares a non-Unit result");
    }
    let parent_type = field_store_parent(module, machine, *destination, path)?;
    let Some(expected) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)
        .and_then(|declaration| match &declaration.shape {
            // A mixed shape's common fields are direct fields at one offset
            // in every case; payload fields stay out of reach of this store.
            StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
                fields.iter().find_map(|candidate| {
                    (candidate.id == *field && !candidate.relevance.is_erased())
                        .then_some(&candidate.field_type)
                        .and_then(|field_type| match field_type {
                            StructuralFieldType::Scalar(scalar_type)
                                if range_obligation.is_none() =>
                            {
                                Some(*scalar_type)
                            }
                            field_type
                                if terminal_semantics::scalar_field_requires_evidence(
                                    field_type,
                                ) && range_obligation.is_some() =>
                            {
                                field_type.scalar_type()
                            }
                            StructuralFieldType::IeeeFloat(format)
                                if range_obligation.is_none() =>
                            {
                                Some(semantic_vocabulary::ScalarType::IeeeFloat(*format))
                            }
                            _ => None,
                        })
                })
            }
            _ => None,
        })
    else {
        return malformed("structural scalar field store does not select a relevant scalar field");
    };
    let actual = machine
        .parameters
        .iter()
        .chain(machine.result.scalar_ref())
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .chain(machine.blocks.iter().flat_map(|block| {
            block
                .operations
                .iter()
                .filter_map(|candidate| candidate.result.scalar_ref())
        }))
        .find(|declaration| declaration.id == *value)
        .map(|declaration| declaration.scalar_type);
    if actual != Some(expected) {
        return malformed("structural scalar field store value type does not match field");
    }
    Ok(())
}

/// Borrowed-storage restoration window moves: `MoveStructuralField` opens the
/// window on one declared structural field beneath a mutable-borrowed root and
/// `StoreStructuralField` reseats exactly that field. Foundation validation
/// proves the field's declared type and the result/stored value's declared type
/// agree and that the root carries mutable-borrow authority; the window's
/// opening, exact repair, and non-crash-exit closure belong to ordered
/// verification, and the moved subtree's custody transfer belongs to the
/// interpreter.
pub(super) fn validate_move_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::MoveStructuralField {
        source,
        path,
        field,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_move_structural_field")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("structural field move has no structural result");
    };
    if !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return malformed("structural field move result carries attached custody");
    }
    let Some(root_type) = window_root_type(machine, *source, path, *field) else {
        return malformed("structural field move requires a mutable-borrowed parameter");
    };
    let field_type = window_field_type(module, root_type, path, *field)?;
    if result.structural_type != field_type {
        return malformed("structural field move result type does not match field");
    }
    Ok(())
}

pub(super) fn validate_store_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StoreStructuralField {
        destination,
        path,
        field,
        value,
    } = &operation.kind
    else {
        unreachable!("dispatched validate_store_structural_field")
    };
    if operation.result != OperationResult::Unit {
        return malformed("structural field store declares a non-Unit result");
    }
    if value.access != terminal_psi::StructuralAccess::Owned || !value.path.is_empty() {
        return malformed("structural field store value must be a whole owned place");
    }
    let Some(root_type) = window_root_type(machine, *destination, path, *field) else {
        return malformed("structural field store requires a mutable-borrowed parameter");
    };
    let field_type = window_field_type(module, root_type, path, *field)?;
    if structural_place_type(machine, value.place) != Some(field_type) {
        return malformed("structural field store value type does not match field");
    }
    Ok(())
}

/// The declared root type a window operation may name. A field window may
/// open beneath a mutable-borrowed parameter or a completed owned home; a
/// whole-root window needs the original exclusive parameter loan. A reference
/// crossing without a following child selector still names a whole referent.
fn window_root_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if !terminal_semantics::structural_window_selects_child(path, field) {
        if !path.is_empty() {
            return None;
        }
        mutable_borrowed_parameter_type(machine, place)
    } else {
        borrowed_field_store_root_type(machine, place)
    }
}

/// The declared type of the root storage a window operation may name: either
/// a declared mutable-borrowed machine/block parameter or a completed owned
/// home. Block bindings retain the same declared storage shape; reconstructing
/// their incoming authority, alias interference, and exact restoration debt
/// belongs to ordered verification, not this serialization foundation check.
fn borrowed_field_store_root_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if let Some(structural_type) = mutable_borrowed_parameter_type(machine, place) {
        return Some(structural_type);
    }
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|producer| {
            matches!(
                producer.kind,
                OperationKind::EstablishStructuralArray { .. }
                    | OperationKind::EstablishRecord { .. }
                    | OperationKind::EstablishZeroedStructural { .. }
                    | OperationKind::EstablishStructuralLocal { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
                    | OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StructuralCaseLeafCopy { .. }
            )
        })
        .filter_map(|producer| producer.result.structural())
        .find(|result| {
            result.place == place
                && matches!(
                    result.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                )
                && result.qualifications.is_empty()
                && result.projected_qualifications.is_empty()
                && result.claims.is_empty()
        })
        .map(|result| result.structural_type)
}

/// A declared mutable-borrowed machine or block parameter with transferable
/// multiplicity and no qualifications or entry claims.
fn mutable_borrowed_parameter_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    machine
        .structural_parameters
        .iter()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| {
            parameter.place == place
                && parameter.access == terminal_psi::StructuralAccess::MutableBorrow
                && matches!(
                    parameter.multiplicity,
                    StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                )
                && parameter.qualifications.is_empty()
                && parameter.projected_qualifications.is_empty()
                && !machine
                    .entry_claims
                    .iter()
                    .any(|claim| claim.input == place)
                && !machine
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == place)
        })
        .map(|parameter| parameter.structural_type)
}

/// Resolve the same exact storage projection the operation encodes. Bounds
/// evidence and restoration custody remain independent verifier obligations.
fn window_field_type(
    module: &TerminalModule,
    root_type: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Result<semantic_vocabulary::StructuralTypeId, CodecError> {
    let Some(projection) = terminal_semantics::structural_window_projection(
        module.structural_types.iter(),
        root_type,
        path,
        field,
    ) else {
        return malformed("structural field move does not select a structural field");
    };
    Ok(projection.structural_type)
}

pub(super) fn validate_integer_structural_field(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let (OperationKind::IntegerStructuralField {
        source,
        path,
        field,
    }
    | OperationKind::BooleanStructuralField {
        source,
        path,
        field,
    }) = &operation.kind
    else {
        unreachable!("dispatched validate_integer_structural_field")
    };
    let Some(result) = operation.result.scalar_ref() else {
        return malformed("scalar structural field has no scalar result");
    };
    if !matches!(
        (&operation.kind, result.scalar_type),
        (
            OperationKind::IntegerStructuralField { .. },
            ScalarType::Integer(_)
        ) | (
            OperationKind::BooleanStructuralField { .. },
            ScalarType::Boolean
        )
    ) {
        return malformed("scalar structural field has an invalid result type");
    }
    // A constructed/call-result record remains a local result, not a
    // synthetic parameter. The verifier separately checks its producer,
    // dominance, live ownership and loans at this observation.
    if let Some((producer, local)) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|producer| {
            matches!(
                producer.kind,
                OperationKind::EstablishStructuralArray { .. }
                    | OperationKind::EstablishRecord { .. }
                    | OperationKind::EstablishZeroedStructural { .. }
                    | OperationKind::EstablishStructuralLocal { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
                    | OperationKind::StructuralLeafCopy { .. }
                    | OperationKind::StructuralCaseLeafCopy { .. }
                    // A loan carrier is equally a readable observation source:
                    // `&cells[i]` mints its referent storage through the
                    // reference establishment, and a view local reads its
                    // element fields through the view descriptor the same way
                    // the store arm admits it as a write destination.
                    | OperationKind::EstablishReference { .. }
                    | OperationKind::EstablishElementView { .. }
                    | OperationKind::EstablishMutableElementView { .. }
            )
        })
        .filter_map(|producer| producer.result.structural().map(|local| (producer, local)))
        .find(|(_, local)| local.place == *source)
    {
        // The observed field belongs to a record carrier or to the payload of
        // the case a final `Case` step selects; which case is active is the
        // verifier's reconstruction judgment, not a decode-time shape.
        // A call result's `Reference` shell keeps the loan's access on the
        // type; the observed record is its referent, matching the peel the
        // validating verifier applies for the same read.
        let (observed_root, access) = match module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == local.structural_type)
            .map(|declaration| &declaration.shape)
        {
            Some(StructuralTypeShape::Reference { referent, access }) => (*referent, *access),
            _ => (
                local.structural_type,
                match &producer.kind {
                    OperationKind::EstablishReference { source }
                    | OperationKind::EstablishElementView { source, .. }
                    | OperationKind::EstablishMutableElementView { source, .. } => source.access,
                    _ => terminal_psi::StructuralAccess::Owned,
                },
            ),
        };
        let matching = terminal_semantics::observed_field_declaration(
            module.structural_types.iter(),
            observed_root,
            path,
            *field,
        )
        .is_some_and(|candidate| {
            candidate.field_type.scalar_type() == Some(result.scalar_type)
                && scalar_observation_authorized(
                    module,
                    observed_root,
                    path,
                    &candidate.field_type,
                    access,
                )
        });
        if !matching
            || local.multiplicity == StructuralMultiplicity::Linear
            || !local.qualifications.is_empty()
            || !local.projected_qualifications.is_empty()
            || !local.claims.is_empty()
        {
            return malformed("scalar record field has invalid local result custody");
        }
        return Ok(());
    }
    let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *source)
        .or_else(|| {
            let declaration = machine
                .structural_places
                .iter()
                .find(|place| place.id == *source)?;
            let StructuralPlaceKind::BlockParameter { block, position } = declaration.kind else {
                return None;
            };
            machine
                .blocks
                .iter()
                .find(|candidate| candidate.id == block)?
                .structural_parameters
                .get(position as usize)
                .filter(|parameter| {
                    parameter.place == *source
                        && parameter.position == position
                        && !parameter.is_self
                })
        })
    else {
        return malformed("scalar structural field source is not a parameter");
    };
    let matching = terminal_semantics::observed_field_declaration(
        module.structural_types.iter(),
        parameter.structural_type,
        path,
        *field,
    )
    .filter(|candidate| match &candidate.field_type {
        StructuralFieldType::Scalar(scalar_type) => *scalar_type == result.scalar_type,
        StructuralFieldType::Atomic(scalar_type) => {
            *scalar_type == result.scalar_type
                && scalar_observation_authorized(
                    module,
                    parameter.structural_type,
                    path,
                    &candidate.field_type,
                    parameter.access,
                )
        }
        StructuralFieldType::BoundedInteger(bounded) => {
            ScalarType::Integer(bounded.integer_type()) == result.scalar_type
        }
        StructuralFieldType::RefinedScalar(refinement) => {
            refinement.carrier() == result.scalar_type
        }
        _ => false,
    });
    if !matches!(
        parameter.multiplicity,
        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
    ) || !matches!(
        parameter.access,
        terminal_psi::StructuralAccess::Owned
            | terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow
    ) || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == *source)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == *source)
        || matching.is_none()
    {
        return malformed("scalar structural field has invalid source custody");
    }
    Ok(())
}

/// Scalar representation alone grants no permission to snapshot a shared cell.
fn scalar_observation_authorized(
    module: &TerminalModule,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[semantic_vocabulary::CanonicalStructuralPathSegment],
    field: &StructuralFieldType,
    access: terminal_psi::StructuralAccess,
) -> bool {
    !matches!(field, StructuralFieldType::Atomic(_))
        || matches!(
            terminal_semantics::observed_field_backing_access(
                module.structural_types.iter(),
                root,
                path,
                access,
            ),
            Some(
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
            )
        )
}

pub(super) fn validate_store_dynamic_descriptor(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::StoreDynamicDescriptor { descriptor_ordinal } = &operation.kind else {
        unreachable!("dispatched validate_store_dynamic_descriptor")
    };
    if operation.result != OperationResult::Unit
        || !module
            .dynamic_dispatch
            .stored_descriptors
            .iter()
            .any(|descriptor| {
                descriptor.owner == machine.id
                    && descriptor.ordinal == *descriptor_ordinal
                    && descriptor.establishment_operation == operation.id
            })
    {
        return malformed("dynamic descriptor storage has no exact catalog row");
    }
    Ok(())
}
