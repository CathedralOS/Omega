use legalized_operations::LegalizedCallUnitParameter;
use selected_instructions::effects::program::identity::encode_path;
use semantic_vocabulary::{
    AffineConstructionElement, BlockId, IeeeFloatFormat, OperationId, PlaceId, StructuralCaseId,
    StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use target_operations::{
    TargetStructuralArgument, TargetStructuralParameter, TargetStructuralRuntimeIndex,
};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, EntryClaim, StructuralAccess, StructuralArgument,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralPlaceDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

use crate::FixedViewCopyDecodeError;

use super::argument_source::{decode_argument_source, encode_argument_source};
use super::calling::{decode_placement, decode_shape};
use super::projected_qualifications::{decode_projected, encode_projected};
use crate::register_homes::recovery::fixed_view_copy::codec::{
    primitives::{
        Cursor, decode_id, decode_ids, decode_option_u64, encode_ids, encode_option_u64, length,
    },
    values::{decode_bool, decode_scalar, encode_scalar},
};
use legalized_operations::{encode_placement, encode_shape};

mod bounded_integer;

pub(super) fn encode_parameter(
    bytes: &mut Vec<u8>,
    parameter: &LegalizedCallUnitParameter,
    retain_projected_qualifications: bool,
) {
    encode_semantic_parameter(bytes, &parameter.semantic, retain_projected_qualifications);
    encode_target_parameter(bytes, &parameter.target);
}

pub(super) fn decode_parameter(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<LegalizedCallUnitParameter, FixedViewCopyDecodeError> {
    Ok(LegalizedCallUnitParameter {
        semantic: decode_semantic_parameter(cursor, retain_projected_qualifications)?,
        target: decode_target_parameter(cursor)?,
    })
}

pub(super) fn encode_type(
    bytes: &mut Vec<u8>,
    declaration: &StructuralTypeDeclaration,
) -> Result<(), terminal_codec::CodecError> {
    bytes.push(u8::from(declaration.quotient_carrier.is_some()));
    if let Some(carrier) = declaration.quotient_carrier {
        crate::codec_bytes::push_encoded(bytes, carrier.get().to_le_bytes());
    }
    crate::codec_bytes::push_encoded(bytes, declaration.id.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, declaration.identity.0.to_le_bytes());
    bytes.push(u8::from(declaration.no_code_affine_cleanup));
    bytes.push(u8::from(declaration.descriptor_cleanup));
    match &declaration.shape {
        StructuralTypeShape::BindingCarrier => bytes.push(11),
        StructuralTypeShape::Reference { referent, access } => {
            bytes.push(7);
            crate::codec_bytes::push_encoded(bytes, referent.get().to_le_bytes());
            encode_access(bytes, *access);
        }
        StructuralTypeShape::PrimitiveScalar(scalar) => {
            bytes.push(if declaration.scalar_restriction.is_some() {
                12
            } else {
                6
            });
            encode_scalar(bytes, *scalar);
            if let Some(restriction) = &declaration.scalar_restriction {
                encode_scalar_restriction(bytes, restriction)?;
            }
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.push(1);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralTypeShape::ElementView { element } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
        }
        StructuralTypeShape::Record { fields, storage } => {
            match storage {
                terminal_psi::StructuralRecordStorage::Packed => bytes.push(2),
                terminal_psi::StructuralRecordStorage::Authored(storage) => {
                    bytes.push(10);
                    crate::codec_bytes::push_encoded(bytes, storage.byte_size.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, storage.byte_alignment.to_le_bytes());
                }
                terminal_psi::StructuralRecordStorage::TargetLayout(application) => {
                    bytes.push(13);
                    crate::codec_bytes::push_encoded(bytes, application.0.to_le_bytes());
                }
            }
            encode_fields(bytes, fields)?;
            if matches!(storage, terminal_psi::StructuralRecordStorage::Authored(_)) {
                for field in fields {
                    encode_authored_field_storage(bytes, field.storage.as_ref());
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, length.to_le_bytes());
        }
        StructuralTypeShape::Sum { cases } => {
            bytes.push(4);
            encode_cases(bytes, cases)?;
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            bytes.push(5);
            encode_fields(bytes, fields)?;
            encode_cases(bytes, cases)?;
        }
    }
    Ok(())
}

pub(super) fn decode_type(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralTypeDeclaration, FixedViewCopyDecodeError> {
    let quotient_carrier = if decode_bool(cursor)? {
        Some(decode_id(cursor, StructuralTypeId::new)?)
    } else {
        None
    };
    let id = decode_id(cursor, StructuralTypeId::new)?;
    let identity = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
    let no_code_affine_cleanup = decode_bool(cursor)?;
    let descriptor_cleanup = decode_bool(cursor)?;
    let mut scalar_restriction = None;
    let shape = match cursor.byte()? {
        11 => StructuralTypeShape::BindingCarrier,
        7 => StructuralTypeShape::Reference {
            referent: decode_id(cursor, StructuralTypeId::new)?,
            access: decode_access(cursor)?,
        },
        1 => StructuralTypeShape::ByteSequence(decode_byte_sequence_carrier(cursor)?),
        2 => StructuralTypeShape::Record {
            storage: terminal_psi::StructuralRecordStorage::Packed,
            fields: decode_fields(cursor)?,
        },
        13 => StructuralTypeShape::Record {
            storage: terminal_psi::StructuralRecordStorage::TargetLayout(
                terminal_psi::LayoutApplicationId(cursor.u32()?),
            ),
            fields: decode_fields(cursor)?,
        },
        10 => {
            let storage = semantic_vocabulary::AuthoredRecordStorage {
                byte_size: cursor.u64()?,
                byte_alignment: cursor.u64()?,
            };
            let mut fields = decode_fields(cursor)?;
            for field in &mut fields {
                field.storage = decode_authored_field_storage(cursor)?;
            }
            StructuralTypeShape::Record {
                fields,
                storage: terminal_psi::StructuralRecordStorage::Authored(storage),
            }
        }
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
        12 => {
            let scalar = decode_scalar(cursor)?;
            let restriction = decode_scalar_restriction(cursor)?;
            if !matches!(
                restriction,
                StructuralFieldType::BoundedInteger(_) | StructuralFieldType::RefinedScalar(_)
            ) || restriction.scalar_type() != Some(scalar)
            {
                return Err(FixedViewCopyDecodeError::InvalidScalarRefinement);
            }
            scalar_restriction = Some(restriction);
            StructuralTypeShape::PrimitiveScalar(scalar)
        }
        tag => return Err(FixedViewCopyDecodeError::UnknownStructuralTypeShape(tag)),
    };
    Ok(StructuralTypeDeclaration {
        quotient_carrier,
        no_code_affine_cleanup,
        descriptor_cleanup,
        scalar_restriction,
        id,
        identity,
        shape,
    })
}

fn encode_authored_field_storage(
    bytes: &mut Vec<u8>,
    storage: Option<&semantic_vocabulary::AuthoredFieldStorage>,
) {
    use semantic_vocabulary::AuthoredFieldStorage;
    match storage {
        None => bytes.push(0),
        Some(AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar,
        }) => {
            bytes.push(if stored_scalar.is_some() { 2 } else { 1 });
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            if let Some(scalar) = stored_scalar {
                encode_scalar(bytes, *scalar);
            }
        }
        Some(AuthoredFieldStorage::Repeated {
            byte_offset,
            element_stride,
        }) => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, element_stride.to_le_bytes());
        }
        Some(AuthoredFieldStorage::Direct { byte_offset }) => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        Some(AuthoredFieldStorage::Fragmented(recipe)) => {
            bytes.push(6);
            length(bytes, recipe.fragments().len());
            for fragment in recipe.fragments() {
                crate::codec_bytes::push_encoded(
                    bytes,
                    fragment.container_byte_offset.to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(
                    bytes,
                    fragment.container_width_bits.to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(bytes, fragment.destination_lsb.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, fragment.source_lsb.to_le_bytes());
                crate::codec_bytes::push_encoded(bytes, fragment.width.to_le_bytes());
            }
        }
    }
}

fn decode_authored_field_storage(
    cursor: &mut Cursor<'_>,
) -> Result<Option<semantic_vocabulary::AuthoredFieldStorage>, FixedViewCopyDecodeError> {
    use semantic_vocabulary::AuthoredFieldStorage;
    Ok(match cursor.byte()? {
        0 => None,
        1 => Some(AuthoredFieldStorage::Scalar {
            byte_offset: cursor.u64()?,
            stored_scalar: None,
        }),
        2 => Some(AuthoredFieldStorage::Scalar {
            byte_offset: cursor.u64()?,
            stored_scalar: Some(decode_scalar(cursor)?),
        }),
        3 => Some(AuthoredFieldStorage::Repeated {
            byte_offset: cursor.u64()?,
            element_stride: cursor.u64()?,
        }),
        4 => Some(AuthoredFieldStorage::Direct {
            byte_offset: cursor.u64()?,
        }),
        6 => {
            let count = cursor.length()?;
            if count == 0 || count > cursor.remaining() / 40 {
                return Err(FixedViewCopyDecodeError::UnknownOption(6));
            }
            let mut fragments = Vec::with_capacity(count);
            for _ in 0..count {
                fragments.push(semantic_vocabulary::AuthoredBitFragment {
                    container_byte_offset: cursor.u64()?,
                    container_width_bits: cursor.u64()?,
                    destination_lsb: cursor.u64()?,
                    source_lsb: cursor.u64()?,
                    width: cursor.u64()?,
                });
            }
            Some(AuthoredFieldStorage::Fragmented(
                semantic_vocabulary::AuthoredBitFragments::new(fragments),
            ))
        }
        tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
    })
}

fn encode_cases(
    bytes: &mut Vec<u8>,
    cases: &[StructuralCaseDeclaration],
) -> Result<(), terminal_codec::CodecError> {
    length(bytes, cases.len());
    for case in cases {
        crate::codec_bytes::push_encoded(bytes, case.id.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, case.identity.0.to_le_bytes());
        encode_fields(bytes, &case.fields)?;
    }
    Ok(())
}

fn decode_cases(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<StructuralCaseDeclaration>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut cases = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        cases.push(StructuralCaseDeclaration {
            id: decode_id(cursor, StructuralCaseId::new)?,
            identity: terminal_psi::TerminalIdentityIndex(cursor.u32()?),
            fields: decode_fields(cursor)?,
        });
    }
    Ok(cases)
}

fn encode_fields(
    bytes: &mut Vec<u8>,
    fields: &[StructuralFieldDeclaration],
) -> Result<(), terminal_codec::CodecError> {
    length(bytes, fields.len());
    for field in fields {
        crate::codec_bytes::push_encoded(bytes, field.id.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, field.identity.0.to_le_bytes());
        bytes.push(match field.relevance {
            BindingRelevance::Relevant => 1,
            BindingRelevance::Erased => 2,
        });
        encode_scalar_restriction(bytes, &field.field_type)?;
    }
    Ok(())
}

fn decode_fields(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<StructuralFieldDeclaration>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut fields = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let id = decode_id(cursor, StructuralFieldId::new)?;
        let identity = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
        let relevance = match cursor.byte()? {
            1 => BindingRelevance::Relevant,
            2 => BindingRelevance::Erased,
            tag => return Err(FixedViewCopyDecodeError::UnknownBindingRelevance(tag)),
        };
        let field_type = decode_scalar_restriction(cursor)?;
        fields.push(StructuralFieldDeclaration {
            storage: None,
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
        // The view's access follows its tag; zero is the absent access of a
        // standalone view type.
        ByteSequenceCarrier::BorrowedView { access } => {
            bytes.push(1);
            match access {
                Some(access) => encode_access(bytes, access),
                None => bytes.push(0),
            }
        }
        ByteSequenceCarrier::BoundedView { access, capacity } => {
            bytes.push(3);
            match access {
                Some(access) => encode_access(bytes, access),
                None => bytes.push(0),
            }
            crate::codec_bytes::push_encoded(bytes, capacity.to_le_bytes());
        }
        ByteSequenceCarrier::BoundedOwned { capacity } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, capacity.to_le_bytes());
        }
    }
}

fn decode_byte_sequence_carrier(
    cursor: &mut Cursor<'_>,
) -> Result<ByteSequenceCarrier, FixedViewCopyDecodeError> {
    match cursor.byte()? {
        1 => Ok(ByteSequenceCarrier::BorrowedView {
            access: match cursor.byte()? {
                0 => None,
                tag => Some(access_from_tag(tag)?),
            },
        }),
        2 => Ok(ByteSequenceCarrier::BoundedOwned {
            capacity: cursor.u64()?,
        }),
        3 => Ok(ByteSequenceCarrier::BoundedView {
            access: match cursor.byte()? {
                0 => None,
                tag => Some(access_from_tag(tag)?),
            },
            capacity: cursor.u64()?,
        }),
        tag => Err(FixedViewCopyDecodeError::UnknownByteSequenceCarrier(tag)),
    }
}

pub(super) fn encode_semantic_parameter(
    bytes: &mut Vec<u8>,
    parameter: &StructuralParameterDeclaration,
    retain_projected_qualifications: bool,
) {
    crate::codec_bytes::push_encoded(bytes, parameter.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, parameter.position.to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, parameter.structural_type.get().to_le_bytes());
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

pub(super) fn decode_semantic_parameter(
    cursor: &mut Cursor<'_>,
    retain_projected_qualifications: bool,
) -> Result<StructuralParameterDeclaration, FixedViewCopyDecodeError> {
    Ok(StructuralParameterDeclaration {
        place: decode_id(cursor, PlaceId::new)?,
        position: cursor.u32()?,
        structural_type: decode_id(cursor, StructuralTypeId::new)?,
        multiplicity: decode_multiplicity(cursor)?,
        access: decode_access(cursor)?,
        qualifications: decode_ids(cursor, semantic_vocabulary::StructuralDomainId::new)?,
        projected_qualifications: decode_projected(cursor, retain_projected_qualifications)?,
    })
}

fn encode_target_parameter(bytes: &mut Vec<u8>, parameter: &TargetStructuralParameter) {
    crate::codec_bytes::push_encoded(bytes, parameter.place.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, parameter.structural_type.get().to_le_bytes());
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

pub(in crate::register_homes::recovery::fixed_view_copy::codec::selected) fn encode_semantic_argument(
    bytes: &mut Vec<u8>,
    argument: &StructuralArgument,
) {
    crate::codec_bytes::push_encoded(bytes, argument.place.get().to_le_bytes());
    encode_path(bytes, &argument.path);
    encode_access(bytes, argument.access);
}

pub(in crate::register_homes::recovery::fixed_view_copy::codec::selected) fn decode_semantic_argument(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralArgument, FixedViewCopyDecodeError> {
    Ok(StructuralArgument {
        place: decode_id(cursor, PlaceId::new)?,
        path: decode_path(cursor)?,
        access: decode_access(cursor)?,
    })
}

pub(super) fn encode_target_argument(bytes: &mut Vec<u8>, argument: &TargetStructuralArgument) {
    crate::codec_bytes::push_encoded(bytes, argument.place.get().to_le_bytes());
    encode_access(bytes, argument.access);
    encode_path(bytes, &argument.path);
    crate::codec_bytes::push_encoded(bytes, argument.root_structural_type.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, argument.structural_type.get().to_le_bytes());
    encode_shape(bytes, argument.shape);
    crate::codec_bytes::push_encoded(bytes, argument.source_byte_offset.to_le_bytes());
    encode_option_u64(bytes, argument.fixed_array_length);
    encode_option_u32(bytes, argument.element_stride);
    bytes.extend_from_slice(
        &u32::try_from(argument.indices.len())
            .unwrap_or(u32::MAX)
            .to_le_bytes(),
    );
    for index in &argument.indices {
        super::normalized_foreign::encode_scalar_source(bytes, index.operand);
        crate::codec_bytes::push_encoded(bytes, index.stride.to_le_bytes());
    }
    encode_argument_source(bytes, &argument.source);
    encode_placement(bytes, &argument.destination);
}

pub(super) fn decode_target_argument(
    cursor: &mut Cursor<'_>,
) -> Result<TargetStructuralArgument, FixedViewCopyDecodeError> {
    let place = decode_id(cursor, PlaceId::new)?;
    let access = decode_access(cursor)?;
    let path = decode_path(cursor)?;
    let root_structural_type = decode_id(cursor, StructuralTypeId::new)?;
    let structural_type = decode_id(cursor, StructuralTypeId::new)?;
    let shape = decode_shape(cursor)?;
    let source_byte_offset = cursor.u32()?;
    let fixed_array_length = decode_option_u64(cursor)?;
    let element_stride = decode_option_u32(cursor)?;
    let indices = (0..cursor.u32()?)
        .map(|_| {
            Ok(TargetStructuralRuntimeIndex {
                operand: super::normalized_foreign::decode_scalar_source(cursor)?,
                stride: cursor.u32()?,
            })
        })
        .collect::<Result<Vec<_>, FixedViewCopyDecodeError>>()?;
    Ok(TargetStructuralArgument {
        place,
        access,
        path,
        root_structural_type,
        structural_type,
        shape,
        source_byte_offset,
        fixed_array_length,
        element_stride,
        indices,
        source: decode_argument_source(cursor)?,
        destination: decode_placement(cursor)?,
    })
}

pub(super) fn encode_place(bytes: &mut Vec<u8>, place: StructuralPlaceDeclaration) {
    crate::codec_bytes::push_encoded(bytes, place.id.get().to_le_bytes());
    match place.kind {
        StructuralPlaceKind::Parameter { position } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        StructuralPlaceKind::Result => bytes.push(2),
        StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, producer.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            structural_type,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, declaration_ordinal.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
        }
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, attachment.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, field.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, boundary.get().to_le_bytes());
        }
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction,
        } => {
            bytes.push(if construction.is_some() { 7 } else { 6 });
            crate::codec_bytes::push_encoded(bytes, declaration_ordinal.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
            if let Some(construction) = construction {
                crate::codec_bytes::push_encoded(
                    bytes,
                    construction.root_structural_type.get().to_le_bytes(),
                );
                crate::codec_bytes::push_encoded(bytes, construction.index.to_le_bytes());
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
            boundary: decode_id(cursor, semantic_vocabulary::BoundaryMachineId::new)?,
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
        8 => StructuralPlaceKind::BlockParameter {
            block: decode_id(cursor, BlockId::new)?,
            position: cursor.u32()?,
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownStructuralPlaceKind(tag)),
    };
    Ok(StructuralPlaceDeclaration { id, kind })
}

pub(super) fn encode_entry_claim(bytes: &mut Vec<u8>, claim: &EntryClaim) {
    crate::codec_bytes::push_encoded(bytes, claim.claim.get().to_le_bytes());
    crate::codec_bytes::push_encoded(bytes, claim.input.get().to_le_bytes());
    encode_path(bytes, &claim.path);
}

pub(super) fn decode_entry_claim(
    cursor: &mut Cursor<'_>,
) -> Result<EntryClaim, FixedViewCopyDecodeError> {
    Ok(EntryClaim {
        claim: decode_id(cursor, semantic_vocabulary::ClaimId::new)?,
        input: decode_id(cursor, PlaceId::new)?,
        path: decode_path(cursor)?,
    })
}

pub(in super::super) fn decode_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<StructuralPathSegment>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut path = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let tag = cursor.byte()?;
        path.push(decode_projection(cursor, tag)?);
    }
    Ok(path)
}

pub(super) fn decode_reference_origin_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<terminal_psi::ReferenceOriginProjection>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut path = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let tag = cursor.byte()?;
        path.push(if tag == 9 {
            terminal_psi::ReferenceOriginProjection::Element
        } else {
            terminal_psi::ReferenceOriginProjection::Exact(decode_projection(cursor, tag)?)
        });
    }
    Ok(path)
}

fn decode_projection(
    cursor: &mut Cursor<'_>,
    tag: u8,
) -> Result<StructuralPathSegment, FixedViewCopyDecodeError> {
    Ok(match tag {
        1 => StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(cursor.u32()?)),
        2 => StructuralPathSegment::FixedIndex(cursor.u64()?),
        3 => StructuralPathSegment::Referent,
        7 => StructuralPathSegment::Case(decode_id(
            cursor,
            semantic_vocabulary::StructuralCaseId::new,
        )?),
        4 => StructuralPathSegment::FixedByteRange {
            start: cursor.u64()?,
            end: cursor.u64()?,
        },
        6 => StructuralPathSegment::RuntimeIndex {
            index: decode_id(cursor, semantic_vocabulary::ValueId::new)?,
            obligation: decode_id(cursor, semantic_vocabulary::ObligationId::new)?,
        },
        8 => StructuralPathSegment::RepresentationView {
            structural_type: decode_id(cursor, StructuralTypeId::new)?,
            stored_scalar: if decode_bool(cursor)? {
                Some(decode_scalar(cursor)?)
            } else {
                None
            },
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownStructuralPathSegment(tag)),
    })
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
    access_from_tag(cursor.byte()?)
}

fn access_from_tag(tag: u8) -> Result<StructuralAccess, FixedViewCopyDecodeError> {
    match tag {
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

fn encode_option_u32(bytes: &mut Vec<u8>, value: Option<u32>) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
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

#[cfg(test)]
mod tests;

fn encode_scalar_restriction(
    bytes: &mut Vec<u8>,
    field_type: &StructuralFieldType,
) -> Result<(), terminal_codec::CodecError> {
    match field_type {
        StructuralFieldType::RefinedScalar(refinement) => {
            bytes.push(9);
            encode_scalar(bytes, refinement.carrier());
            let predicate =
                terminal_codec::canonical_proposition_order_key(refinement.predicate())?;
            length(bytes, predicate.len());
            bytes.extend_from_slice(&predicate);
        }
        StructuralFieldType::Atomic(scalar) => {
            bytes.push(8);
            encode_scalar(bytes, *scalar);
        }
        StructuralFieldType::BoundedInteger(bounds) => {
            bytes.push(6);
            bounded_integer::encode(bytes, *bounds);
        }
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
        StructuralFieldType::PredicateByteSequence {
            carrier,
            predicates,
        } => {
            bytes.push(7);
            encode_byte_sequence_carrier(bytes, *carrier);
            bytes.push(predicates.bits());
        }
        StructuralFieldType::ByteSequence(carrier) => {
            bytes.push(3);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralFieldType::Structural(structural_type) => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, structural_type.get().to_le_bytes());
        }
        StructuralFieldType::Erased { type_identity } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, type_identity.0.to_le_bytes());
        }
    }
    Ok(())
}

fn decode_scalar_restriction(
    cursor: &mut Cursor<'_>,
) -> Result<StructuralFieldType, FixedViewCopyDecodeError> {
    Ok(match cursor.byte()? {
        9 => {
            let carrier = decode_scalar(cursor)?;
            let length = cursor.length()?;
            let predicate = terminal_codec::decode_canonical_proposition(cursor.take(length)?)
                .map_err(|_| FixedViewCopyDecodeError::InvalidScalarRefinement)?;
            StructuralFieldType::RefinedScalar(
                semantic_vocabulary::ScalarRefinement::new(carrier, predicate)
                    .map_err(|_| FixedViewCopyDecodeError::InvalidScalarRefinement)?,
            )
        }
        8 => {
            let field = StructuralFieldType::Atomic(decode_scalar(cursor)?);
            field
                .atomic_scalar_type()
                .ok_or(FixedViewCopyDecodeError::UnknownStructuralFieldType(8))?;
            field
        }
        6 => StructuralFieldType::BoundedInteger(bounded_integer::decode(cursor)?),
        1 => StructuralFieldType::Scalar(decode_scalar(cursor)?),
        2 => StructuralFieldType::IeeeFloat(match cursor.byte()? {
            1 => IeeeFloatFormat::Binary32,
            2 => IeeeFloatFormat::Binary64,
            tag => return Err(FixedViewCopyDecodeError::UnknownIeeeFloatFormat(tag)),
        }),
        3 => StructuralFieldType::ByteSequence(decode_byte_sequence_carrier(cursor)?),
        7 => {
            let carrier = decode_byte_sequence_carrier(cursor)?;
            let bits = cursor.byte()?;
            let predicates = terminal_psi::ByteSequencePredicateSet::from_bits(bits)
                .filter(|predicates| !predicates.is_empty())
                .ok_or(FixedViewCopyDecodeError::InvalidVocabulary(u16::from(bits)))?;
            StructuralFieldType::PredicateByteSequence {
                carrier,
                predicates,
            }
        }
        4 => StructuralFieldType::Structural(decode_id(cursor, StructuralTypeId::new)?),
        5 => StructuralFieldType::Erased {
            type_identity: terminal_psi::TerminalIdentityIndex(cursor.u32()?),
        },
        tag => return Err(FixedViewCopyDecodeError::UnknownStructuralFieldType(tag)),
    })
}
