//! Canonical proof declaration and evidence-interface wire format.
//!
//! This module owns proposition binder declarations, source-handle-free
//! application identities, optional evidence projections, and evidence
//! interface requirements. Recursive propositions and proof admission remain
//! outside the codec layer.

use psi_terminal::{
    EvidenceInterfaceIdentity, EvidenceProjectionIdentity, EvidenceRequirementIdentity,
    PropositionApplicationIdentity, PropositionBinderArgumentIdentity,
    PropositionBinderArgumentKind, PropositionBinderDeclaration, PropositionBinderKind,
    PropositionDeclaration, PropositionEvidence,
};

use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted};

pub(super) fn encode_proposition_declaration(
    writer: &mut Writer,
    declaration: &PropositionDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.string("proposition name", &declaration.name)?;
    writer.len("proposition binders", declaration.binders.len())?;
    for binder in &declaration.binders {
        writer.string("proposition binder name", &binder.name)?;
        match &binder.kind {
            PropositionBinderKind::Type => writer.u8(1),
            PropositionBinderKind::Const { type_identity } => {
                writer.u8(2);
                writer.string("proposition const binder type", type_identity)?;
            }
            PropositionBinderKind::Machine => writer.u8(3),
        }
    }
    writer.len(
        "proposition parameter types",
        declaration.parameter_types.len(),
    )?;
    for parameter_type in &declaration.parameter_types {
        writer.string("proposition parameter type", parameter_type)?;
    }
    match &declaration.evidence {
        PropositionEvidence::FactOnly => writer.u8(1),
        PropositionEvidence::Witness { evidence_type } => {
            writer.u8(2);
            writer.string("proposition evidence type", evidence_type)?;
        }
    }
    Ok(())
}

pub(super) fn encode_proposition_application(
    writer: &mut Writer,
    application: &PropositionApplicationIdentity,
) -> Result<(), CodecError> {
    writer.id(application.id);
    writer.id(application.declaration);
    writer.len(
        "proposition binder arguments",
        application.binder_arguments.len(),
    )?;
    for argument in &application.binder_arguments {
        writer.u8(match argument.kind {
            PropositionBinderArgumentKind::Type => 1,
            PropositionBinderArgumentKind::Const => 2,
            PropositionBinderArgumentKind::Machine => 3,
        });
        match &argument.evidence_projection {
            None => {
                writer.u8(0);
                writer.string("proposition binder argument", &argument.identity)?;
            }
            Some(projection) => {
                writer.u8(1);
                writer.id(projection.term);
                writer.string(
                    "evidence projection declaring trait",
                    &projection.declaring_trait_identity,
                )?;
                writer.len(
                    "evidence projection declaring trait arguments",
                    projection.declaring_trait_arguments.len(),
                )?;
                for argument in &projection.declaring_trait_arguments {
                    writer.string("evidence projection declaring trait argument", argument)?;
                }
                writer.string(
                    "evidence projection requirement",
                    &projection.requirement_identity,
                )?;
            }
        }
    }
    writer.len("proposition arguments", application.arguments.len())?;
    for argument in &application.arguments {
        writer.string("proposition argument", argument)?;
    }
    match &application.evidence_interface {
        None => writer.u8(0),
        Some(interface) => {
            writer.u8(1);
            encode_evidence_interface(writer, interface)?;
        }
    }
    Ok(())
}

pub(super) fn encode_evidence_interface(
    writer: &mut Writer,
    interface: &EvidenceInterfaceIdentity,
) -> Result<(), CodecError> {
    writer.string(
        "evidence interface trait identity",
        &interface.trait_identity,
    )?;
    writer.len("evidence interface arguments", interface.arguments.len())?;
    for argument in &interface.arguments {
        writer.string("evidence interface argument", argument)?;
    }
    writer.len(
        "evidence interface requirements",
        interface.requirements.len(),
    )?;
    for requirement in &interface.requirements {
        writer.string(
            "evidence requirement declaring trait",
            &requirement.declaring_trait_identity,
        )?;
        writer.len(
            "evidence requirement declaring trait arguments",
            requirement.declaring_trait_arguments.len(),
        )?;
        for argument in &requirement.declaring_trait_arguments {
            writer.string("evidence requirement declaring trait argument", argument)?;
        }
        writer.string(
            "evidence requirement identity",
            &requirement.requirement_identity,
        )?;
    }
    Ok(())
}

pub(super) fn decode_proposition_declaration(
    reader: &mut Reader<'_>,
) -> Result<PropositionDeclaration, CodecError> {
    let id = reader.id("PropositionId")?;
    let name = reader.string("proposition name")?;
    let binder_count = reader.count()?;
    let mut binders = Vec::with_capacity(binder_count as usize);
    for _ in 0..binder_count {
        let name = reader.string("proposition binder name")?;
        let kind = match reader.u8()? {
            1 => PropositionBinderKind::Type,
            2 => PropositionBinderKind::Const {
                type_identity: reader.string("proposition const binder type")?,
            },
            3 => PropositionBinderKind::Machine,
            tag => return Err(CodecError::InvalidTag("PropositionBinderKind", tag)),
        };
        binders.push(PropositionBinderDeclaration { name, kind });
    }
    let parameter_count = reader.count()?;
    let mut parameter_types = Vec::with_capacity(parameter_count as usize);
    for _ in 0..parameter_count {
        parameter_types.push(reader.string("proposition parameter type")?);
    }
    let evidence = match reader.u8()? {
        1 => PropositionEvidence::FactOnly,
        2 => PropositionEvidence::Witness {
            evidence_type: reader.string("proposition evidence type")?,
        },
        tag => return Err(CodecError::InvalidTag("PropositionEvidence", tag)),
    };
    Ok(PropositionDeclaration {
        id,
        name,
        binders,
        parameter_types,
        evidence,
    })
}

pub(super) fn decode_proposition_application(
    reader: &mut Reader<'_>,
) -> Result<PropositionApplicationIdentity, CodecError> {
    let id = reader.id("PropositionId")?;
    let declaration = reader.id("PropositionId")?;
    let binder_count = reader.count()?;
    let mut binder_arguments = Vec::with_capacity(binder_count as usize);
    for _ in 0..binder_count {
        let kind = match reader.u8()? {
            1 => PropositionBinderArgumentKind::Type,
            2 => PropositionBinderArgumentKind::Const,
            3 => PropositionBinderArgumentKind::Machine,
            tag => {
                return Err(CodecError::InvalidTag("PropositionBinderArgumentKind", tag));
            }
        };
        let (identity, evidence_projection) = match reader.u8()? {
            0 => (reader.string("proposition binder argument")?, None),
            1 => (
                String::new(),
                Some(EvidenceProjectionIdentity {
                    term: reader.id("EvidenceTermId")?,
                    declaring_trait_identity: reader
                        .string("evidence projection declaring trait")?,
                    declaring_trait_arguments: decode_counted(reader, |reader| {
                        reader.string("evidence projection declaring trait argument")
                    })?,
                    requirement_identity: reader.string("evidence projection requirement")?,
                }),
            ),
            tag => return Err(CodecError::InvalidTag("PropositionBinderArgument", tag)),
        };
        binder_arguments.push(PropositionBinderArgumentIdentity {
            kind,
            identity,
            evidence_projection,
        });
    }
    let argument_count = reader.count()?;
    let mut arguments = Vec::with_capacity(argument_count as usize);
    for _ in 0..argument_count {
        arguments.push(reader.string("proposition argument")?);
    }
    let evidence_interface = match reader.u8()? {
        0 => None,
        1 => Some(decode_evidence_interface(reader)?),
        tag => return Err(CodecError::InvalidTag("PropositionEvidenceInterface", tag)),
    };
    Ok(PropositionApplicationIdentity {
        id,
        declaration,
        binder_arguments,
        arguments,
        evidence_interface,
    })
}

pub(super) fn decode_evidence_interface(
    reader: &mut Reader<'_>,
) -> Result<EvidenceInterfaceIdentity, CodecError> {
    Ok(EvidenceInterfaceIdentity {
        trait_identity: reader.string("evidence interface trait identity")?,
        arguments: decode_counted(reader, |reader| {
            reader.string("evidence interface argument")
        })?,
        requirements: decode_counted(reader, |reader| {
            Ok(EvidenceRequirementIdentity {
                declaring_trait_identity: reader.string("evidence requirement declaring trait")?,
                declaring_trait_arguments: decode_counted(reader, |reader| {
                    reader.string("evidence requirement declaring trait argument")
                })?,
                requirement_identity: reader.string("evidence requirement identity")?,
            })
        })?,
    })
}
