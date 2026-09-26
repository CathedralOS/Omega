//! Evidence terms and evidence contract lanes on the wire.

use super::super::CodecError;
use super::super::proof_declaration_wire::{decode_evidence_interface, encode_evidence_interface};
use super::super::wire::{Reader, Writer};
use terminal_psi::{EvidenceContractLane, EvidenceContractLaneKind, EvidenceTermDeclaration};

pub(super) fn encode_evidence_term(
    writer: &mut Writer,
    term: &EvidenceTermDeclaration,
) -> Result<(), CodecError> {
    writer.id(term.id);
    writer.id(term.proposition);
    encode_evidence_interface(writer, &term.interface)?;
    Ok(())
}

pub(super) fn encode_evidence_contract_lane(
    writer: &mut Writer,
    lane: &EvidenceContractLane,
) -> Result<(), CodecError> {
    writer.id(lane.machine);
    writer.u8(match lane.kind {
        EvidenceContractLaneKind::Requires => 1,
        EvidenceContractLaneKind::Ensures => 2,
    });
    writer.u32(lane.position);
    writer.id(lane.term);
    writer.boolean(lane.output_field.is_some());
    if let Some(field) = &lane.output_field {
        writer.string("evidence output field", field)?;
    }
    Ok(())
}

pub(super) fn decode_evidence_term(
    reader: &mut Reader<'_>,
) -> Result<EvidenceTermDeclaration, CodecError> {
    Ok(EvidenceTermDeclaration {
        id: reader.id("EvidenceTermId")?,
        proposition: reader.id("PropositionId")?,
        interface: decode_evidence_interface(reader)?,
    })
}

pub(super) fn decode_evidence_contract_lane(
    reader: &mut Reader<'_>,
) -> Result<EvidenceContractLane, CodecError> {
    let machine = reader.id("MachineId")?;
    let kind = match reader.u8()? {
        1 => EvidenceContractLaneKind::Requires,
        2 => EvidenceContractLaneKind::Ensures,
        tag => return Err(CodecError::InvalidTag("EvidenceContractLaneKind", tag)),
    };
    Ok(EvidenceContractLane {
        machine,
        kind,
        position: reader.u32()?,
        term: reader.id("EvidenceTermId")?,
        output_field: reader
            .boolean()?
            .then(|| reader.string("evidence output field"))
            .transpose()?,
    })
}
