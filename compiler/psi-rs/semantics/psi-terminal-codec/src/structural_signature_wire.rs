//! Canonical structural signatures and boundary declaration wire format.
//!
//! This module owns exact structural parameter rows, published service
//! ceilings, and boundary-machine declaration envelopes. Operation bodies and
//! provider refinement semantics remain outside this module.

use psi_core::ServiceId;
use psi_terminal::{
    BoundaryMachineDeclaration, StructuralDomainRequirement, StructuralMultiplicity,
    StructuralParameterDeclaration,
};

use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::wire::{Reader, Writer};
use super::{CodecError, decode_counted, decode_ids, decode_optional_id, encode_optional_id};

pub(super) fn encode_boundary_machine(
    writer: &mut Writer,
    declaration: &BoundaryMachineDeclaration,
) -> Result<(), CodecError> {
    writer.id(declaration.id);
    writer.string("boundary machine identity", &declaration.identity)?;
    encode_optional_id(writer, declaration.attachment);
    encode_structural_parameters(writer, &declaration.structural_parameters)?;
    writer.boolean(declaration.result.is_some());
    if let Some(result) = declaration.result {
        encode_scalar_type(writer, result);
    }
    writer.len(
        "boundary structural requirements",
        declaration.requires.len(),
    )?;
    for requirement in &declaration.requires {
        writer.u32(requirement.argument_index);
        writer.id(requirement.domain);
    }
    encode_service_ceiling(writer, &declaration.published_service_ceiling)
}

pub(super) fn encode_structural_parameters(
    writer: &mut Writer,
    parameters: &[StructuralParameterDeclaration],
) -> Result<(), CodecError> {
    writer.len("structural parameters", parameters.len())?;
    for parameter in parameters {
        writer.id(parameter.place);
        writer.u32(parameter.position);
        writer.u8(u8::from(parameter.is_self));
        writer.id(parameter.structural_type);
        writer.u8(match parameter.multiplicity {
            StructuralMultiplicity::Unrestricted => 1,
            StructuralMultiplicity::Affine => 2,
            StructuralMultiplicity::Linear => 3,
        });
        writer.len(
            "structural parameter qualifications",
            parameter.qualifications.len(),
        )?;
        for qualification in &parameter.qualifications {
            writer.id(*qualification);
        }
    }
    Ok(())
}

pub(super) fn encode_service_ceiling(
    writer: &mut Writer,
    services: &[ServiceId],
) -> Result<(), CodecError> {
    writer.len("published service ceiling", services.len())?;
    for service in services {
        writer.id(*service);
    }
    Ok(())
}

pub(super) fn decode_boundary_machine(
    reader: &mut Reader<'_>,
) -> Result<BoundaryMachineDeclaration, CodecError> {
    Ok(BoundaryMachineDeclaration {
        id: reader.id("BoundaryMachineId")?,
        identity: reader.string("boundary machine identity")?,
        attachment: decode_optional_id(reader, "StructuralTypeId")?,
        structural_parameters: decode_structural_parameters(reader)?,
        result: reader
            .boolean()?
            .then(|| decode_scalar_type(reader))
            .transpose()?,
        requires: decode_counted(reader, |reader| {
            Ok(StructuralDomainRequirement {
                argument_index: reader.u32()?,
                domain: reader.id("StructuralDomainId")?,
            })
        })?,
        published_service_ceiling: decode_ids(reader, "ServiceId")?,
    })
}

pub(super) fn decode_structural_parameters(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralParameterDeclaration>, CodecError> {
    decode_counted(reader, |reader| {
        let place = reader.id("PlaceId")?;
        let position = reader.u32()?;
        let is_self = reader.boolean()?;
        let structural_type = reader.id("StructuralTypeId")?;
        let multiplicity = match reader.u8()? {
            1 => StructuralMultiplicity::Unrestricted,
            2 => StructuralMultiplicity::Affine,
            3 => StructuralMultiplicity::Linear,
            tag => return Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
        };
        Ok(StructuralParameterDeclaration {
            place,
            position,
            is_self,
            structural_type,
            multiplicity,
            qualifications: decode_ids(reader, "StructuralDomainId")?,
        })
    })
}
