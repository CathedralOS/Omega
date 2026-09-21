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
    StructuralDomainDeclaration, StructuralEstablishmentRoute,
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
                writer.string("route requirement identity", requirement)?;
            }
            StructuralEstablishmentRoute::ExactMachine { machine } => {
                writer.u8(2);
                writer.string("route machine identity", machine)?;
            }
            StructuralEstablishmentRoute::BoundaryRequirement { requirement } => {
                writer.u8(3);
                writer.string("route boundary requirement identity", requirement)?;
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
                requirement: reader.string("route requirement identity")?,
            },
            2 => StructuralEstablishmentRoute::ExactMachine {
                machine: reader.string("route machine identity")?,
            },
            3 => StructuralEstablishmentRoute::BoundaryRequirement {
                requirement: reader.string("route boundary requirement identity")?,
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
    writer.string("structural domain identity", &declaration.identity)?;
    writer.id(declaration.carrier);
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
    writer.string("service identity", &declaration.identity)?;
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
    writer.string(
        "installation reach requirement identity",
        &dependency.requirement_identity,
    )?;
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
        identity: reader.string("structural domain identity")?,
        carrier: reader.id("StructuralTypeId")?,
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
        identity: reader.string("service identity")?,
        parents: decode_ids(reader, "ServiceId")?,
    })
}

pub(super) fn decode_installation_reach_dependency(
    reader: &mut Reader<'_>,
) -> Result<InstallationReachDependency, CodecError> {
    Ok(InstallationReachDependency {
        requirement_identity: reader.string("installation reach requirement identity")?,
        upper_bound: decode_ids(reader, "ServiceId")?,
    })
}
