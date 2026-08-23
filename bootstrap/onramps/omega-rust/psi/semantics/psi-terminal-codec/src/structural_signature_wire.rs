//! Canonical structural signatures and boundary declaration wire format.
//!
//! This module owns exact structural parameter rows, published service
//! ceilings, and boundary-machine declaration envelopes. Operation bodies and
//! provider refinement semantics remain outside this module.

use psi_core::{
    ContentAlgebra, ContentAlgebraKind, ContentProjectionIdentity, ProgramLocalCapacityExpression,
    ProgramLocalCapacityScalar, ServiceId,
};
use psi_terminal::{
    BoundaryMachineDeclaration, ProgramLocalRootIntroductionSchema, StructuralDomainRequirement,
    StructuralMultiplicity, StructuralParameterDeclaration,
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
    writer.len(
        "boundary scalar parameters",
        declaration.scalar_parameters.len(),
    )?;
    for parameter in &declaration.scalar_parameters {
        encode_scalar_type(writer, *parameter);
    }
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
    writer.len(
        "program-local root introduction schemas",
        declaration.program_local_root_introductions.len(),
    )?;
    for schema in &declaration.program_local_root_introductions {
        writer.u32(schema.argument_index);
        writer.u32(schema.source_parameter_position);
        writer.id(schema.qualification);
        writer.id(schema.carrier);
        writer.id(schema.projection.domain);
        writer.u64(schema.projection.projection_fingerprint);
        writer.u8(match schema.algebra.kind {
            ContentAlgebraKind::IntervalSet => 1,
            ContentAlgebraKind::CountedQuantity => 2,
        });
        writer.string(
            "program-local content algebra parameter",
            &schema.algebra.parameter,
        )?;
        encode_capacity(writer, &schema.capacity)?;
        writer.u64(schema.identity);
    }
    encode_service_ceiling(writer, &declaration.published_service_ceiling)
}

fn encode_capacity(
    writer: &mut Writer,
    capacity: &ProgramLocalCapacityExpression,
) -> Result<(), CodecError> {
    match capacity {
        ProgramLocalCapacityExpression::IntervalSet(members) => {
            writer.u8(1);
            writer.len("program-local interval members", members.len())?;
            for (start, end) in members {
                encode_capacity_scalar(writer, start)?;
                encode_capacity_scalar(writer, end)?;
            }
        }
        ProgramLocalCapacityExpression::CountedQuantity(magnitude) => {
            writer.u8(2);
            encode_capacity_scalar(writer, magnitude)?;
        }
    }
    Ok(())
}

fn encode_capacity_scalar(
    writer: &mut Writer,
    scalar: &ProgramLocalCapacityScalar,
) -> Result<(), CodecError> {
    match scalar {
        ProgramLocalCapacityScalar::SubjectField(path)
        | ProgramLocalCapacityScalar::RuntimeScalarEmbedding(path) => {
            writer.u8(
                if matches!(scalar, ProgramLocalCapacityScalar::SubjectField(_)) {
                    1
                } else {
                    2
                },
            );
            writer.strings("program-local capacity field path", path)?;
        }
        ProgramLocalCapacityScalar::Natural(value) => {
            writer.u8(3);
            writer.string("program-local natural", value)?;
        }
        ProgramLocalCapacityScalar::Successor(inner) => {
            writer.u8(4);
            encode_capacity_scalar(writer, inner)?;
        }
        ProgramLocalCapacityScalar::Add(left, right)
        | ProgramLocalCapacityScalar::Subtract(left, right)
        | ProgramLocalCapacityScalar::Multiply(left, right) => {
            writer.u8(match scalar {
                ProgramLocalCapacityScalar::Add(_, _) => 5,
                ProgramLocalCapacityScalar::Subtract(_, _) => 6,
                ProgramLocalCapacityScalar::Multiply(_, _) => 7,
                _ => unreachable!(),
            });
            encode_capacity_scalar(writer, left)?;
            encode_capacity_scalar(writer, right)?;
        }
    }
    Ok(())
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
        scalar_parameters: decode_counted(reader, decode_scalar_type)?,
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
        program_local_root_introductions: decode_counted(reader, |reader| {
            let argument_index = reader.u32()?;
            let source_parameter_position = reader.u32()?;
            let qualification = reader.id("StructuralDomainId")?;
            let carrier = reader.id("StructuralTypeId")?;
            let projection_domain = reader.id("ContentDomainId")?;
            let projection_fingerprint = reader.u64()?;
            let algebra_kind = match reader.u8()? {
                1 => ContentAlgebraKind::IntervalSet,
                2 => ContentAlgebraKind::CountedQuantity,
                tag => return Err(CodecError::InvalidTag("ContentAlgebraKind", tag)),
            };
            let algebra_parameter = reader.string("program-local content algebra parameter")?;
            let capacity = decode_capacity(reader, 0)?;
            let identity = reader.u64()?;
            Ok(ProgramLocalRootIntroductionSchema {
                argument_index,
                source_parameter_position,
                qualification,
                carrier,
                projection: ContentProjectionIdentity {
                    domain: projection_domain,
                    projection_fingerprint,
                },
                algebra: ContentAlgebra {
                    kind: algebra_kind,
                    parameter: algebra_parameter,
                },
                capacity,
                identity,
            })
        })?,
        published_service_ceiling: decode_ids(reader, "ServiceId")?,
    })
}

fn decode_capacity(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ProgramLocalCapacityExpression, CodecError> {
    if depth > 256 {
        return Err(CodecError::InvalidTag(
            "ProgramLocalCapacityExpressionDepth",
            0,
        ));
    }
    match reader.u8()? {
        1 => Ok(ProgramLocalCapacityExpression::IntervalSet(decode_counted(
            reader,
            |reader| {
                Ok((
                    decode_capacity_scalar(reader, depth + 1)?,
                    decode_capacity_scalar(reader, depth + 1)?,
                ))
            },
        )?)),
        2 => Ok(ProgramLocalCapacityExpression::CountedQuantity(
            decode_capacity_scalar(reader, depth + 1)?,
        )),
        tag => Err(CodecError::InvalidTag(
            "ProgramLocalCapacityExpression",
            tag,
        )),
    }
}

fn decode_capacity_scalar(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ProgramLocalCapacityScalar, CodecError> {
    if depth > 256 {
        return Err(CodecError::InvalidTag("ProgramLocalCapacityScalarDepth", 0));
    }
    match reader.u8()? {
        1 => Ok(ProgramLocalCapacityScalar::SubjectField(
            reader.strings("program-local capacity field path")?,
        )),
        2 => Ok(ProgramLocalCapacityScalar::RuntimeScalarEmbedding(
            reader.strings("program-local capacity field path")?,
        )),
        3 => Ok(ProgramLocalCapacityScalar::Natural(
            reader.string("program-local natural")?,
        )),
        4 => Ok(ProgramLocalCapacityScalar::Successor(Box::new(
            decode_capacity_scalar(reader, depth + 1)?,
        ))),
        5 => Ok(ProgramLocalCapacityScalar::Add(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        6 => Ok(ProgramLocalCapacityScalar::Subtract(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        7 => Ok(ProgramLocalCapacityScalar::Multiply(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        tag => Err(CodecError::InvalidTag("ProgramLocalCapacityScalar", tag)),
    }
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
