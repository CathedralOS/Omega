//! Structural domain and service declarations and installation reach
//! dependencies on the wire.

use super::super::CodecError;
use super::super::content_wire::{decode_content_algebra, encode_content_algebra};
use super::super::structural_signature_wire::{
    decode_content_projection_expression, encode_content_projection_expression,
};
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::{decode_counted, decode_ids};
use semantic_vocabulary::ContentProjectionIdentity;
use terminal_psi::{
    InstallationReachDependency, ServiceDeclaration, StructuralContentProjection,
    StructuralDomainDeclaration, StructuralEstablishmentRoute, TerminalIdentityIndex,
};

fn encode_establishment_routes(
    writer: &mut Writer,
    routes: &[StructuralEstablishmentRoute],
) -> Result<(), CodecError> {
    writer.len("domain establishment routes", routes.len())?;
    for route in routes {
        match route {
            StructuralEstablishmentRoute::Requirement { requirement } => {
                writer.u8(1);
                writer.u32(requirement.0);
            }
            StructuralEstablishmentRoute::ExactMachine { machine } => {
                writer.u8(2);
                writer.u32(machine.0);
            }
            StructuralEstablishmentRoute::BoundaryRequirement { requirement } => {
                writer.u8(3);
                writer.u32(requirement.0);
            }
        }
    }
    Ok(())
}

fn decode_establishment_routes(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralEstablishmentRoute>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(match reader.u8()? {
            1 => StructuralEstablishmentRoute::Requirement {
                requirement: TerminalIdentityIndex(reader.u32()?),
            },
            2 => StructuralEstablishmentRoute::ExactMachine {
                machine: TerminalIdentityIndex(reader.u32()?),
            },
            3 => StructuralEstablishmentRoute::BoundaryRequirement {
                requirement: TerminalIdentityIndex(reader.u32()?),
            },
            tag => return Err(CodecError::InvalidTag("StructuralEstablishmentRoute", tag)),
        })
    })
}

pub(super) fn encode_structural_domain(
    writer: &mut Writer,
    declaration: &StructuralDomainDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.id(declaration.semantic_domain);
    writer.u32(declaration.identity.0);
    writer.id(declaration.carrier);
    writer.boolean(declaration.predicate.is_some());
    if let Some(predicate) = &declaration.predicate {
        writer.id(predicate.subject);
        super::super::proposition_wire::encode_proposition(writer, &predicate.proposition, 0)?;
    }
    writer.u8(declaration
        .byte_predicate
        .map_or(0, |predicate| predicate.mask_bit()));
    writer.boolean(declaration.content_projection.is_some());
    if let Some(projection) = &declaration.content_projection {
        writer.id(projection.identity.domain);
        writer.u64(projection.identity.projection_report_fingerprint);
        encode_content_algebra(writer, &projection.algebra)?;
        encode_content_projection_expression(writer, &projection.expression)?;
    }
    encode_establishment_routes(writer, &declaration.establishment_routes)?;
    Ok(())
}

pub(super) fn encode_service(
    writer: &mut Writer,
    declaration: &ServiceDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.u32(declaration.identity.0);
    writer.len("service parents", declaration.parents.len())?;
    for parent in &declaration.parents {
        writer.id(*parent);
    }
    Ok(())
}

pub(super) fn encode_installation_reach_dependency(
    writer: &mut Writer,
    dependency: &InstallationReachDependency,
) -> Result<(), CodecError> {
    writer.u32(dependency.requirement_identity.0);
    writer.len(
        "installation reach upper bound",
        dependency.upper_bound.len(),
    )?;
    for service in &dependency.upper_bound {
        writer.id(*service);
    }
    Ok(())
}

pub(super) fn decode_structural_domain(
    reader: &mut Reader<'_>,
) -> Result<StructuralDomainDeclaration, CodecError> {
    Ok(StructuralDomainDeclaration {
        id: reader.id("StructuralDomainId")?,
        semantic_domain: reader.id("DomainSemanticId")?,
        identity: TerminalIdentityIndex(reader.u32()?),
        carrier: reader.id("StructuralTypeId")?,
        predicate: if reader.boolean()? {
            Some(terminal_psi::StructuralDomainPredicate {
                subject: reader.id("domain predicate subject")?,
                proposition: super::super::proposition_wire::decode_proposition(reader, 0)?,
            })
        } else {
            None
        },
        byte_predicate: match reader.u8()? {
            0 => None,
            1 => Some(terminal_psi::ByteSequencePredicate::ValidUtf8),
            2 => Some(terminal_psi::ByteSequencePredicate::NoNul),
            4 => Some(terminal_psi::ByteSequencePredicate::AsciiOnly),
            8 => Some(terminal_psi::ByteSequencePredicate::NonEmpty),
            tag => return Err(CodecError::InvalidTag("ByteSequencePredicate", tag)),
        },
        content_projection: if reader.boolean()? {
            Some(StructuralContentProjection {
                identity: ContentProjectionIdentity {
                    domain: reader.id("ContentDomainId")?,
                    projection_report_fingerprint: reader.u64()?,
                },
                algebra: decode_content_algebra(reader)?,
                expression: decode_content_projection_expression(reader, 0)?,
            })
        } else {
            None
        },
        establishment_routes: decode_establishment_routes(reader)?,
    })
}

pub(super) fn decode_service(reader: &mut Reader<'_>) -> Result<ServiceDeclaration, CodecError> {
    Ok(ServiceDeclaration {
        id: reader.id("ServiceId")?,
        identity: TerminalIdentityIndex(reader.u32()?),
        parents: decode_ids(reader, "ServiceId")?,
    })
}

pub(super) fn decode_installation_reach_dependency(
    reader: &mut Reader<'_>,
) -> Result<InstallationReachDependency, CodecError> {
    Ok(InstallationReachDependency {
        requirement_identity: TerminalIdentityIndex(reader.u32()?),
        upper_bound: decode_ids(reader, "ServiceId")?,
    })
}
