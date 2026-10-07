//! Operations that read or write retained storage: references, byte-sequence
//! fields and views, primitive and structural scalar stores, field reads and
//! case membership.

use super::{structural_place_type, validate_structural_path};
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
