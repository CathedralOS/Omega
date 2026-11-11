/*
We encode and decode structural type, parameter, argument, place, path, and
qualification declarations inside the `FixedViewCopyPlan` selected payload.

Each structural type declaration preserves its quotient carrier, declaration-
owned linearity, no-code affine cleanup authority, descriptor cleanup, logical
copyability, scalar refinement predicates, and record/array/sum/mixed/reference/
element-view/byte-sequence/atomic-cell shapes. Parameter-rooted qualification
paths (`decode_qualification_path`) admit `AllElements` wildcards as retained
metadata while ordinary address paths (`decode_path`) and reference origin paths
(`decode_reference_origin_path`) reject wildcards.
*/

pub(super) use legalized_operations::identity::encode_structural_place as encode_place;

use ::selected_instructions::effects::program::identity::encode_path;
use legalized_operations::identity::encode_authored_field_storage;
use legalized_operations::{
    LegalizedCallUnitParameter, encode_integer, encode_placement, encode_shape,
};
use semantic_vocabulary::{
    AffineConstructionElement, BlockId, BoundedIntegerType, IeeeFloatFormat, OperationId, PlaceId,
    ScalarType, StructuralCaseId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use target_operations::{
    TargetStructuralArgument, TargetStructuralParameter, TargetStructuralRuntimeIndex,
};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, EntryClaim, StructuralAccess, StructuralArgument,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPathQualification,
    StructuralPathSegment, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use crate::FixedViewCopyDecodeError;

use super::fixed_view_copy_codec::{
    Cursor, decode_bool, decode_id, decode_ids, decode_integer, decode_list, decode_option_u64,
    decode_scalar, encode_ids, encode_option_u64, encode_scalar, length,
};
use super::structural_calls::{
    decode_argument_source, decode_placement, decode_scalar_source, decode_shape,
    encode_argument_source, encode_scalar_source,
};

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
    bytes.push(u8::from(declaration.owns_linear_claim));
    bytes.push(u8::from(declaration.no_code_affine_cleanup));
    bytes.push(u8::from(declaration.descriptor_cleanup));
    bytes.push(u8::from(declaration.copyable));
    match &declaration.shape {
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
        StructuralTypeShape::AtomicCell(cell) => {
            bytes.push(16);
            encode_scalar_restriction(bytes, &(*cell).into())?;
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            bytes.push(1);
            encode_byte_sequence_carrier(bytes, *carrier);
        }
        StructuralTypeShape::ElementView { element, access } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
            bytes.push(u8::from(access.is_some()));
            if let Some(access) = access {
                encode_access(bytes, *access);
            }
        }
        StructuralTypeShape::Record { fields, storage } => {
            match storage {
                terminal_psi::StructuralRecordStorage::AccessView(storage) => {
                    bytes.push(14);
                    crate::codec_bytes::push_encoded(bytes, storage.byte_size.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, storage.byte_alignment.to_le_bytes());
                }
                terminal_psi::StructuralRecordStorage::Accessor(storage) => {
                    bytes.push(15);
                    crate::codec_bytes::push_encoded(bytes, storage.byte_size.to_le_bytes());
                    crate::codec_bytes::push_encoded(bytes, storage.byte_alignment.to_le_bytes());
                }
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
            if matches!(
                storage,
                terminal_psi::StructuralRecordStorage::Authored(_)
                    | terminal_psi::StructuralRecordStorage::AccessView(_)
                    | terminal_psi::StructuralRecordStorage::Accessor(_)
            ) {
                for field in fields {
                    encode_authored_field_storage(bytes, field.storage.as_ref());
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, element.get().to_le_bytes());
            match length {
                terminal_psi::ArrayExtent::Literal(length) => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(bytes, length.to_le_bytes());
                }
                terminal_psi::ArrayExtent::Application(application) => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(bytes, application.get().to_le_bytes());
                }
            }
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
    let owns_linear_claim = decode_bool(cursor)?;
    let no_code_affine_cleanup = decode_bool(cursor)?;
    let descriptor_cleanup = decode_bool(cursor)?;
    let copyable = decode_bool(cursor)?;
    let mut scalar_restriction = None;
    let shape = match cursor.byte()? {
        7 => StructuralTypeShape::Reference {
            referent: decode_id(cursor, StructuralTypeId::new)?,
            access: decode_access(cursor)?,
        },
        8 => StructuralTypeShape::ElementView {
            element: decode_id(cursor, StructuralTypeId::new)?,
            access: if decode_bool(cursor)? {
                Some(decode_access(cursor)?)
            } else {
                None
            },
        },
        1 => StructuralTypeShape::ByteSequence(decode_byte_sequence_carrier(cursor)?),
        2 => StructuralTypeShape::Record {
            storage: terminal_psi::StructuralRecordStorage::Packed,
            fields: decode_fields(cursor)?,
        },
        tag @ (14 | 15) => {
            let geometry = semantic_vocabulary::AuthoredRecordStorage {
                byte_size: cursor.u64()?,
                byte_alignment: cursor.u64()?,
            };
            let mut fields = decode_fields(cursor)?;
            for field in &mut fields {
                field.storage = decode_authored_field_storage(cursor)?;
            }
            StructuralTypeShape::Record {
                fields,
                storage: if tag == 14 {
                    terminal_psi::StructuralRecordStorage::AccessView(geometry)
                } else {
                    terminal_psi::StructuralRecordStorage::Accessor(geometry)
                },
            }
        }
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
            length: match cursor.byte()? {
                0 => terminal_psi::ArrayExtent::Literal(cursor.u64()?),
                1 => terminal_psi::ArrayExtent::Application(decode_id(
                    cursor,
                    semantic_vocabulary::ScalarApplicationId::new,
                )?),
                tag => return Err(FixedViewCopyDecodeError::UnknownArrayExtent(tag)),
            },
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
        16 => {
            let cell = decode_scalar_restriction(cursor)?
                .atomic_cell_type()
                .filter(|cell| cell.is_supported())
                .ok_or(FixedViewCopyDecodeError::InvalidScalarRefinement)?;
            StructuralTypeShape::AtomicCell(cell)
        }
        tag => return Err(FixedViewCopyDecodeError::UnknownStructuralTypeShape(tag)),
    };
    Ok(StructuralTypeDeclaration {
        requires: Vec::new(),
        quotient_carrier,
        owns_linear_claim,
        no_code_affine_cleanup,
        copyable,
        descriptor_cleanup,
        scalar_restriction,
        id,
        identity,
        shape,
    })
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
    decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        Ok(StructuralCaseDeclaration {
            id: decode_id(cursor, StructuralCaseId::new)?,
            identity: terminal_psi::TerminalIdentityIndex(cursor.u32()?),
            fields: decode_fields(cursor)?,
        })
    })
}

fn encode_fields(
    bytes: &mut Vec<u8>,
    fields: &[StructuralFieldDeclaration],
) -> Result<(), terminal_codec::CodecError> {
    length(bytes, fields.len());
    for field in fields {
        crate::codec_bytes::push_encoded(bytes, field.id.get().to_le_bytes());
        crate::codec_bytes::push_encoded(bytes, field.identity.0.to_le_bytes());
        encode_option_u64(bytes, field.inline_carrier.map(|carrier| carrier.get()));
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
    decode_list(cursor, |cursor| {
        let id = decode_id(cursor, StructuralFieldId::new)?;
        let identity = terminal_psi::TerminalIdentityIndex(cursor.u32()?);
        let inline_carrier = match cursor.byte()? {
            0 => None,
            1 => Some(decode_id(cursor, StructuralTypeId::new)?),
            tag => return Err(FixedViewCopyDecodeError::UnknownOption(tag)),
        };
        let relevance = match cursor.byte()? {
            1 => BindingRelevance::Relevant,
            2 => BindingRelevance::Erased,
            tag => return Err(FixedViewCopyDecodeError::UnknownBindingRelevance(tag)),
        };
        let field_type = decode_scalar_restriction(cursor)?;
        Ok(StructuralFieldDeclaration {
            inline_carrier,
            storage: None,
            id,
            identity,
            relevance,
            field_type,
        })
    })
}

fn encode_byte_sequence_carrier(bytes: &mut Vec<u8>, carrier: ByteSequenceCarrier) {
    match carrier {
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
    encode_projected(bytes, &parameter.additional_input_qualifications, true);
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
        additional_input_qualifications: decode_projected(cursor, true)?,
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

pub(super) fn encode_semantic_argument(bytes: &mut Vec<u8>, argument: &StructuralArgument) {
    crate::codec_bytes::push_encoded(bytes, argument.place.get().to_le_bytes());
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
        encode_scalar_source(bytes, index.operand);
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
                operand: decode_scalar_source(cursor)?,
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

pub(super) fn decode_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<StructuralPathSegment>, FixedViewCopyDecodeError> {
    decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        let tag = cursor.byte()?;
        decode_projection(cursor, tag)
    })
}

pub(super) fn decode_qualification_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<StructuralPathSegment>, FixedViewCopyDecodeError> {
    let count = cursor.length()?;
    let mut path = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let tag = cursor.byte()?;
        path.push(if tag == 10 {
            StructuralPathSegment::AllElements
        } else {
            decode_projection(cursor, tag)?
        });
    }
    Ok(path)
}

pub(super) fn decode_reference_origin_path(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<terminal_psi::ReferenceOriginProjection>, FixedViewCopyDecodeError> {
    decode_list::<_, FixedViewCopyDecodeError>(cursor, |cursor| {
        let tag = cursor.byte()?;
        Ok(if tag == 9 {
            terminal_psi::ReferenceOriginProjection::Element
        } else {
            terminal_psi::ReferenceOriginProjection::Exact(decode_projection(cursor, tag)?)
        })
    })
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
        4 => StructuralPathSegment::FixedRange {
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

pub(super) fn encode_projected(
    bytes: &mut Vec<u8>,
    qualifications: &[StructuralPathQualification],
    retain: bool,
) {
    if !retain {
        return;
    }
    length(bytes, qualifications.len());
    for qualification in qualifications {
        encode_path(bytes, &qualification.path);
        crate::codec_bytes::push_encoded(bytes, qualification.domain.get().to_le_bytes());
    }
}

pub(super) fn decode_projected(
    cursor: &mut Cursor<'_>,
    retain: bool,
) -> Result<Vec<StructuralPathQualification>, FixedViewCopyDecodeError> {
    if !retain {
        return Ok(Vec::new());
    }
    decode_list(cursor, |cursor| {
        Ok(StructuralPathQualification {
            path: decode_qualification_path(cursor)?,
            domain: decode_id(cursor, semantic_vocabulary::StructuralDomainId::new)?,
        })
    })
}

fn encode_bounded_integer(bytes: &mut Vec<u8>, bounds: BoundedIntegerType) {
    encode_scalar(bytes, ScalarType::Integer(bounds.integer_type()));
    encode_integer(bytes, bounds.minimum());
    encode_integer(bytes, bounds.maximum());
}

fn decode_bounded_integer(
    cursor: &mut Cursor<'_>,
) -> Result<BoundedIntegerType, FixedViewCopyDecodeError> {
    let Some(integer) = decode_scalar(cursor)?.as_integer() else {
        return Err(FixedViewCopyDecodeError::UnknownStructuralFieldType(6));
    };
    let minimum = decode_integer(cursor)?;
    let maximum = decode_integer(cursor)?;
    BoundedIntegerType::new(integer, minimum, maximum)
        .map_err(|_| FixedViewCopyDecodeError::UnknownStructuralFieldType(6))
}

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
        StructuralFieldType::AtomicBoundedInteger(bounds) => {
            bytes.push(10);
            encode_bounded_integer(bytes, *bounds);
        }
        StructuralFieldType::Atomic(scalar) => {
            bytes.push(8);
            encode_scalar(bytes, *scalar);
        }
        StructuralFieldType::BoundedInteger(bounds) => {
            bytes.push(6);
            encode_bounded_integer(bytes, *bounds);
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
        10 => {
            let field = StructuralFieldType::AtomicBoundedInteger(decode_bounded_integer(cursor)?);
            field
                .atomic_scalar_type()
                .ok_or(FixedViewCopyDecodeError::UnknownStructuralFieldType(10))?;
            field
        }
        6 => StructuralFieldType::BoundedInteger(decode_bounded_integer(cursor)?),
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

#[cfg(test)]
mod tests {
    use super::{
        BlockId, ByteSequenceCarrier, Cursor, FixedViewCopyDecodeError, PlaceId, StructuralAccess,
        StructuralMultiplicity, StructuralPathQualification, StructuralPathSegment,
        StructuralPlaceDeclaration, StructuralPlaceKind, StructuralTypeDeclaration,
        StructuralTypeId, StructuralTypeShape, decode_path, decode_place,
        decode_qualification_path, decode_reference_origin_path, decode_type, encode_path,
        encode_place, encode_type,
    };
    use crate::recovery::structural_calls::{decode_provider, encode_provider};
    use semantic_vocabulary::{BoundaryMachineId, MachineId, StructuralDomainId};
    use terminal_codec::canonical_proposition_order_key;
    use terminal_psi::{
        NominalDeclarationIdentity, ProviderCandidateConformance, ProviderParameterRefinement,
        ProviderRefinement, ProviderSignature, ProviderSignatureParameter,
        ReferenceOriginProjection, StructuralDomainRequirement, TerminalIdentityIndex,
    };

    #[test]
    fn logical_copy_permission_changes_encoded_identity() {
        let mut encodings = Vec::new();
        for copyable in [false, true] {
            let declaration = StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                descriptor_cleanup: true,
                copyable,
                scalar_restriction: None,
                id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(1),
                shape: StructuralTypeShape::Record {
                    fields: Vec::new(),
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                },
            };
            let mut bytes = Vec::new();
            encode_type(&mut bytes, &declaration).unwrap();
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
            assert_eq!(cursor.remaining(), 0);
            assert!(!encodings.contains(&bytes));
            encodings.push(bytes);
        }
    }

    #[test]
    fn declaration_owned_linearity_round_trips_and_changes_bytes() {
        let mut encodings = Vec::new();
        for owns_linear_claim in [false, true] {
            let declaration = StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(1),
                shape: StructuralTypeShape::Record {
                    fields: Vec::new(),
                    storage: terminal_psi::StructuralRecordStorage::Packed,
                },
            };
            let mut bytes = Vec::new();
            encode_type(&mut bytes, &declaration).unwrap();
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
            assert_eq!(cursor.remaining(), 0);
            assert!(!encodings.contains(&bytes));
            encodings.push(bytes);
        }
    }

    #[test]
    fn element_view_encoding_round_trips_each_access_ceiling() {
        let mut encodings = Vec::new();
        for access in [
            None,
            Some(StructuralAccess::SharedBorrow),
            Some(StructuralAccess::MutableBorrow),
            Some(StructuralAccess::WriteOnlyBorrow),
        ] {
            let declaration = StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: true,
                scalar_restriction: None,
                id: StructuralTypeId::new(2).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(1),
                shape: StructuralTypeShape::ElementView {
                    element: StructuralTypeId::new(1).unwrap(),
                    access,
                },
            };
            let mut bytes = Vec::new();
            encode_type(&mut bytes, &declaration).unwrap();
            assert!(!encodings.contains(&bytes));
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
            assert_eq!(cursor.remaining(), 0);
            encodings.push(bytes);
        }
    }

    #[test]
    fn scalar_refinement_round_trip_retains_full_predicate_and_rejects_foreign_subject() {
        use semantic_vocabulary::{Proposition, ScalarRefinement, ScalarTerm, ScalarType, ValueId};
        use terminal_psi::{BindingRelevance, StructuralFieldDeclaration, StructuralFieldType};
        let predicate = Proposition::Equal(
            ScalarTerm::value(ScalarRefinement::SUBJECT, ScalarType::Boolean),
            ScalarTerm::boolean(true),
        );
        let declaration = StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: StructuralTypeId::new(7).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(1),
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: vec![StructuralFieldDeclaration {
                    inline_carrier: Some(StructuralTypeId::new(8).unwrap()),
                    storage: None,
                    id: semantic_vocabulary::StructuralFieldId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(2),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::RefinedScalar(
                        ScalarRefinement::new(ScalarType::Boolean, predicate.clone()).unwrap(),
                    ),
                }],
            },
        };
        let mut bytes = Vec::new();
        encode_type(&mut bytes, &declaration).unwrap();
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
        assert_eq!(cursor.remaining(), 0);
        let predicate_length = canonical_proposition_order_key(&predicate).unwrap().len();
        let mut foreign = predicate;
        foreign.remap_value_ids(|_| ValueId::new(2).unwrap());
        let foreign = canonical_proposition_order_key(&foreign).unwrap();
        bytes.truncate(bytes.len() - predicate_length - 8);
        bytes.extend_from_slice(&u64::try_from(foreign.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&foreign);
        assert_eq!(
            decode_type(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::InvalidScalarRefinement)
        );
    }

    #[test]
    fn retired_binding_carrier_tag_decodes_as_an_unknown_shape() {
        let record = StructuralTypeDeclaration {
            requires: Vec::new(),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: false,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            id: StructuralTypeId::new(7).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(3),
            shape: StructuralTypeShape::Record {
                storage: terminal_psi::StructuralRecordStorage::Packed,
                fields: Vec::new(),
            },
        };
        let mut reference = record.clone();
        reference.shape = StructuralTypeShape::Reference {
            referent: StructuralTypeId::new(7).unwrap(),
            access: StructuralAccess::SharedBorrow,
        };
        let mut record_bytes = Vec::new();
        encode_type(&mut record_bytes, &record).unwrap();
        let mut reference_bytes = Vec::new();
        encode_type(&mut reference_bytes, &reference).unwrap();
        let header = record_bytes
            .iter()
            .zip(&reference_bytes)
            .take_while(|(left, right)| left == right)
            .count();
        let mut retired = record_bytes[..header].to_vec();
        retired.push(11);
        assert_eq!(
            decode_type(&mut Cursor::new(&retired)),
            Err(FixedViewCopyDecodeError::UnknownStructuralTypeShape(11)),
        );
    }

    #[test]
    fn fixed_byte_range_path_round_trip_retains_exact_endpoints_and_rejects_truncation() {
        for (start, end) in [(0, 0), (1, 3), (2, 4), (u64::MAX - 1, u64::MAX)] {
            let path = vec![StructuralPathSegment::FixedRange { start, end }];
            let mut bytes = Vec::new();
            encode_path(&mut bytes, &path);
            let mut expected = 1_u64.to_le_bytes().to_vec();
            expected.push(4);
            crate::codec_bytes::push_encoded(&mut expected, start.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut expected, end.to_le_bytes());
            assert_eq!(bytes, expected);
            let mut cursor = Cursor::new(&bytes);
            let decoded = decode_path(&mut cursor).unwrap();
            assert_eq!(decoded, path);
            assert_eq!(cursor.remaining(), 0);
            let mut reencoded = Vec::new();
            encode_path(&mut reencoded, &decoded);
            assert_eq!(reencoded, bytes);
            for truncated_length in 0..bytes.len() {
                assert!(decode_path(&mut Cursor::new(&bytes[..truncated_length])).is_err());
            }
        }
    }

    #[test]
    fn reference_type_and_referent_path_round_trip_without_erasing_custody() {
        for (access, descriptor_cleanup) in [
            (StructuralAccess::MutableBorrow, false),
            (StructuralAccess::SharedBorrow, true),
            (StructuralAccess::MutableBorrow, true),
            (StructuralAccess::WriteOnlyBorrow, true),
        ] {
            let declaration = StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup,
                scalar_restriction: None,
                id: StructuralTypeId::new(7).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(4),
                shape: StructuralTypeShape::Reference {
                    referent: StructuralTypeId::new(3).unwrap(),
                    access,
                },
            };
            let mut bytes = Vec::new();
            encode_type(&mut bytes, &declaration).unwrap();
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
            assert_eq!(cursor.remaining(), 0);
        }
        let path = vec![StructuralPathSegment::Referent];
        let mut bytes = Vec::new();
        encode_path(&mut bytes, &path);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_path(&mut cursor).unwrap(), path);
        assert_eq!(cursor.remaining(), 0);
    }

    #[test]
    fn block_parameter_place_round_trip_retains_exact_tag_block_and_position() {
        for (block, position) in [(1, 0), (2, 0), (1, 1), (u64::MAX, u32::MAX)] {
            let place = StructuralPlaceDeclaration {
                id: PlaceId::new(7).unwrap(),
                kind: StructuralPlaceKind::BlockParameter {
                    block: BlockId::new(block).unwrap(),
                    position,
                },
            };
            let mut encoded = Vec::new();
            encode_place(&mut encoded, place);
            let mut expected = 7_u64.to_le_bytes().to_vec();
            expected.push(8);
            crate::codec_bytes::push_encoded(&mut expected, block.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut expected, position.to_le_bytes());
            assert_eq!(encoded, expected);
            let mut cursor = Cursor::new(&encoded);
            assert_eq!(decode_place(&mut cursor).unwrap(), place);
            assert_eq!(cursor.remaining(), 0);
        }
    }

    #[test]
    fn block_parameter_place_rejects_zero_block_and_truncated_coordinates() {
        let mut encoded = Vec::new();
        encode_place(
            &mut encoded,
            StructuralPlaceDeclaration {
                id: PlaceId::new(7).unwrap(),
                kind: StructuralPlaceKind::BlockParameter {
                    block: BlockId::new(3).unwrap(),
                    position: 0,
                },
            },
        );
        for length in 0..encoded.len() {
            assert!(matches!(
                decode_place(&mut Cursor::new(&encoded[..length])),
                Err(FixedViewCopyDecodeError::Truncated)
            ));
        }
        crate::codec_bytes::patch_encoded(&mut encoded, 9, 0_u64.to_le_bytes());
        assert!(matches!(
            decode_place(&mut Cursor::new(&encoded)),
            Err(FixedViewCopyDecodeError::InvalidSemanticId(0))
        ));
    }

    #[test]
    fn byte_view_type_round_trip_retains_its_access() {
        for access in [
            None,
            Some(StructuralAccess::SharedBorrow),
            Some(StructuralAccess::MutableBorrow),
            Some(StructuralAccess::WriteOnlyBorrow),
        ] {
            let declaration = StructuralTypeDeclaration {
                requires: Vec::new(),
                quotient_carrier: None,
                owns_linear_claim: false,
                no_code_affine_cleanup: false,
                copyable: false,
                descriptor_cleanup: false,
                scalar_restriction: None,
                id: StructuralTypeId::new(7).unwrap(),
                identity: terminal_psi::TerminalIdentityIndex(5),
                shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                    access,
                }),
            };
            let mut bytes = Vec::new();
            encode_type(&mut bytes, &declaration).unwrap();
            let mut cursor = Cursor::new(&bytes);
            assert_eq!(decode_type(&mut cursor).unwrap(), declaration);
            assert_eq!(cursor.remaining(), 0);
        }
    }

    #[test]
    fn case_path_round_trip_retains_canonical_identity_and_rejects_zero() {
        let case = semantic_vocabulary::StructuralCaseId::new(41).unwrap();
        let path = vec![
            StructuralPathSegment::Case(case),
            StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(6)),
        ];
        let mut bytes = Vec::new();
        encode_path(&mut bytes, &path);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_path(&mut cursor).unwrap(), path);
        assert_eq!(cursor.remaining(), 0);
        let mut other = Vec::new();
        encode_path(
            &mut other,
            &[
                StructuralPathSegment::FixedIndex(case.get()),
                StructuralPathSegment::Field(terminal_psi::TerminalIdentityIndex(6)),
            ],
        );
        assert_ne!(bytes, other);
        let mut zero = Vec::new();
        encode_path(&mut zero, &[StructuralPathSegment::Case(case)]);
        let start = zero.len() - 8;
        zero[start..].fill(0);
        assert!(matches!(
            decode_path(&mut Cursor::new(&zero)),
            Err(FixedViewCopyDecodeError::InvalidSemanticId(0))
        ));
        bytes.pop();
        assert!(decode_path(&mut Cursor::new(&bytes)).is_err());
    }

    fn provider() -> ProviderCandidateConformance {
        let path = vec![
            StructuralPathSegment::Field(TerminalIdentityIndex(7)),
            StructuralPathSegment::Referent,
            StructuralPathSegment::AllElements,
        ];
        let domain = StructuralDomainId::new(3).unwrap();
        ProviderCandidateConformance {
            boundary: BoundaryMachineId::new(1).unwrap(),
            requirement_identity: TerminalIdentityIndex(1),
            source_requirement_identity: TerminalIdentityIndex(2),
            declaration_application: TerminalIdentityIndex(3),
            receiving_application: TerminalIdentityIndex(3),
            receiving_region_arguments: vec![TerminalIdentityIndex(4)],
            provider_identity: TerminalIdentityIndex(5),
            provider_declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(6)),
            candidate_identity: TerminalIdentityIndex(8),
            source_candidate_identity: TerminalIdentityIndex(9),
            source_candidate_package_identity: None,
            candidate: MachineId::new(2).unwrap(),
            signature: ProviderSignature {
                parameters: vec![ProviderSignatureParameter {
                    position: 0,
                    structural_type: StructuralTypeId::new(2).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: StructuralAccess::MutableBorrow,
                    qualifications: Vec::new(),
                    projected_qualifications: vec![StructuralPathQualification {
                        path: path.clone(),
                        domain,
                    }],
                }],
            },
            refinement: ProviderRefinement {
                positional_parameters: vec![ProviderParameterRefinement {
                    boundary_index: 0,
                    candidate_index: 0,
                }],
                required_domains: vec![StructuralDomainRequirement {
                    argument_index: 0,
                    path,
                    domain,
                }],
                realized_service_ceiling: Vec::new(),
            },
        }
    }

    #[test]
    fn provider_required_domain_round_trip_retains_quantified_formal_scope() {
        let expected = provider();
        let mut bytes = Vec::new();
        encode_provider(&mut bytes, &expected, true);
        let mut cursor = Cursor::new(&bytes);
        assert_eq!(decode_provider(&mut cursor, true).unwrap(), expected);
        assert_eq!(cursor.remaining(), 0);

        let mut other = expected;
        other.refinement.required_domains[0].path[2] = StructuralPathSegment::FixedIndex(0);
        let mut other_bytes = Vec::new();
        encode_provider(&mut other_bytes, &other, true);
        assert_ne!(other_bytes, bytes);
        assert_eq!(
            decode_provider(&mut Cursor::new(&other_bytes), true).unwrap(),
            other
        );
    }

    #[test]
    fn qualification_wildcard_cannot_decode_as_address_or_reference_origin() {
        let path = vec![StructuralPathSegment::AllElements];
        let mut bytes = Vec::new();
        encode_path(&mut bytes, &path);
        assert_eq!(
            decode_qualification_path(&mut Cursor::new(&bytes)).unwrap(),
            path
        );
        assert_eq!(
            decode_path(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownStructuralPathSegment(10)),
        );
        assert_eq!(
            decode_reference_origin_path(&mut Cursor::new(&bytes)),
            Err(FixedViewCopyDecodeError::UnknownStructuralPathSegment(10)),
        );

        let path = vec![StructuralPathSegment::FixedIndex(2)];
        let mut bytes = Vec::new();
        encode_path(&mut bytes, &path);
        assert_eq!(decode_path(&mut Cursor::new(&bytes)).unwrap(), path);
        assert_eq!(
            decode_reference_origin_path(&mut Cursor::new(&bytes)).unwrap(),
            vec![ReferenceOriginProjection::Exact(path[0].clone())],
        );
    }

    #[test]
    fn provider_required_domain_rejects_truncated_quantified_metadata() {
        let mut bytes = Vec::new();
        encode_provider(&mut bytes, &provider(), true);
        for length in 0..bytes.len() {
            assert!(
                decode_provider(&mut Cursor::new(&bytes[..length]), true).is_err(),
                "provider metadata prefix of {length} bytes must not be accepted"
            );
        }
    }
}
