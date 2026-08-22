//! Canonical terminal-machine envelope wire format.
//!
//! This module owns machine identity, parameters/results, structural places,
//! entry/content claims, service ceilings, ordered block envelopes, and the
//! retained contract. Block bodies and their operation vocabulary remain in
//! the parent codec.

use psi_terminal::{
    EntryClaim, StructuralMultiplicity, StructuralPlaceDeclaration, StructuralResultDeclaration,
    TerminalMachine, TerminalMachineResult, ValueDeclaration,
};

use super::content_wire::{
    decode_content_entry_claim, decode_content_identity_reshuffle,
    decode_content_partition_composition, encode_content_entry_claim,
    encode_content_identity_reshuffle, encode_content_partition_composition,
};
use super::contract_wire::{decode_contract, encode_contract};
use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::structural_signature_wire::{
    decode_structural_parameters, encode_service_ceiling, encode_structural_parameters,
};
use super::wire::{Reader, Writer};
use super::{
    CodecError, decode_block, decode_counted, decode_ids, decode_optional_id,
    decode_structural_path, decode_structural_place_kind, encode_block, encode_optional_id,
    encode_structural_path, encode_structural_place_kind,
};

pub(super) fn encode_machine(
    writer: &mut Writer,
    machine: &TerminalMachine,
) -> Result<(), CodecError> {
    writer.id(machine.id);
    encode_optional_id(writer, machine.attachment);
    encode_declarations(writer, "machine parameters", &machine.parameters)?;
    encode_structural_parameters(writer, &machine.structural_parameters)?;
    match &machine.result {
        TerminalMachineResult::Unit => writer.u8(0),
        TerminalMachineResult::Scalar(result) => {
            writer.u8(1);
            encode_declaration(writer, *result);
        }
        TerminalMachineResult::Structural(result) => {
            writer.u8(2);
            writer.id(result.place);
            writer.id(result.structural_type);
            writer.u8(match result.multiplicity {
                StructuralMultiplicity::Unrestricted => 1,
                StructuralMultiplicity::Affine => 2,
                StructuralMultiplicity::Linear => 3,
            });
            writer.len(
                "structural result qualifications",
                result.qualifications.len(),
            )?;
            for qualification in &result.qualifications {
                writer.id(*qualification);
            }
        }
    }
    writer.len("structural places", machine.structural_places.len())?;
    for place in &machine.structural_places {
        writer.id(place.id);
        encode_structural_place_kind(writer, place.kind);
    }
    writer.len("entry claims", machine.entry_claims.len())?;
    for claim in &machine.entry_claims {
        writer.id(claim.claim);
        writer.id(claim.input);
        encode_structural_path(writer, "entry claim path", &claim.path)?;
    }
    encode_service_ceiling(writer, &machine.published_service_ceiling)?;
    writer.len("content entry claims", machine.content_entry_claims.len())?;
    for binding in &machine.content_entry_claims {
        encode_content_entry_claim(writer, binding)?;
    }
    writer.len(
        "content identity reshuffles",
        machine.content_identity_reshuffles.len(),
    )?;
    for reshuffle in &machine.content_identity_reshuffles {
        encode_content_identity_reshuffle(writer, reshuffle)?;
    }
    writer.len(
        "content partition compositions",
        machine.content_partition_compositions.len(),
    )?;
    for composition in &machine.content_partition_compositions {
        encode_content_partition_composition(writer, composition)?;
    }
    writer.id(machine.entry);
    writer.len("blocks", machine.blocks.len())?;
    for block in &machine.blocks {
        encode_block(writer, block)?;
    }
    encode_contract(writer, &machine.contract)
}

pub(super) fn encode_declarations(
    writer: &mut Writer,
    label: &'static str,
    declarations: &[ValueDeclaration],
) -> Result<(), CodecError> {
    writer.len(label, declarations.len())?;
    for declaration in declarations {
        encode_declaration(writer, *declaration);
    }
    Ok(())
}

pub(super) fn encode_declaration(writer: &mut Writer, declaration: ValueDeclaration) {
    writer.id(declaration.id);
    encode_scalar_type(writer, declaration.scalar_type);
}

pub(super) fn decode_machine(reader: &mut Reader<'_>) -> Result<TerminalMachine, CodecError> {
    let id = reader.id("MachineId")?;
    let attachment = decode_optional_id(reader, "StructuralTypeId")?;
    let parameters = decode_declarations(reader)?;
    let structural_parameters = decode_structural_parameters(reader)?;
    let result = match reader.u8()? {
        0 => TerminalMachineResult::Unit,
        1 => TerminalMachineResult::Scalar(decode_declaration(reader)?),
        2 => TerminalMachineResult::Structural(StructuralResultDeclaration {
            place: reader.id("PlaceId")?,
            structural_type: reader.id("StructuralTypeId")?,
            multiplicity: match reader.u8()? {
                1 => StructuralMultiplicity::Unrestricted,
                2 => StructuralMultiplicity::Affine,
                3 => StructuralMultiplicity::Linear,
                tag => return Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
            },
            qualifications: decode_ids(reader, "StructuralDomainId")?,
        }),
        tag => return Err(CodecError::InvalidTag("TerminalMachineResult", tag)),
    };
    let count = reader.count()?;
    let mut structural_places = Vec::new();
    for _ in 0..count {
        structural_places.push(StructuralPlaceDeclaration {
            id: reader.id("PlaceId")?,
            kind: decode_structural_place_kind(reader)?,
        });
    }
    let entry_claims = decode_counted(reader, |reader| {
        Ok(EntryClaim {
            claim: reader.id("ClaimId")?,
            input: reader.id("PlaceId")?,
            path: decode_structural_path(reader)?,
        })
    })?;
    let published_service_ceiling = decode_ids(reader, "ServiceId")?;
    let count = reader.count()?;
    let mut content_entry_claims = Vec::new();
    for _ in 0..count {
        content_entry_claims.push(decode_content_entry_claim(reader)?);
    }
    let count = reader.count()?;
    let mut content_identity_reshuffles = Vec::new();
    for _ in 0..count {
        content_identity_reshuffles.push(decode_content_identity_reshuffle(reader)?);
    }
    let count = reader.count()?;
    let mut content_partition_compositions = Vec::new();
    for _ in 0..count {
        content_partition_compositions.push(decode_content_partition_composition(reader)?);
    }
    let entry = reader.id("BlockId")?;
    let block_count = reader.count()?;
    let mut blocks = Vec::new();
    for _ in 0..block_count {
        blocks.push(decode_block(reader)?);
    }
    let contract = decode_contract(reader)?;
    Ok(TerminalMachine {
        id,
        attachment,
        parameters,
        structural_parameters,
        result,
        structural_places,
        entry_claims,
        published_service_ceiling,
        content_entry_claims,
        content_identity_reshuffles,
        content_partition_compositions,
        entry,
        blocks,
        contract,
    })
}

pub(super) fn decode_declarations(
    reader: &mut Reader<'_>,
) -> Result<Vec<ValueDeclaration>, CodecError> {
    let count = reader.count()?;
    let mut declarations = Vec::new();
    for _ in 0..count {
        declarations.push(decode_declaration(reader)?);
    }
    Ok(declarations)
}

pub(super) fn decode_declaration(reader: &mut Reader<'_>) -> Result<ValueDeclaration, CodecError> {
    Ok(ValueDeclaration {
        id: reader.id("ValueId")?,
        scalar_type: decode_scalar_type(reader)?,
    })
}
