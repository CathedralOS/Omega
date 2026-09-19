//! Operations that read or write retained storage: references, byte-sequence
//! fields and views, primitive and structural scalar stores, field reads,
//! case membership and dynamic descriptors.

use super::{structural_place_type, validate_structural_path};
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::{IntegerSign, ScalarType, StructuralPlaceKind};
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralFieldType, StructuralMultiplicity,
    StructuralPlaceDeclaration, StructuralTypeShape, TerminalMachine, TerminalModule,
    is_bounded_structural_scalar_store_path,
};

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
    let Some(StructuralTypeShape::Reference { referent, access }) = module
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
        || validate_structural_path(module, source_type, &source.path)? != *referent
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
    let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *destination)
    else {
        return malformed("byte field store destination is not a parameter");
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
        || !is_bounded_structural_scalar_store_path(path)
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
    let parent_type = validate_structural_path(module, parameter.structural_type, path)?;
    let bounded_field = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)
        .is_some_and(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().any(|candidate| {
                candidate.id == *field
                    && !candidate.relevance.is_erased()
                    && matches!(
                        candidate.field_type,
                        StructuralFieldType::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                        )
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
    let source_type = match source_place.kind {
        StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } => structural_type,
        StructuralPlaceKind::Parameter { position, is_self } => {
            let Some(parameter) = machine.structural_parameters.iter().find(|parameter| {
                parameter.place == *source
                    && parameter.position == position
                    && parameter.is_self == is_self
                    && parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && parameter.qualifications.is_empty()
                    && parameter.projected_qualifications.is_empty()
            }) else {
                return malformed("byte field store source is not an immutable whole view");
            };
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
                return malformed("byte field store subslice source has no producer");
            };
            if producers.next().is_some()
                || !matches!(producer.kind, OperationKind::ByteSequenceSubslice { .. })
                || !producer.result.structural().is_some_and(|result| {
                    result.place == *source
                        && result.structural_type == structural_type
                        && result.multiplicity == StructuralMultiplicity::Unrestricted
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                })
            {
                return malformed("byte field store subslice source has inexact producer custody");
            }
            structural_type
        }
        _ => return malformed("byte field store source is not an immutable whole view"),
    };
    if !module.structural_types.iter().any(|declaration| declaration.id == source_type
        && matches!(declaration.shape, StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView)))
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
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !module.structural_types.iter().any(|row| {
            row.id == result.structural_type
                && matches!(
                    row.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView
                    )
                )
        }) || !machine.structural_places.iter().any(|row| {
        row.id == result.place
            && matches!(row.kind, StructuralPlaceKind::OperationResult { producer, structural_type }
                if producer == operation.id && structural_type == result.structural_type)
    }) {
        return malformed(
            "byte-sequence subslice requires its exact immutable borrowed result place",
        );
    }
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
    let destination_type = if !path.is_empty() {
        if let Some(parameter) = machine
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
            if !matches!(
                parameter.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
                    | terminal_psi::StructuralAccess::WriteOnlyBorrow
            ) || !matches!(
                parameter.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            ) || !parameter.qualifications.is_empty()
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
                return malformed("projected primitive store has invalid destination custody");
            }
            parameter.structural_type
        } else {
            let Some(result) = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|producer| producer.result.structural())
                .find(|result| {
                    result.place == *destination
                        && result.claims.is_empty()
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && matches!(
                            result.multiplicity,
                            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                        )
                })
            else {
                return malformed("projected primitive store has no owned root");
            };
            // Exact producer, liveness and reference custody belong to independent verification.
            result.structural_type
        }
    } else if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == *destination)
    {
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || parameter.multiplicity != StructuralMultiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || machine
                .entry_claims
                .iter()
                .any(|claim| claim.input == *destination)
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == *destination)
            || !matches!(
                machine.structural_places.iter().find(|place| place.id == *destination),
                Some(StructuralPlaceDeclaration {
                    kind: StructuralPlaceKind::Parameter { position, is_self },
                    ..
                }) if *position == parameter.position && *is_self == parameter.is_self
            )
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
                matches!(producer.kind, OperationKind::EstablishPrimitiveLocal { .. })
            })
            .filter_map(|producer| producer.result.structural())
            .find(|result| result.place == *destination)
        else {
            return malformed(
                "primitive store destination is neither a parameter nor an initialized local",
            );
        };
        // Each establishment is independently checked against its exact
        // operation-result declaration below; the verifier checks dominance.
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
    let Some(expected) = terminal_semantics::primitive_place_type(
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

/// The runtime index is always a projection of the destination root, so this
/// mirrors the projected branch of `validate_write_only_primitive_store`:
/// whole-root replacement through `EstablishPrimitiveLocal` can never name an
/// array. The verifier independently reconstructs custody, dominance, and the
/// `index < declared extent` obligation; this pass checks wire-level shape.
pub(super) fn validate_write_only_indexed_primitive_store(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::WriteOnlyIndexedPrimitiveStore {
        destination,
        path,
        index,
        value,
        ..
    } = &operation.kind
    else {
        unreachable!("dispatched validate_write_only_indexed_primitive_store")
    };
    if operation.result != OperationResult::Unit {
        return malformed("write-only indexed primitive store declares a non-Unit result");
    }
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
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || !matches!(
            parameter.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !parameter.qualifications.is_empty()
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
            return malformed("indexed primitive store has invalid destination custody");
        }
        parameter.structural_type
    } else {
        let Some(result) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|producer| producer.result.structural())
            .find(|result| {
                result.place == *destination
                    && result.claims.is_empty()
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && matches!(
                        result.multiplicity,
                        StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
                    )
            })
        else {
            return malformed("indexed primitive store has no writable root");
        };
        result.structural_type
    };
    let Some((element_type, _)) = terminal_semantics::fixed_array_place_shape(
        module.structural_types.iter(),
        destination_type,
        path,
    ) else {
        return malformed("indexed primitive store requires a fixed-array destination path");
    };
    let declared_scalar = |value| {
        machine
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
            .find(|declaration| declaration.id == value)
            .map(|declaration| declaration.scalar_type)
    };
    let expected_index = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
    );
    if declared_scalar(*index) != Some(expected_index) {
        return malformed("indexed primitive store requires an unsigned 64-bit index");
    }
    if declared_scalar(*value) != Some(element_type) {
        return malformed("indexed primitive store value type does not match the element");
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
            return malformed("structural scalar field store has invalid destination custody");
        }
        parameter.structural_type
    } else {
        let Some(result) = machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|producer| {
                matches!(
                    producer.kind,
                    OperationKind::EstablishRecord { .. }
                        | OperationKind::CallStructural { .. }
                        | OperationKind::CallStructuralWithScalarArguments { .. }
                )
            })
            .filter_map(|producer| producer.result.structural())
            .find(|result| result.place == *destination)
        else {
            return malformed("structural scalar field store destination has no record home");
        };
        if !matches!(
            result.multiplicity,
            StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
        ) || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
        {
            return malformed("structural scalar field store has invalid local custody");
        }
        // Foundation validation also checks the exact producer/place binding;
        // ordered availability and whole affine liveness belong to verification.
        result.structural_type
    };
    if !is_bounded_structural_scalar_store_path(path)
        || machine
            .entry_claims
            .iter()
            .any(|claim| claim.input == *destination)
        || machine
            .content_entry_claims
            .iter()
            .any(|claim| claim.input.root == *destination)
    {
        return malformed("structural scalar field store has invalid destination custody");
    }
    let parent_type = validate_structural_path(module, destination_type, path)?;
    let Some(expected) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)
        .and_then(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().find_map(|candidate| {
                (candidate.id == *field && !candidate.relevance.is_erased())
                    .then_some(&candidate.field_type)
                    .and_then(|field_type| match field_type {
                        StructuralFieldType::Scalar(scalar_type) if range_obligation.is_none() => {
                            Some(*scalar_type)
                        }
                        StructuralFieldType::BoundedInteger(bounds)
                            if range_obligation.is_some() =>
                        {
                            Some(semantic_vocabulary::ScalarType::Integer(
                                bounds.integer_type(),
                            ))
                        }
                        StructuralFieldType::IeeeFloat(format) if range_obligation.is_none() => {
                            Some(semantic_vocabulary::ScalarType::IeeeFloat(*format))
                        }
                        _ => None,
                    })
            }),
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
    let Some(parameter) = borrowed_field_store_root(machine, *source) else {
        return malformed("structural field move requires a mutable-borrowed parameter");
    };
    let parent_type = validate_structural_path(module, parameter.structural_type, path)?;
    let Some(field_type) = declared_structural_field_type(module, parent_type, *field) else {
        return malformed("structural field move does not select a structural field");
    };
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
    let Some(parameter) = borrowed_field_store_root(machine, *destination) else {
        return malformed("structural field store requires a mutable-borrowed parameter");
    };
    let parent_type = validate_structural_path(module, parameter.structural_type, path)?;
    let Some(field_type) = declared_structural_field_type(module, parent_type, *field) else {
        return malformed("structural field store does not select a structural field");
    };
    if structural_place_type(machine, value.place) != Some(field_type) {
        return malformed("structural field store value type does not match field");
    }
    Ok(())
}

fn borrowed_field_store_root(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<&terminal_psi::StructuralParameterDeclaration> {
    machine.structural_parameters.iter().find(|parameter| {
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
}

fn declared_structural_field_type(
    module: &TerminalModule,
    parent_type: semantic_vocabulary::StructuralTypeId,
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parent_type)
        .and_then(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } => {
                fields.iter().find_map(|candidate| {
                    (candidate.id == field && !candidate.relevance.is_erased())
                        .then_some(&candidate.field_type)
                        .and_then(|field_type| match field_type {
                            StructuralFieldType::Structural(structural_type) => {
                                Some(*structural_type)
                            }
                            _ => None,
                        })
                })
            }
            _ => None,
        })
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
    if let Some(local) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|producer| {
            matches!(
                producer.kind,
                OperationKind::EstablishRecord { .. }
                    | OperationKind::CallStructural { .. }
                    | OperationKind::CallStructuralWithScalarArguments { .. }
            )
        })
        .filter_map(|producer| producer.result.structural())
        .find(|local| local.place == *source)
    {
        let carrier = terminal_semantics::record_field_carrier(
            module.structural_types.iter(),
            local.structural_type,
            path,
        );
        let matching = carrier.and_then(|carrier| module.structural_types.iter().find(|declaration|
            declaration.id == carrier.structural_type)).is_some_and(|declaration|
                matches!(&declaration.shape, StructuralTypeShape::Record { fields }
                    if fields.iter().any(|candidate| candidate.id == *field && !candidate.relevance.is_erased()
                            && candidate.field_type.scalar_type() == Some(result.scalar_type))));
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
    let carrier = terminal_semantics::record_field_carrier(
        module.structural_types.iter(),
        parameter.structural_type,
        path,
    );
    let matching = carrier
        .and_then(|carrier| {
            module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == carrier.structural_type)
        })
        .and_then(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields } => fields.iter().find(|candidate| {
                candidate.id == *field
                    && !candidate.relevance.is_erased()
                    && match candidate.field_type {
                        StructuralFieldType::Scalar(scalar_type) => {
                            scalar_type == result.scalar_type
                        }
                        StructuralFieldType::BoundedInteger(bounded) => {
                            ScalarType::Integer(bounded.integer_type()) == result.scalar_type
                        }
                        _ => false,
                    }
            }),
            _ => None,
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
