use super::*;

/// Canonical shape only: a Unit store of a shared borrowed-view slot -- a
/// field below a current declared storage carrier, or, without a field and
/// with an empty path, a whole declared place of shared view type -- sourced
/// from a declared descriptor place. Destination authority, lifetime
/// authority, the source's establishment and predicate implication remain
/// verifier judgments.
pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_structural_byte_view_field_store(
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
    let shared_view = terminal_psi::ByteSequenceCarrier::BorrowedView {
        access: Some(terminal_psi::StructuralAccess::SharedBorrow),
    };
    let view_slot = match field {
        Some(field) => {
            let (root_type, carrier_path) =
                field_store_carrier(module, machine, *destination, path, *field)?;
            terminal_semantics::projected_field_declaration(
                module.structural_types.iter(),
                root_type,
                carrier_path,
                *field,
            )
            .is_some_and(|candidate| {
                candidate.field_type.byte_sequence_carrier() == Some(shared_view)
            })
        }
        None => {
            path.is_empty()
                && structural_place_type(machine, *destination).is_some_and(|home_type| {
                    module.structural_types.iter().any(|declaration| {
                        declaration.id == home_type
                            && matches!(
                                declaration.shape,
                                StructuralTypeShape::ByteSequence(
                                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                                        access: None
                                            | Some(terminal_psi::StructuralAccess::SharedBorrow),
                                    }
                                )
                            )
                    })
                })
        }
    };
    if !view_slot {
        return malformed("byte view field store does not select a shared view slot");
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

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_structural_byte_sequence_field_store(
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
    let reference = reference_store_carrier(module, machine, *destination, path, *field);
    let (destination_type, carrier_path) = if let Some(reference) = reference {
        reference
    } else if let Some(structural_type) = owned_store_carrier(module, machine, *destination, path) {
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
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return malformed("byte field store has invalid destination custody");
        }
        (parameter.structural_type, path.as_slice())
    };
    if !is_static_structural_scalar_store_path(carrier_path)
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
        .structural_type(parent_type)
        .is_some_and(|declaration| match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => fields.iter().any(|candidate| {
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
    let Some(source_place) = machine.structural_place(*source) else {
        return malformed("byte field store source is unknown");
    };
    let mut owned_source = false;
    let source_type = match source_place.kind {
        StructuralPlaceKind::ByteSequenceLiteral {
            structural_type, ..
        } => structural_type,
        StructuralPlaceKind::Parameter { .. } | StructuralPlaceKind::BlockParameter { .. } => {
            // A state parameter is the same arrival storage as a machine
            // parameter. Declaration location grants no additional permission:
            // both retain exact access, predicates, capacity and live length.
            let Some(parameter) =
                terminal_semantics::arrival_parameter(machine, *source).filter(|parameter| {
                    parameter.multiplicity == StructuralMultiplicity::Unrestricted
                        && parameter.projected_qualifications.is_empty()
                        && ((parameter.access == terminal_psi::StructuralAccess::SharedBorrow
                            && parameter.qualifications.is_empty())
                            || (parameter.access == terminal_psi::StructuralAccess::Owned
                                && terminal_semantics::byte_parameter_predicates(
                                    module, parameter,
                                )
                                .is_some()))
                })
            else {
                return malformed("byte field store source is not a readable whole byte carrier");
            };
            owned_source = parameter.access == terminal_psi::StructuralAccess::Owned;
            parameter.structural_type
        }
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            let mut producers = machine
                .operations()
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
        || !machine.operations().any(|candidate| {
            matches!(&candidate.kind, OperationKind::ByteSequenceLength { source: measured } if measured.place == *source && measured.path.is_empty())
                && candidate.result.scalar_ref().is_some_and(|result| result.id == *length
                    && matches!(result.scalar_type, ScalarType::Integer(integer) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && integer.bits() == 64))
        })
    {
        return malformed("byte field store source or exact length is invalid");
    }
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_structural_byte_sequence_field_length(
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

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_structural_byte_sequence_field_read(
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

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_byte_sequence_subslice(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ByteSequenceSubslice { source, .. } = &operation.kind else {
        unreachable!("dispatched validate_byte_sequence_subslice")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("byte-sequence subslice requires a structural result");
    };
    if !matches!(
        result.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
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
    validate_byte_descriptor_source(module, machine, source, true)?;
    let root = place_structural_type(machine, source.place).expect("source checked above");
    let selected = validate_structural_path(module, root, &source.path)?;
    if selected != result.structural_type
        || (result.multiplicity == StructuralMultiplicity::Affine
            && !matches!(
                source.access,
                terminal_psi::StructuralAccess::Owned
                    | terminal_psi::StructuralAccess::MutableBorrow
            ))
    {
        return malformed("byte subslice must preserve its selected descriptor and access");
    }
    // Shape admission does not grant access. Independent module validation
    // reconstructs the source loan and forbids a shared-to-mutable result.
    Ok(())
}

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_byte_sequence_read(
    operation: &Operation,
) -> Result<(), CodecError> {
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

pub(in crate::sections::semantic_module::module_foundation_validation) fn validate_byte_sequence_length(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::ByteSequenceLength { source } = &operation.kind else {
        unreachable!("dispatched validate_byte_sequence_length")
    };
    super::validate_u64_length_result(
        operation,
        "byte-sequence length requires an unsigned 64-bit scalar result",
    )?;
    validate_byte_descriptor_source(module, machine, source, false)?;
    Ok(())
}

/// Validate the selected byte carrier, not its descriptor word count. Bounded
/// loans carry static extents; live access and bounds are independently
/// reconstructed at the Terminal trust boundary.
fn validate_byte_descriptor_source(
    module: &TerminalModule,
    machine: &TerminalMachine,
    source: &terminal_psi::StructuralArgument,
    borrowed_only: bool,
) -> Result<(), CodecError> {
    if source.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return malformed("byte descriptor requires readable access");
    }
    let Some(root) = place_structural_type(machine, source.place) else {
        return malformed("byte descriptor has no source declaration");
    };
    let selected = validate_structural_path(module, root, &source.path)?;
    if !module.structural_type(selected).is_some_and(|declaration| {
        declaration.shape.is_borrowed_byte_carrier()
            || (!borrowed_only
                && (declaration.shape.is_bounded_owned_byte_sequence()
                    || matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::FixedArray { .. }
                    ) && terminal_semantics::fixed_byte_extent(
                        module.structural_types.iter(),
                        selected,
                    )
                    .is_some()))
    }) {
        return malformed("byte operation requires its exact selected byte descriptor");
    }
    Ok(())
}
