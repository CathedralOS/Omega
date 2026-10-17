//! Operations that read or write retained storage: references, byte-sequence
//! fields and views, primitive and structural scalar stores, field reads and
//! case membership.

use super::{declared_scalar_type, structural_place_type, validate_structural_path};
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::{IntegerSign, ScalarType, StructuralPlaceKind};

mod byte_sequences;
mod element_views;
mod structural_fields;
pub(super) use byte_sequences::*;
pub(super) use element_views::*;
pub(super) use structural_fields::*;
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralFieldType, StructuralMultiplicity,
    StructuralTypeShape, TerminalMachine, TerminalModule, is_static_structural_scalar_store_path,
};

/// Resolve only the reference shell's declared geometry. Origin, live loan
/// authority and the referent's owned-content restrictions remain independent
/// verifier judgments; canonical encoding does not grant a storage permission.
fn reference_store_carrier<'path>(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: semantic_vocabulary::PlaceId,
    path: &'path [terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Option<(
    semantic_vocabulary::StructuralTypeId,
    &'path [terminal_psi::StructuralPathSegment],
)> {
    let crossing = path
        .iter()
        .rposition(|segment| *segment == terminal_psi::StructuralPathSegment::Referent)?;
    let suffix = &path[crossing + 1..];
    let structural_type = structural_place_type(machine, destination)?;
    let member = terminal_semantics::projected_field_declaration(
        module.structural_types.iter(),
        structural_type,
        path,
        field,
    )?;
    let mut written = path.to_vec();
    written.push(terminal_psi::StructuralPathSegment::Field(member.identity));
    let preserves = |whole: &[_], projected: &[_]| {
        terminal_semantics::structural_memberships_disjoint_from_write(whole, projected, &written)
    };
    let (multiplicity, access) = if let Some(parameter) = machine.structural_parameter(destination)
    {
        if !preserves(
            &parameter.qualifications,
            &parameter.projected_qualifications,
        ) {
            return None;
        }
        (parameter.multiplicity, parameter.access)
    } else {
        let result = machine
            .operations()
            .filter_map(|operation| operation.result.structural())
            .find(|result| {
                result.place == destination
                    && result.structural_type == structural_type
                    && preserves(&result.qualifications, &result.projected_qualifications)
                    && result.claims.is_empty()
            })?;
        (result.multiplicity, terminal_psi::StructuralAccess::Owned)
    };
    // Descriptor ownership and referent access are different contracts. A
    // borrowed aggregate is unrestricted but can carry an exclusive reference.
    // Check the declared access along every crossing, not whether the root
    // happened to be produced as an affine owned reference value.
    if !terminal_semantics::reference_source_projection_accessible(
        module.structural_types.iter(),
        structural_type,
        multiplicity,
        access,
        path,
        terminal_psi::StructuralAccess::WriteOnlyBorrow,
    ) {
        return None;
    }
    let StructuralTypeShape::Reference {
        referent,
        access:
            terminal_psi::StructuralAccess::MutableBorrow
            | terminal_psi::StructuralAccess::WriteOnlyBorrow,
    } = module
        .structural_type(
            validate_structural_path(module, structural_type, &path[..crossing]).ok()?,
        )?
        .shape
    else {
        return None;
    };
    module
        .structural_types
        .iter()
        .any(|declaration| declaration.id == referent)
        .then_some((referent, suffix))
}

/// Canonical formation retains an existing owned home's producer and type.
/// It does not prove establishment or dominance; receiving verification checks
/// those independently before a store can mutate the home. Shape does not grant
/// mutation: every reference crossing retains its own access ceiling.
fn owned_store_carrier(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: semantic_vocabulary::PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<semantic_vocabulary::StructuralTypeId> {
    let (structural_type, multiplicity) =
        if let Some(parameter) = terminal_semantics::arrival_parameter(machine, destination) {
            if parameter.access != terminal_psi::StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return None;
            }
            (parameter.structural_type, parameter.multiplicity)
        } else {
            let StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } = machine.structural_place(destination)?.kind
            else {
                return None;
            };
            let mut producers = machine
                .operations()
                .filter(|operation| operation.id == producer);
            let result = producers.next()?.result.structural()?;
            if producers.next().is_some()
                || result.place != destination
                || result.structural_type != structural_type
            {
                return None;
            }
            (structural_type, result.multiplicity)
        };
    // This authenticates the home and declared access, not copy/disposal
    // permission. Field replacement preserves its owner's custody and
    // predicates; receiving verification checks the selected write.
    terminal_semantics::reference_source_projection_accessible(
        module.structural_types.iter(),
        structural_type,
        multiplicity,
        terminal_psi::StructuralAccess::Owned,
        path,
        terminal_psi::StructuralAccess::WriteOnlyBorrow,
    )
    .then_some(structural_type)
}

/// Shared length-result check: the operation must produce an unsigned
/// 64-bit scalar; source custody and extent provenance stay with full
/// module validation.
pub(super) fn validate_u64_length_result(
    operation: &Operation,
    malformed_label: &'static str,
) -> Result<(), CodecError> {
    let expected = ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid"),
    );
    if operation
        .result
        .scalar()
        .is_none_or(|result| result.scalar_type != expected)
    {
        return malformed(malformed_label);
    }
    Ok(())
}

pub(super) fn validate_establish_placed_reference(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishPlacedReference { source } = &operation.kind else {
        unreachable!("dispatched validate_establish_placed_reference")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("placed establishment requires a structural reference result");
    };
    let Some(source_type) = structural_place_type(machine, source.place) else {
        return malformed("placed establishment source is unknown");
    };
    let source_type = validate_structural_path(module, source_type, &source.path)?;
    if result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return malformed("placed establishment cannot introduce owned content or qualifications");
    }
    if let Err(reason) = terminal_semantics::placed_reference_correspondence(
        module,
        source_type,
        result.structural_type,
        source.access,
    ) {
        return Err(CodecError::PlacedRamCorrespondence(reason));
    }
    Ok(())
}

pub(super) fn validate_establish_reference(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Result<(), CodecError> {
    let OperationKind::EstablishReference { source, storage } = &operation.kind else {
        unreachable!("dispatched validate_establish_reference")
    };
    let Some(result) = operation.result.structural() else {
        return malformed("reference establishment requires a structural result");
    };
    let Some(StructuralTypeShape::Reference { access, .. }) = module
        .structural_type(result.structural_type)
        .map(|row| &row.shape)
    else {
        return malformed("reference establishment requires a reference carrier type");
    };
    let Some(source_type) = structural_place_type(machine, source.place) else {
        return malformed("reference establishment source is unknown");
    };
    let source_path = terminal_semantics::reference_source_storage_path(
        module.structural_types.iter(),
        source_type,
        &source.path,
    );
    let inline_owner = storage.is_none()
        && terminal_semantics::runtime_structural_field(
            module.structural_types.iter(),
            source_type,
            source_path,
        )
        .is_some_and(|field| {
            matches!(
                field.field_type.byte_sequence_carrier(),
                Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
            )
        });
    let field_compatible = inline_owner
        && terminal_semantics::reference_byte_field_compatible(
            module.structural_types.iter(),
            module.structural_domains.iter(),
            source_type,
            source_path,
            result.structural_type,
            &result.projected_qualifications,
        );
    if let Some(storage) = storage {
        let offset_type = machine
            .parameters
            .iter()
            .chain(machine.result.scalar_ref())
            .chain(machine.block_parameters())
            .chain(
                machine
                    .operations()
                    .filter_map(|candidate| candidate.result.scalar_ref()),
            )
            .find(|value| value.id == storage.byte_offset)
            .map(|value| value.scalar_type);
        let offset_is_u64 = matches!(offset_type, Some(semantic_vocabulary::ScalarType::Integer(integer))
            if !integer.is_address() && integer.bits() == 64
                && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned);
        if !offset_is_u64 {
            return malformed("reference storage offset is not a u64 scalar");
        }
    }
    let compatible = match storage {
        Some(storage) => {
            // An explicit storage interpretation still observes a shared
            // restricted scalar through its read carrier. The stored codec,
            // offset and receiving interpretation remain independent checks.
            let observed = (*access == terminal_psi::StructuralAccess::SharedBorrow)
                .then(|| {
                    terminal_semantics::borrowed_primitive_projection_declaration(
                        module.structural_types.iter(),
                        source_type,
                        source_path,
                        None,
                    )
                })
                .flatten();
            let source_type = match observed {
                Some(observed) => observed,
                None => validate_structural_path(module, source_type, source_path)?,
            };
            terminal_semantics::reference_storage_source_compatible(
                module.structural_types.iter(),
                source_type,
                result.structural_type,
                storage.stored_scalar,
            )
        }
        None => terminal_semantics::reference_source_projection_compatible(
            module.structural_types.iter(),
            source_type,
            source_path,
            result.structural_type,
        ),
    };
    // Inline owner predicates belong to the field, not the root's nominal
    // membership roster. Carrier compatibility cannot replace their handback
    // obligation, including when the result advertises no qualifications.
    let ancestors_preserved = *access == terminal_psi::StructuralAccess::SharedBorrow
        || structural_fields::declared_memberships(machine, source.place).is_some_and(
            |(whole, projected)| {
                terminal_semantics::reference_preserves_ancestor_qualifications(
                    module.structural_types.iter(),
                    module.structural_domains.iter(),
                    &module.identities,
                    whole,
                    projected,
                    source_path,
                    *access,
                ) && terminal_semantics::store_preserves_content(
                    module.structural_types.iter(),
                    module.structural_domains.iter(),
                    &module.identities,
                    source_type,
                    source.place,
                    machine
                        .content_entry_claims
                        .iter()
                        .filter(|claim| claim.input.segments.len() < source_path.len()),
                    source_path,
                )
            },
        );
    let memberships_match = if inline_owner {
        field_compatible
    } else {
        structural_fields::declared_memberships(machine, source.place).is_some_and(
            |(whole, projected)| {
                let Ok(mut captured) = terminal_semantics::reference_source_memberships(
                    module.structural_types.iter(),
                    source_type,
                    whole,
                    projected,
                    source_path,
                    *access,
                ) else {
                    return false;
                };
                if *access == terminal_psi::StructuralAccess::SharedBorrow
                    && result
                        .projected_qualifications
                        .iter()
                        .any(|row| !captured.contains(row))
                {
                    let position = machine.blocks.iter().find_map(|block| {
                        block
                            .operations
                            .iter()
                            .position(|candidate| candidate.id == operation.id)
                            .and_then(|ordinal| {
                                u32::try_from(ordinal)
                                    .ok()
                                    .map(|ordinal| (block.id, ordinal))
                            })
                    });
                    let Some(position) = position else {
                        return false;
                    };
                    let graph = terminal_semantics::descriptor_results::terminal_graph(module);
                    let Ok(current) =
                        terminal_semantics::descriptor_results::current_selected_payload_contract(
                            &module.structural_types,
                            &module.structural_domains,
                            &graph,
                            machine.id,
                            position,
                            source.place,
                            source_path,
                            &|place| {
                                terminal_semantics::descriptor_results::terminal_payload_contract(
                                    machine, place,
                                )
                            },
                        )
                    else {
                        return false;
                    };
                    terminal_psi::StructuralPathQualification::append_under(
                        &current.qualifications,
                        &current.projected_qualifications,
                        &[terminal_psi::StructuralPathSegment::Referent],
                        &mut captured,
                    );
                }
                result.projected_qualifications.iter().all(|row| captured.contains(row))
                // Mutable formation retains preservation obligations, not just
                // optional observations. Erasing a row cannot erase a debt.
                && (*access == terminal_psi::StructuralAccess::SharedBorrow
                    || captured.iter().all(|row| result.projected_qualifications.contains(row)))
            },
        )
    };
    if !ancestors_preserved
        || result.multiplicity != StructuralMultiplicity::Affine
        || !result.qualifications.is_empty()
        || ((inline_owner
            || !result.projected_qualifications.is_empty()
            || *access != terminal_psi::StructuralAccess::SharedBorrow)
            && !memberships_match)
        || !result.claims.is_empty()
        || *access != source.access
        || source.access == terminal_psi::StructuralAccess::Owned
        || !compatible
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
    let OperationKind::ReleaseReference { source, path } = &operation.kind else {
        unreachable!("dispatched validate_release_reference")
    };
    let Some(source_type) = structural_place_type(machine, *source) else {
        return malformed("reference release source is absent");
    };
    if operation.result != OperationResult::Unit
        || path.iter().any(|segment| {
            !matches!(
                segment,
                terminal_psi::StructuralPathSegment::Field(_)
                    | terminal_psi::StructuralPathSegment::FixedIndex(_)
            )
        })
    {
        return malformed("reference release requires an owned descriptor path and Unit result");
    }
    let leaf_type = validate_structural_path(module, source_type, path)?;
    if !module.structural_types.iter().any(|row| {
        row.id == leaf_type && matches!(row.shape, StructuralTypeShape::Reference { .. })
    }) {
        return malformed("reference release requires a reference carrier");
    }
    Ok(())
}

/// Canonical storage shape shared by scalar and descriptor field replacement.
/// Exact result/place linkage is checked by foundation validation; current
/// authority and selector bounds are independently replayed by the verifier.
fn field_store_carrier<'path>(
    module: &TerminalModule,
    machine: &TerminalMachine,
    destination: semantic_vocabulary::PlaceId,
    path: &'path [terminal_psi::StructuralPathSegment],
    field: semantic_vocabulary::StructuralFieldId,
) -> Result<
    (
        semantic_vocabulary::StructuralTypeId,
        &'path [terminal_psi::StructuralPathSegment],
    ),
    CodecError,
> {
    let preserves = |root, whole: &[_], projected: &[_]| {
        terminal_semantics::scalar_field_store_preserves_qualifications(
            module.structural_types.iter(),
            module.structural_domains.iter(),
            &module.identities,
            root,
            whole,
            projected,
            path,
            field,
        )
    };
    let reference = reference_store_carrier(module, machine, destination, path, field);
    let destination_type = if let Some((referent, _)) = reference {
        referent
    } else if let Some(parameter) = machine.structural_parameter(destination) {
        if !matches!(
            parameter.access,
            terminal_psi::StructuralAccess::Owned
                | terminal_psi::StructuralAccess::MutableBorrow
                | terminal_psi::StructuralAccess::WriteOnlyBorrow
        ) || !preserves(
            parameter.structural_type,
            &parameter.qualifications,
            &parameter.projected_qualifications,
        ) {
            return malformed("structural field store cannot preserve destination custody");
        }
        parameter.structural_type
    } else {
        let Some(result) = machine
            .operations()
            .filter_map(|producer| producer.result.structural())
            .find(|result| result.place == destination)
        else {
            return malformed("structural field store destination has no record home");
        };
        if !preserves(
            result.structural_type,
            &result.qualifications,
            &result.projected_qualifications,
        ) {
            return malformed("structural field store has invalid local custody");
        }
        // Foundation validation also checks the exact producer/place binding;
        // ordered availability and regional claim liveness belong to execution.
        // A scalar write preserves resource claims; their presence on the
        // original producer does not make the containing storage immutable.
        result.structural_type
    };
    // Content claims and the full write path name the original storage root.
    // The referent carrier below is only the scalar emission geometry; using
    // it here would cross a retained Referent segment a second time.
    let Some(content_root) = structural_place_type(machine, destination) else {
        return malformed("structural field store has no declared content root");
    };
    if !terminal_semantics::scalar_field_store_preserves_content(
        module.structural_types.iter(),
        module.structural_domains.iter(),
        &module.identities,
        content_root,
        destination,
        machine.content_entry_claims.iter(),
        path,
        field,
    ) {
        return malformed("structural scalar field store changes conserved content");
    }
    let carrier_path = reference.map_or(path, |(_, suffix)| suffix);
    if !terminal_psi::is_structural_scalar_store_path(carrier_path) {
        return malformed("structural field store has invalid destination custody");
    }
    // The leaf resolver validates this complete path. A final Case is the
    // sum's field namespace, not an independently materialized carrier type.
    Ok((destination_type, carrier_path))
}

/// The structural type one place carries, read through the machine's declared
/// parameter rows and produced-place table.
fn place_structural_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if let Some(parameter) = machine.signature_structural_parameter(place) {
        return Some(parameter.structural_type);
    }
    if let Some(parameter) = machine
        .block_structural_parameters()
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
