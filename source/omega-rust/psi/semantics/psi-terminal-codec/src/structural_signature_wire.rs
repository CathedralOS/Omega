//! Canonical structural signatures and boundary declaration wire format.
//!
//! This module owns exact structural parameter rows, published service
//! ceilings, and boundary-machine declaration envelopes. Operation bodies and
//! provider refinement semantics remain outside this module.

use psi_core::{
    ContentAlgebra, ContentAlgebraKind, ContentPlaceSegment, ContentPlaceVersion,
    ContentProjectionExpression, ContentProjectionIdentity, ContentProjectionScalar,
    DomainSemanticId, ServiceId,
};
use psi_terminal::{
    BoundaryContentGuarantee, BoundaryMachineDeclaration, ProgramLocalRootIntroductionSchema,
    RetainedBorrowContentProjection, RetainedBorrowCustody, RetainedBorrowPlace,
    RetainedBorrowPlaceRoot, StructuralAccess, StructuralContentProjection,
    StructuralDomainRequirement, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathQualification,
};

use super::content_wire::{
    decode_content_conservation_guarantee, encode_content_conservation_guarantee,
};
use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::wire::{Reader, Writer};
use super::{
    CodecError, decode_counted, decode_ids, decode_optional_id, decode_structural_path,
    encode_optional_id, encode_structural_path,
};

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
        writer.u64(schema.projection.projection_report_fingerprint);
        writer.u8(match schema.algebra.kind {
            ContentAlgebraKind::IntervalSet => 1,
            ContentAlgebraKind::CountedQuantity => 2,
        });
        writer.string(
            "program-local content algebra parameter",
            &schema.algebra.parameter,
        )?;
        encode_content_projection_expression(writer, &schema.capacity)?;
        writer.u64(schema.compatibility_report_identity);
    }
    writer.len(
        "boundary content guarantees",
        declaration.content_guarantees.len(),
    )?;
    for guarantee in &declaration.content_guarantees {
        match guarantee {
            BoundaryContentGuarantee::Conservation(guarantee) => {
                writer.u8(1);
                encode_content_conservation_guarantee(writer, guarantee)?;
            }
            BoundaryContentGuarantee::RetainedBorrow(custody) => {
                writer.u8(2);
                encode_retained_borrow_custody(writer, custody)?;
            }
        }
    }
    encode_service_ceiling(writer, &declaration.published_service_ceiling)
}

pub(super) fn encode_content_projection_expression(
    writer: &mut Writer,
    capacity: &ContentProjectionExpression,
) -> Result<(), CodecError> {
    match capacity {
        ContentProjectionExpression::IntervalSet(members) => {
            writer.u8(1);
            writer.len("program-local interval members", members.len())?;
            for (start, end) in members {
                encode_capacity_scalar(writer, start)?;
                encode_capacity_scalar(writer, end)?;
            }
        }
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            writer.u8(2);
            encode_capacity_scalar(writer, magnitude)?;
        }
    }
    Ok(())
}

fn encode_capacity_scalar(
    writer: &mut Writer,
    scalar: &ContentProjectionScalar,
) -> Result<(), CodecError> {
    match scalar {
        ContentProjectionScalar::SubjectField(path)
        | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
            writer.u8(
                if matches!(scalar, ContentProjectionScalar::SubjectField(_)) {
                    1
                } else {
                    2
                },
            );
            writer.strings("program-local capacity field path", path)?;
        }
        ContentProjectionScalar::Natural(value) => {
            writer.u8(3);
            writer.string("program-local natural", value)?;
        }
        ContentProjectionScalar::Successor(inner) => {
            writer.u8(4);
            encode_capacity_scalar(writer, inner)?;
        }
        ContentProjectionScalar::Add(left, right)
        | ContentProjectionScalar::Subtract(left, right)
        | ContentProjectionScalar::Multiply(left, right) => {
            writer.u8(match scalar {
                ContentProjectionScalar::Add(_, _) => 5,
                ContentProjectionScalar::Subtract(_, _) => 6,
                ContentProjectionScalar::Multiply(_, _) => 7,
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
        encode_structural_access(writer, parameter.access);
        writer.len(
            "structural parameter qualifications",
            parameter.qualifications.len(),
        )?;
        for qualification in &parameter.qualifications {
            writer.id(*qualification);
        }
        encode_projected_qualifications(writer, &parameter.projected_qualifications)?;
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
            let projection_report_fingerprint = reader.u64()?;
            let algebra_kind = match reader.u8()? {
                1 => ContentAlgebraKind::IntervalSet,
                2 => ContentAlgebraKind::CountedQuantity,
                tag => return Err(CodecError::InvalidTag("ContentAlgebraKind", tag)),
            };
            let algebra_parameter = reader.string("program-local content algebra parameter")?;
            let capacity = decode_content_projection_expression(reader, 0)?;
            let compatibility_report_identity = reader.u64()?;
            Ok(ProgramLocalRootIntroductionSchema {
                argument_index,
                source_parameter_position,
                qualification,
                carrier,
                projection: ContentProjectionIdentity {
                    domain: projection_domain,
                    projection_report_fingerprint,
                },
                algebra: ContentAlgebra {
                    kind: algebra_kind,
                    parameter: algebra_parameter,
                },
                capacity,
                compatibility_report_identity,
            })
        })?,
        content_guarantees: decode_counted(reader, |reader| match reader.u8()? {
            1 => Ok(BoundaryContentGuarantee::Conservation(
                decode_content_conservation_guarantee(reader)?,
            )),
            2 => Ok(BoundaryContentGuarantee::RetainedBorrow(
                decode_retained_borrow_custody(reader)?,
            )),
            tag => Err(CodecError::InvalidTag("BoundaryContentGuarantee", tag)),
        })?,
        published_service_ceiling: decode_ids(reader, "ServiceId")?,
    })
}

fn encode_retained_borrow_place(
    writer: &mut Writer,
    place: &RetainedBorrowPlace,
) -> Result<(), CodecError> {
    writer.u8(match place.version {
        ContentPlaceVersion::Entry => 1,
        ContentPlaceVersion::Current => 2,
    });
    match &place.root {
        RetainedBorrowPlaceRoot::Parameter {
            position,
            identity,
            is_self,
        } => {
            writer.u8(1);
            writer.u32(*position);
            writer.string("retained-borrow parameter identity", identity)?;
            writer.boolean(*is_self);
        }
        RetainedBorrowPlaceRoot::Result => writer.u8(2),
    }
    writer.len("retained-borrow place segments", place.segments.len())?;
    for segment in &place.segments {
        match segment {
            ContentPlaceSegment::Case(identity) => {
                writer.u8(1);
                writer.string("retained-borrow case identity", identity)?;
            }
            ContentPlaceSegment::Field(identity) => {
                writer.u8(2);
                writer.string("retained-borrow field identity", identity)?;
            }
            ContentPlaceSegment::FixedIndex(index) => {
                writer.u8(3);
                writer.u64(*index);
            }
        }
    }
    Ok(())
}

fn decode_retained_borrow_place(
    reader: &mut Reader<'_>,
) -> Result<RetainedBorrowPlace, CodecError> {
    let version = match reader.u8()? {
        1 => ContentPlaceVersion::Entry,
        2 => ContentPlaceVersion::Current,
        tag => return Err(CodecError::InvalidTag("ContentPlaceVersion", tag)),
    };
    let root = match reader.u8()? {
        1 => RetainedBorrowPlaceRoot::Parameter {
            position: reader.u32()?,
            identity: reader.string("retained-borrow parameter identity")?,
            is_self: reader.boolean()?,
        },
        2 => RetainedBorrowPlaceRoot::Result,
        tag => return Err(CodecError::InvalidTag("RetainedBorrowPlaceRoot", tag)),
    };
    let segments = decode_counted(reader, |reader| match reader.u8()? {
        1 => Ok(ContentPlaceSegment::Case(
            reader.string("retained-borrow case identity")?,
        )),
        2 => Ok(ContentPlaceSegment::Field(
            reader.string("retained-borrow field identity")?,
        )),
        3 => Ok(ContentPlaceSegment::FixedIndex(reader.u64()?)),
        tag => Err(CodecError::InvalidTag("ContentPlaceSegment", tag)),
    })?;
    Ok(RetainedBorrowPlace {
        version,
        root,
        segments,
    })
}

fn encode_retained_borrow_projection(
    writer: &mut Writer,
    projection: &RetainedBorrowContentProjection,
) -> Result<(), CodecError> {
    writer.id(projection.semantic_domain);
    writer.string(
        "retained-borrow projection carrier identity",
        &projection.carrier_identity,
    )?;
    writer.id(projection.projection.identity.domain);
    writer.u64(projection.projection.identity.projection_report_fingerprint);
    writer.u8(match projection.projection.algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    writer.string(
        "retained-borrow content algebra parameter",
        &projection.projection.algebra.parameter,
    )?;
    encode_content_projection_expression(writer, &projection.projection.expression)
}

fn decode_retained_borrow_projection(
    reader: &mut Reader<'_>,
) -> Result<RetainedBorrowContentProjection, CodecError> {
    let semantic_domain = reader.id::<DomainSemanticId>("DomainSemanticId")?;
    let carrier_identity = reader.string("retained-borrow projection carrier identity")?;
    let domain = reader.id("ContentDomainId")?;
    let projection_report_fingerprint = reader.u64()?;
    let kind = match reader.u8()? {
        1 => ContentAlgebraKind::IntervalSet,
        2 => ContentAlgebraKind::CountedQuantity,
        tag => return Err(CodecError::InvalidTag("ContentAlgebraKind", tag)),
    };
    let parameter = reader.string("retained-borrow content algebra parameter")?;
    let expression = decode_content_projection_expression(reader, 0)?;
    Ok(RetainedBorrowContentProjection {
        semantic_domain,
        carrier_identity,
        projection: StructuralContentProjection {
            identity: ContentProjectionIdentity {
                domain,
                projection_report_fingerprint,
            },
            algebra: ContentAlgebra { kind, parameter },
            expression,
        },
    })
}

fn encode_retained_borrow_custody(
    writer: &mut Writer,
    custody: &RetainedBorrowCustody,
) -> Result<(), CodecError> {
    writer.string(
        "retained-borrow callable identity",
        &custody.callable_identity,
    )?;
    encode_retained_borrow_place(writer, &custody.source)?;
    encode_retained_borrow_place(writer, &custody.result)?;
    writer.u8(match custody.access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
    writer.u32(custody.callable_lifetime_parameter_count);
    writer.u32(custody.callable_lifetime_parameter_ordinal);
    writer.string(
        "retained-borrow result nominal identity",
        &custody.result_nominal_identity,
    )?;
    writer.u8(match custody.result_multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
    writer.u32(custody.result_lifetime_argument_count);
    writer.u32(custody.result_lifetime_argument_ordinal);
    writer.boolean(custody.result_lifetime_slot_is_erased);
    writer.id(custody.retained_semantic_domain);
    encode_retained_borrow_projection(writer, &custody.source_projection)?;
    encode_retained_borrow_projection(writer, &custody.result_projection)
}

fn decode_retained_borrow_custody(
    reader: &mut Reader<'_>,
) -> Result<RetainedBorrowCustody, CodecError> {
    Ok(RetainedBorrowCustody {
        callable_identity: reader.string("retained-borrow callable identity")?,
        source: decode_retained_borrow_place(reader)?,
        result: decode_retained_borrow_place(reader)?,
        access: match reader.u8()? {
            1 => StructuralAccess::Owned,
            2 => StructuralAccess::SharedBorrow,
            3 => StructuralAccess::MutableBorrow,
            4 => StructuralAccess::WriteOnlyBorrow,
            tag => return Err(CodecError::InvalidTag("StructuralAccess", tag)),
        },
        callable_lifetime_parameter_count: reader.u32()?,
        callable_lifetime_parameter_ordinal: reader.u32()?,
        result_nominal_identity: reader.string("retained-borrow result nominal identity")?,
        result_multiplicity: match reader.u8()? {
            1 => StructuralMultiplicity::Unrestricted,
            2 => StructuralMultiplicity::Affine,
            3 => StructuralMultiplicity::Linear,
            tag => return Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
        },
        result_lifetime_argument_count: reader.u32()?,
        result_lifetime_argument_ordinal: reader.u32()?,
        result_lifetime_slot_is_erased: reader.boolean()?,
        retained_semantic_domain: reader.id("DomainSemanticId")?,
        source_projection: decode_retained_borrow_projection(reader)?,
        result_projection: decode_retained_borrow_projection(reader)?,
    })
}

pub(super) fn decode_content_projection_expression(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ContentProjectionExpression, CodecError> {
    if depth > 256 {
        return Err(CodecError::InvalidTag(
            "ContentProjectionExpressionDepth",
            0,
        ));
    }
    match reader.u8()? {
        1 => Ok(ContentProjectionExpression::IntervalSet(decode_counted(
            reader,
            |reader| {
                Ok((
                    decode_capacity_scalar(reader, depth + 1)?,
                    decode_capacity_scalar(reader, depth + 1)?,
                ))
            },
        )?)),
        2 => Ok(ContentProjectionExpression::CountedQuantity(
            decode_capacity_scalar(reader, depth + 1)?,
        )),
        tag => Err(CodecError::InvalidTag("ContentProjectionExpression", tag)),
    }
}

fn decode_capacity_scalar(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<ContentProjectionScalar, CodecError> {
    if depth > 256 {
        return Err(CodecError::InvalidTag("ContentProjectionScalarDepth", 0));
    }
    match reader.u8()? {
        1 => Ok(ContentProjectionScalar::SubjectField(
            reader.strings("program-local capacity field path")?,
        )),
        2 => Ok(ContentProjectionScalar::RuntimeScalarEmbedding(
            reader.strings("program-local capacity field path")?,
        )),
        3 => Ok(ContentProjectionScalar::Natural(
            reader.string("program-local natural")?,
        )),
        4 => Ok(ContentProjectionScalar::Successor(Box::new(
            decode_capacity_scalar(reader, depth + 1)?,
        ))),
        5 => Ok(ContentProjectionScalar::Add(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        6 => Ok(ContentProjectionScalar::Subtract(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        7 => Ok(ContentProjectionScalar::Multiply(
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
            Box::new(decode_capacity_scalar(reader, depth + 1)?),
        )),
        tag => Err(CodecError::InvalidTag("ContentProjectionScalar", tag)),
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
        let access = decode_structural_access(reader)?;
        Ok(StructuralParameterDeclaration {
            place,
            position,
            is_self,
            structural_type,
            multiplicity,
            access,
            qualifications: decode_ids(reader, "StructuralDomainId")?,
            projected_qualifications: decode_projected_qualifications(reader)?,
        })
    })
}

pub(super) fn encode_projected_qualifications(
    writer: &mut Writer,
    qualifications: &[StructuralPathQualification],
) -> Result<(), CodecError> {
    writer.len(
        "projected structural parameter qualifications",
        qualifications.len(),
    )?;
    for qualification in qualifications {
        encode_structural_path(
            writer,
            "projected structural qualification path",
            &qualification.path,
        )?;
        writer.id(qualification.domain);
    }
    Ok(())
}

pub(super) fn decode_projected_qualifications(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralPathQualification>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(StructuralPathQualification {
            path: decode_structural_path(reader)?,
            domain: reader.id("StructuralDomainId")?,
        })
    })
}

pub(super) fn encode_structural_access(writer: &mut Writer, access: StructuralAccess) {
    writer.u8(match access {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

pub(super) fn decode_structural_access(
    reader: &mut Reader<'_>,
) -> Result<StructuralAccess, CodecError> {
    match reader.u8()? {
        1 => Ok(StructuralAccess::Owned),
        2 => Ok(StructuralAccess::SharedBorrow),
        3 => Ok(StructuralAccess::MutableBorrow),
        4 => Ok(StructuralAccess::WriteOnlyBorrow),
        tag => Err(CodecError::InvalidTag("StructuralAccess", tag)),
    }
}
