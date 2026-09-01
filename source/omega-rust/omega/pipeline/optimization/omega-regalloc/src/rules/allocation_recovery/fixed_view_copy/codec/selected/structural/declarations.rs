use omega_selected_instructions::{
    SelectedStructuralUnitCallArgument, SelectedStructuralUnitParameter,
};
use omega_target_operations::{TargetStructuralArgument, TargetStructuralParameter};
use psi_core::{
    AffineConstructionElement, IeeeFloatFormat, OperationId, PlaceId, StructuralCaseId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use psi_terminal::{
    BindingRelevance, ByteSequenceCarrier, EntryClaim, StructuralAccess, StructuralArgument,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

use crate::FixedViewCopyDecodeError;

use super::calling::{decode_placement, decode_shape, encode_placement, encode_shape};
use super::projected_qualifications::{decode_projected, encode_projected};
use crate::rules::allocation_recovery::fixed_view_copy::codec::{
    primitives::{Cursor, decode_id, decode_ids, encode_ids, length},
    values::{decode_scalar, encode_scalar},
};

pub(super) fn encode_parameter(
    bytes: &mut Vec<u8>,
    parameter: &SelectedStructuralUnitParameter,
    retain_projected_qualifications: bool,
) {
    encode_semantic_parameter(bytes, &parameter.semantic, retain_projected_qualifications);
    encode_target_parameter(bytes, &parameter.target);
}

pub(super) fn decode_parameter(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<SelectedStructuralUnitParameter, FixedViewCopyDecodeError> {
    Ok(SelectedStructuralUnitParameter {
        semantic: decode_semantic_parameter(cursor, retain_projected_qualifications)?,
        target: decode_target_parameter(cursor)?,
    })
}

pub(super) fn encode_argument(bytes: &mut Vec<u8>, argument: &SelectedStructuralUnitCallArgument) {
    encode_semantic_argument(bytes, &argument.semantic);
    encode_target_argument(bytes, &argument.target);
}

pub(super) fn decode_argument(
    cursor: &mut Cursor<'_>,
) -> Result<SelectedStructuralUnitCallArgument, FixedViewCopyDecodeError> {
    Ok(SelectedStructuralUnitCallArgument {
        semantic: decode_semantic_argument(cursor)?,
        target: decode_target_argument(cursor)?,
    })
}

pub(super) fn encode_type(bytes: &mut Vec<u8>, declaration: &StructuralTypeDeclaration) {
    bytes.extend_from_slice(&declaration.id.get().to_le_bytes());
    encode_string(bytes, &declaration.identity);
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => {
            bytes.push(6);
            encode_scalar(bytes, *scalar);
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.push(1);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralTypeShape::Record { fields } => {
            bytes.push(2);
            encode_fields(bytes, fields);
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.push(3);
            bytes.extend_from_slice(&element.get().to_le_bytes());
            bytes.extend_from_slice(&length.to_le_bytes());
        }
        StructuralTypeShape::Sum { cases } => {
            bytes.push(4);
            encode_cases(bytes, cases);
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            bytes.push(5);
            encode_fields(bytes, fields);
            encode_cases(bytes, cases);
        }
    }
}

pub(super) fn decode_type(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralTypeDeclaration, FixedViewCopyDecodeError> {
    let id = decode_id(cursor, StructuralTypeId::new)?;
    let identity = decode_string(cursor)?;
    let shape = match cursor.byte()? {
        1 => StructuralTypeShape::ByteSequence(decode_byte_sequence_carrier(cursor)?),
        2 => StructuralTypeShape::Record {
            fields: decode_fields(cursor)?,
        },
        3 => StructuralTypeShape::FixedArray {
            element: decode_id(cursor, StructuralTypeId::new)?,
            length: cursor.u64()?,
        },
        4 => StructuralTypeShape::Sum {
            cases: decode_cases(cursor)?,
        },
        5 => StructuralTypeShape::Mixed {
            fields: decode_fields(cursor)?,
            cases: decode_cases(cursor)?,
        },
        6 => StructuralTypeShape::PrimitiveScalar(decode_scalar(cursor)?),
        tag => return Err(FixedViewCopyDecodeError::UnknownStructuralTypeShape(tag)),
    };
    Ok(StructuralTypeDeclaration {
        id,
        identity,
        shape,
    })
}

fn encode_cases(bytes: &mut Vec<u8>, cases: &[StructuralCaseDeclaration]) {
    length(bytes, cases.len());
    for case in cases {
        bytes.extend_from_slice(&case.id.get().to_le_bytes());
        encode_string(bytes, &case.identity);
        encode_fields(bytes, &case.fields);
    }
}

fn decode_cases(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<StructuralCaseDeclaration>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut cases = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        cases.push(StructuralCaseDeclaration {
            id: decode_id(cursor, StructuralCaseId::new)?,
            identity: decode_string(cursor)?,
            fields: decode_fields(cursor)?,
        });
    }
    Ok(cases)
}

fn encode_fields(bytes: &mut Vec<u8>, fields: &[StructuralFieldDeclaration]) {
    length(bytes, fields.len());
    for field in fields {
        bytes.extend_from_slice(&field.id.get().to_le_bytes());
        encode_string(bytes, &field.identity);
        bytes.push(match field.relevance {
            BindingRelevance::Relevant => 1,
            BindingRelevance::Erased => 2,
        });
        match &field.field_type {
            StructuralFieldType::Scalar(scalar) => {
                bytes.push(1);
                encode_scalar(bytes, *scalar);
            }
            StructuralFieldType::IeeeFloat(format) => {
                bytes.push(2);
                bytes.push(match format {
                    IeeeFloatFormat::Binary32 => 1,
                    IeeeFloatFormat::Binary64 => 2,
                });
            }
            StructuralFieldType::ByteSequence(carrier) => {
                bytes.push(3);
                encode_byte_sequence_carrier(bytes, *carrier);
            }
            StructuralFieldType::Structural(structural_type) => {
                bytes.push(4);
                bytes.extend_from_slice(&structural_type.get().to_le_bytes());
            }
            StructuralFieldType::Erased { type_identity } => {
                bytes.push(5);
                encode_string(bytes, type_identity);
            }
        }
    }
}

fn decode_fields(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<StructuralFieldDeclaration>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut fields = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let id = decode_id(cursor, StructuralFieldId::new)?;
        let identity = decode_string(cursor)?;
        let relevance = match cursor.byte()? {
            1 => BindingRelevance::Relevant,
            2 => BindingRelevance::Erased,
            tag => return Err(FixedViewCopyDecodeError::UnknownBindingRelevance(tag)),
        };
        let field_type = match cursor.byte()? {
            1 => StructuralFieldType::Scalar(decode_scalar(cursor)?),
            2 => StructuralFieldType::IeeeFloat(match cursor.byte()? {
                1 => IeeeFloatFormat::Binary32,
                2 => IeeeFloatFormat::Binary64,
                tag => return Err(FixedViewCopyDecodeError::UnknownIeeeFloatFormat(tag)),
            }),
            3 => StructuralFieldType::ByteSequence(decode_byte_sequence_carrier(cursor)?),
            4 => StructuralFieldType::Structural(decode_id(cursor, StructuralTypeId::new)?),
            5 => StructuralFieldType::Erased {
                type_identity: decode_string(cursor)?,
            },
            tag => return Err(FixedViewCopyDecodeError::UnknownStructuralFieldType(tag)),
        };
        fields.push(StructuralFieldDeclaration {
            id,
            identity,
            relevance,
            field_type,
        });
    }
    Ok(fields)
}

fn encode_byte_sequence_carrier(bytes: &mut Vec<u8>, carrier: ByteSequenceCarrier) {
    match carrier {
        ByteSequenceCarrier::BorrowedView => bytes.push(1),
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            bytes.push(2);
            bytes.extend_from_slice(&capacity.to_le_bytes());
        }
    }
}

fn decode_byte_sequence_carrier(
    cursor: &mut Cursor<'_>,
) -> Result<ByteSequenceCarrier, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(ByteSequenceCarrier::BorrowedView),
        2 => Ok(ByteSequenceCarrier::BoundedOwned {
            capacity: cursor.u64()?,
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownByteSequenceCarrier(tag)),
    }
}

fn encode_semantic_parameter(
    bytes: &mut Vec<u8>,
    parameter: &StructuralParameterDeclaration,
    retain_projected_qualifications: bool,
) {
    bytes.extend_from_slice(&parameter.place.get().to_le_bytes());
    bytes.extend_from_slice(&parameter.position.to_le_bytes());
    bytes.push(u8::from(parameter.is_self));
    bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_ids(
        bytes,
        parameter.qualifications.iter().map(|value| value.get()),
    );
    encode_projected(
        bytes,
        &parameter.projected_qualifications,
        retain_projected_qualifications,
    );
}

fn decode_semantic_parameter(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<StructuralParameterDeclaration, FixedViewCopyDecodeError> {
    Ok(StructuralParameterDeclaration {
        place: decode_id(cursor, PlaceId::new)?,
        position: cursor.u32()?,
        is_self: decode_bool(cursor)?,
        structural_type: decode_id(cursor, StructuralTypeId::new)?,
        multiplicity: decode_multiplicity(cursor)?,
        access: decode_access(cursor)?,
        qualifications: decode_ids(cursor, psi_core::StructuralDomainId::new)?,
        projected_qualifications: decode_projected(cursor, retain_projected_qualifications)?,
    })
}

fn encode_target_parameter(bytes: &mut Vec<u8>, parameter: &TargetStructuralParameter) {
    bytes.extend_from_slice(&parameter.place.get().to_le_bytes());
    bytes.extend_from_slice(&parameter.structural_type.get().to_le_bytes());
    encode_multiplicity(bytes, parameter.multiplicity);
    encode_access(bytes, parameter.access);
    encode_shape(bytes, parameter.shape);
    encode_placement(bytes, &parameter.placement);
}

fn decode_target_parameter(
    cursor: &mut Cursor<'_>,
) -> Result<TargetStructuralParameter, FixedViewCopyDecodeError> {
    Ok(TargetStructuralParameter {
        place: decode_id(cursor, PlaceId::new)?,
        structural_type: decode_id(cursor, StructuralTypeId::new)?,
        multiplicity: decode_multiplicity(cursor)?,
        access: decode_access(cursor)?,
        projected_qualifications: Vec::new(),
        shape: decode_shape(cursor)?,
        placement: decode_placement(cursor)?,
    })
}

pub(super) fn encode_semantic_argument(bytes: &mut Vec<u8>, argument: &StructuralArgument) {
    bytes.extend_from_slice(&argument.place.get().to_le_bytes());
    encode_path(bytes, &argument.path);
    encode_access(bytes, argument.access);
}

pub(super) fn decode_semantic_argument(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralArgument, FixedViewCopyDecodeError> {
    Ok(StructuralArgument {
        place: decode_id(cursor, PlaceId::new)?,
        path: decode_path(cursor)?,
        access: decode_access(cursor)?,
    })
}

fn encode_target_argument(bytes: &mut Vec<u8>, argument: &TargetStructuralArgument) {
    bytes.extend_from_slice(&argument.place.get().to_le_bytes());
    encode_access(bytes, argument.access);
    encode_path(bytes, &argument.path);
    bytes.extend_from_slice(&argument.root_structural_type.get().to_le_bytes());
    bytes.extend_from_slice(&argument.structural_type.get().to_le_bytes());
    encode_shape(bytes, argument.shape);
    bytes.extend_from_slice(&argument.source_byte_offset.to_le_bytes());
    encode_option_u64(bytes, argument.fixed_array_length);
    encode_option_u32(bytes, argument.element_stride);
    encode_placement(bytes, &argument.source);
    encode_placement(bytes, &argument.destination);
}

fn decode_target_argument(
    cursor: &mut Cursor<'_>,
) -> Result<TargetStructuralArgument, FixedViewCopyDecodeError> {
    Ok(TargetStructuralArgument {
        place: decode_id(cursor, PlaceId::new)?,
        access: decode_access(cursor)?,
        path: decode_path(cursor)?,
        root_structural_type: decode_id(cursor, StructuralTypeId::new)?,
        structural_type: decode_id(cursor, StructuralTypeId::new)?,
        shape: decode_shape(cursor)?,
        source_byte_offset: cursor.u32()?,
        fixed_array_length: decode_option_u64(cursor)?,
        element_stride: decode_option_u32(cursor)?,
        source: decode_placement(cursor)?,
        destination: decode_placement(cursor)?,
    })
}

pub(super) fn encode_place(bytes: &mut Vec<u8>, place: StructuralPlaceDeclaration) {
    bytes.extend_from_slice(&place.id.get().to_le_bytes());
    match place.kind {
        StructuralPlaceKind::Parameter { position, is_self } => {
            bytes.push(1);
            bytes.extend_from_slice(&position.to_le_bytes());
            bytes.push(u8::from(is_self));
        }
        StructuralPlaceKind::Result => bytes.push(2),
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            bytes.push(3);
            bytes.extend_from_slice(&producer.get().to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.push(4);
            bytes.extend_from_slice(&declaration_ordinal.to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            bytes.push(5);
            bytes.extend_from_slice(&attachment.get().to_le_bytes());
            bytes.extend_from_slice(&field.get().to_le_bytes());
            bytes.extend_from_slice(&boundary.get().to_le_bytes());
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction,
        } => {
            bytes.push(if construction.is_some() { 7 } else { 6 });
            bytes.extend_from_slice(&declaration_ordinal.to_le_bytes());
            bytes.extend_from_slice(&structural_type.get().to_le_bytes());
            if let Some(construction) = construction {
                bytes.extend_from_slice(&construction.root_structural_type.get().to_le_bytes());
                bytes.extend_from_slice(&construction.index.to_le_bytes());
            }
        }
    }
}

pub(super) fn decode_place(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralPlaceDeclaration, FixedViewCopyDecodeError> {
    let id = decode_id(cursor, PlaceId::new)?;
    let kind = match cursor.byte()? {
        1 => StructuralPlaceKind::Parameter {
            position: cursor.u32()?,
            is_self: decode_bool(cursor)?,
        },
        2 => StructuralPlaceKind::Result,
        3 => StructuralPlaceKind::OperationResult {
            producer: decode_id(cursor, OperationId::new)?,
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
        },
        4 => StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal: cursor.u32()?,
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
        },
        5 => StructuralPlaceKind::ProviderAttachment {
            attachment: decode_id(cursor, StructuralTypeId::new)?,
            field: decode_id(cursor, StructuralFieldId::new)?,
            boundary: decode_id(cursor, psi_core::BoundaryMachineId::new)?,
        },
        6 => StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: cursor.u32()?,
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
            construction: None,
        },
        7 => StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: cursor.u32()?,
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
            construction: Some(AffineConstructionElement {
                root_structural_type: decode_id(cursor, StructuralTypeId::new)?,
                index: cursor.u64()?,
            }),
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownStructuralPlaceKind(tag)),
    };
    Ok(StructuralPlaceDeclaration { id, kind })
}

pub(super) fn encode_entry_claim(bytes: &mut Vec<u8>, claim: &EntryClaim) {
    bytes.extend_from_slice(&claim.claim.get().to_le_bytes());
    bytes.extend_from_slice(&claim.input.get().to_le_bytes());
    encode_path(bytes, &claim.path);
}

pub(super) fn decode_entry_claim(
    cursor: &mut Cursor<'_>,
) -> Result<EntryClaim, FixedViewCopyDecodeError> {
    Ok(EntryClaim {
        claim: decode_id(cursor, psi_core::ClaimId::new)?,
        input: decode_id(cursor, PlaceId::new)?,
        path: decode_path(cursor)?,
    })
}

pub(super) fn encode_path(bytes: &mut Vec<u8>, path: &[StructuralPathSegment]) {
    length(bytes, path.len());
    for segment in path {
        match segment {
            StructuralPathSegment::Field(value) => {
                bytes.push(1);
                encode_string(bytes, value);
            }
            StructuralPathSegment::FixedIndex(value) => {
                bytes.push(2);
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
    }
}

pub(super) fn decode_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<StructuralPathSegment>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut path = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        path.push(match cursor.byte()? {
            1 => StructuralPathSegment::Field(decode_string(cursor)?),
            2 => StructuralPathSegment::FixedIndex(cursor.u64()?),
            tag => return Err(FixedViewCopyDecodeError::UnknownStructuralPathSegment(tag)),
        });
    }
    Ok(path)
}

pub(super) fn encode_multiplicity(bytes: &mut Vec<u8>, value: StructuralMultiplicity) {
    bytes.push(match value {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}

pub(super) fn decode_multiplicity(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralMultiplicity, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(StructuralMultiplicity::Unrestricted),
        2 => Ok(StructuralMultiplicity::Affine),
        3 => Ok(StructuralMultiplicity::Linear),
        tag => Err(FixedViewCopyDecodeError::UnknownStructuralMultiplicity(tag)),
    }
}

pub(super) fn encode_access(bytes: &mut Vec<u8>, value: StructuralAccess) {
    bytes.push(match value {
        StructuralAccess::Owned => 1,
        StructuralAccess::SharedBorrow => 2,
        StructuralAccess::MutableBorrow => 3,
        StructuralAccess::WriteOnlyBorrow => 4,
    });
}

pub(super) fn decode_access(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralAccess, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(StructuralAccess::Owned),
        2 => Ok(StructuralAccess::SharedBorrow),
        3 => Ok(StructuralAccess::MutableBorrow),
        4 => Ok(StructuralAccess::WriteOnlyBorrow),
        tag => Err(FixedViewCopyDecodeError::UnknownStructuralAccess(tag)),
    }
}

pub(super) fn encode_string(bytes: &mut Vec<u8>, value: &str) {
    length(bytes, value.len());
    bytes.extend_from_slice(value.as_bytes());
}

pub(super) fn decode_string(cursor: &mut Cursor<'_>) -> Result<String, FixedViewCopyDecodeError> {
    let length = cursor.length()?;
    std::str::from_utf8(cursor.take(length)?)
        .map(str::to_owned)
        .map_err(|_| FixedViewCopyDecodeError::InvalidUtf8)
}

fn decode_bool(cursor: &mut Cursor<'_>) -> Result<bool, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(FixedViewCopyDecodeError::UnknownBoolean(tag)),
    }
}

fn encode_option_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn decode_option_u64(cursor: &mut Cursor<'_>) -> Result<Option<u64>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.u64()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}

fn encode_option_u32(bytes: &mut Vec<u8>, value: Option<u32>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn decode_option_u32(cursor: &mut Cursor<'_>) -> Result<Option<u32>, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        0 => Ok(None),
        1 => Ok(Some(cursor.u32()?)),
        tag => Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    }
}
