//! Evidence terms and evidence contract lanes on the wire.

use super::super::CodecError;
use super::super::proof_declaration_wire::{decode_evidence_interface, encode_evidence_interface};
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::structural_place_wire::{
    decode_structural_path, encode_structural_path,
};
use crate::sections::semantic_module::wire::decode_counted;
use terminal_psi::{
    EvidenceArgumentSubject, EvidenceContractLane, EvidenceContractLaneKind,
    EvidenceTermDeclaration,
};

pub(super) fn encode_evidence_term(
    writer: &mut Writer,
    term: &EvidenceTermDeclaration,
) -> Result<(), CodecError> {
    writer.id(term.id);
    writer.id(term.proposition);
    encode_evidence_interface(writer, &term.interface)
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
    writer.len("evidence argument subjects", lane.argument_subjects.len())?;
    for subject in &lane.argument_subjects {
        match subject {
            EvidenceArgumentSubject::Parameter(parameter) => {
                writer.u8(1);
                writer.u32(parameter.argument_position);
                writer.u32(parameter.parameter_position);
                writer.boolean(parameter.structural_path.is_some());
                if let Some(path) = &parameter.structural_path {
                    encode_structural_path(writer, "evidence argument subject path", path)?;
                }
            }
            EvidenceArgumentSubject::Result { argument_position } => {
                writer.u8(2);
                writer.u32(*argument_position);
            }
            EvidenceArgumentSubject::Closed { argument_position } => {
                writer.u8(3);
                writer.u32(*argument_position);
            }
            EvidenceArgumentSubject::Unknown { argument_position } => {
                writer.u8(4);
                writer.u32(*argument_position);
            }
        }
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
        argument_subjects: decode_counted(reader, |reader| {
            let kind = reader.u8()?;
            let argument_position = reader.u32()?;
            Ok(match kind {
                1 => EvidenceArgumentSubject::Parameter(
                    terminal_psi::OutcomeSpecificCallArgumentSubstitution {
                        argument_position,
                        parameter_position: reader.u32()?,
                        structural_path: reader
                            .boolean()?
                            .then(|| decode_structural_path(reader))
                            .transpose()?,
                    },
                ),
                2 => EvidenceArgumentSubject::Result { argument_position },
                3 => EvidenceArgumentSubject::Closed { argument_position },
                4 => EvidenceArgumentSubject::Unknown { argument_position },
                tag => return Err(CodecError::InvalidTag("EvidenceArgumentSubject", tag)),
            })
        })?,
    })
}
