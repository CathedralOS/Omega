//! Canonical structural-type declaration wire format.
//!
//! This module owns the exact record, fixed-array, and sum tags plus ordered
//! case payload envelopes. Field payloads remain delegated to the dedicated
//! structural-field codec.

use terminal_psi::TerminalIdentityIndex;
use terminal_psi::{
    StructuralCaseDeclaration, StructuralRecordStorage, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use super::CodecError;
use super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::structural_field_wire::{decode_structural_field, encode_structural_field};
use super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::decode_counted;

pub(crate) fn encode_structural_type(
    writer: &mut Writer,
    declaration: &StructuralTypeDeclaration,
) -> Result<(), CodecError> {
    writer.boolean(declaration.quotient_carrier.is_some());
    if let Some(carrier) = declaration.quotient_carrier {
        writer.id(carrier);
    }
    writer.id(declaration.id);
    writer.u32(declaration.identity.0);
    writer.boolean(declaration.no_code_affine_cleanup);
    writer.boolean(declaration.descriptor_cleanup);
    match &declaration.shape {
        StructuralTypeShape::BindingCarrier => writer.u8(11),
        StructuralTypeShape::Reference { referent, access } => {
            writer.u8(7);
            writer.id(*referent);
            super::structural_signature_wire::encode_structural_access(writer, *access);
        }
        StructuralTypeShape::PrimitiveScalar(scalar_type) => {
            writer.u8(if declaration.scalar_restriction.is_some() {
                12
            } else {
                6
            });
            encode_scalar_type(writer, *scalar_type);
            if let Some(restriction) = &declaration.scalar_restriction {
                super::structural_field_wire::encode_structural_field_type(writer, restriction)?;
            }
        }
        StructuralTypeShape::ByteSequence(carrier) => {
            writer.u8(4);
            super::structural_field_wire::encode_byte_sequence_carrier(writer, *carrier);
        }
        StructuralTypeShape::ElementView { element } => {
            writer.u8(8);
            writer.id(*element);
        }
        StructuralTypeShape::Record { fields, storage } => {
            writer.u8(match storage {
                StructuralRecordStorage::Packed => 1,
                StructuralRecordStorage::PlacedView(_) => 14,
                StructuralRecordStorage::PlacedAccessor(_) => 15,
                StructuralRecordStorage::Authored(_) => 10,
                StructuralRecordStorage::TargetLayout(_) => 13,
            });
            if let StructuralRecordStorage::TargetLayout(application) = storage {
                writer.u32(application.0);
            }
            if let StructuralRecordStorage::Authored(storage)
            | StructuralRecordStorage::PlacedView(storage)
            | StructuralRecordStorage::PlacedAccessor(storage) = storage
            {
                writer.u64(storage.byte_size);
                writer.u64(storage.byte_alignment);
            }
            writer.len("structural fields", fields.len())?;
            for field in fields {
                encode_structural_field(writer, field)?;
            }
            if matches!(
                storage,
                StructuralRecordStorage::Authored(_)
                    | StructuralRecordStorage::PlacedView(_)
                    | StructuralRecordStorage::PlacedAccessor(_)
            ) {
                for field in fields {
                    encode_authored_field_storage(writer, field.storage.as_ref())?;
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } => {
            writer.u8(2);
            writer.id(*element);
            writer.u64(*length);
        }
        StructuralTypeShape::Sum { cases } => {
            writer.u8(3);
            writer.len("structural cases", cases.len())?;
            for case in cases {
                writer.id(case.id);
                writer.u32(case.identity.0);
                writer.len("structural case payload fields", case.fields.len())?;
                for field in &case.fields {
                    encode_structural_field(writer, field)?;
                }
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            writer.u8(5);
            writer.len("mixed structural fields", fields.len())?;
            for field in fields {
                encode_structural_field(writer, field)?;
            }
            writer.len("mixed structural cases", cases.len())?;
            for case in cases {
                writer.id(case.id);
                writer.u32(case.identity.0);
                writer.len("mixed structural case payload fields", case.fields.len())?;
                for field in &case.fields {
                    encode_structural_field(writer, field)?;
                }
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_structural_type(
    reader: &mut Reader<'_>,
) -> Result<StructuralTypeDeclaration, CodecError> {
    let quotient_carrier = if reader.boolean()? {
        Some(reader.id("StructuralTypeId")?)
    } else {
        None
    };
    let id = reader.id("StructuralTypeId")?;
    let identity = TerminalIdentityIndex(reader.u32()?);
    let no_code_affine_cleanup = reader.boolean()?;
    let descriptor_cleanup = reader.boolean()?;
    let mut scalar_restriction = None;
    let shape = match reader.u8()? {
        11 => StructuralTypeShape::BindingCarrier,
        7 => StructuralTypeShape::Reference {
            referent: reader.id("StructuralTypeId")?,
            access: super::structural_signature_wire::decode_structural_access(reader)?,
        },
        6 => StructuralTypeShape::PrimitiveScalar(decode_scalar_type(reader)?),
        12 => {
            let carrier = decode_scalar_type(reader)?;
            scalar_restriction = Some(super::structural_field_wire::decode_structural_field_type(
                reader,
            )?);
            StructuralTypeShape::PrimitiveScalar(carrier)
        }
        tag @ (14 | 15) => {
            let geometry = semantic_vocabulary::AuthoredRecordStorage {
                byte_size: reader.u64()?,
                byte_alignment: reader.u64()?,
            };
            let mut fields = decode_counted(reader, decode_structural_field)?;
            for field in &mut fields {
                field.storage = decode_authored_field_storage(reader)?;
            }
            StructuralTypeShape::Record {
                fields,
                storage: if tag == 14 {
                    StructuralRecordStorage::PlacedView(geometry)
                } else {
                    StructuralRecordStorage::PlacedAccessor(geometry)
                },
            }
        }
        1 => StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: decode_counted(reader, decode_structural_field)?,
        },
        10 => {
            let storage = semantic_vocabulary::AuthoredRecordStorage {
                byte_size: reader.u64()?,
                byte_alignment: reader.u64()?,
            };
            let mut fields = decode_counted(reader, decode_structural_field)?;
            for field in &mut fields {
                field.storage = decode_authored_field_storage(reader)?;
            }
            StructuralTypeShape::Record {
                fields,
                storage: StructuralRecordStorage::Authored(storage),
            }
        }
        13 => StructuralTypeShape::Record {
            storage: StructuralRecordStorage::TargetLayout(terminal_psi::LayoutApplicationId(
                reader.u32()?,
            )),
            fields: decode_counted(reader, decode_structural_field)?,
        },
        8 => StructuralTypeShape::ElementView {
            element: reader.id("StructuralTypeId")?,
        },
        2 => StructuralTypeShape::FixedArray {
            element: reader.id("StructuralTypeId")?,
            length: reader.u64()?,
        },
        3 => StructuralTypeShape::Sum {
            cases: decode_counted(reader, |reader| {
                Ok(StructuralCaseDeclaration {
                    id: reader.id("StructuralCaseId")?,
                    identity: TerminalIdentityIndex(reader.u32()?),
                    fields: decode_counted(reader, decode_structural_field)?,
                })
            })?,
        },
        4 => StructuralTypeShape::ByteSequence(
            super::structural_field_wire::decode_byte_sequence_carrier(reader)?,
        ),
        5 => StructuralTypeShape::Mixed {
            fields: decode_counted(reader, decode_structural_field)?,
            cases: decode_counted(reader, |reader| {
                Ok(StructuralCaseDeclaration {
                    id: reader.id("StructuralCaseId")?,
                    identity: TerminalIdentityIndex(reader.u32()?),
                    fields: decode_counted(reader, decode_structural_field)?,
                })
            })?,
        },
        tag => return Err(CodecError::InvalidTag("StructuralTypeShape", tag)),
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

pub(crate) fn encode_authored_field_storage(
    writer: &mut Writer,
    storage: Option<&semantic_vocabulary::AuthoredFieldStorage>,
) -> Result<(), CodecError> {
    use semantic_vocabulary::AuthoredFieldStorage;
    match storage {
        None => writer.u8(0),
        Some(AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar,
        }) => {
            writer.u8(if stored_scalar.is_some() { 2 } else { 1 });
            writer.u64(*byte_offset);
            if let Some(scalar) = stored_scalar {
                encode_scalar_type(writer, *scalar);
            }
        }
        Some(AuthoredFieldStorage::Repeated {
            byte_offset,
            element_stride,
        }) => {
            writer.u8(3);
            writer.u64(*byte_offset);
            writer.u64(*element_stride);
        }
        Some(AuthoredFieldStorage::Direct { byte_offset }) => {
            writer.u8(4);
            writer.u64(*byte_offset);
        }
        Some(AuthoredFieldStorage::Fragmented(recipe)) => {
            writer.u8(6);
            writer.len("authored bit fragments", recipe.fragments().len())?;
            for fragment in recipe.fragments() {
                writer.u64(fragment.container_byte_offset);
                writer.u64(fragment.container_width_bits);
                writer.u64(fragment.destination_lsb);
                writer.u64(fragment.source_lsb);
                writer.u64(fragment.width);
            }
        }
    }
    Ok(())
}

pub(crate) fn decode_authored_field_storage(
    reader: &mut Reader<'_>,
) -> Result<Option<semantic_vocabulary::AuthoredFieldStorage>, CodecError> {
    use semantic_vocabulary::AuthoredFieldStorage;
    Ok(match reader.u8()? {
        0 => None,
        1 => Some(AuthoredFieldStorage::Scalar {
            byte_offset: reader.u64()?,
            stored_scalar: None,
        }),
        2 => Some(AuthoredFieldStorage::Scalar {
            byte_offset: reader.u64()?,
            stored_scalar: Some(decode_scalar_type(reader)?),
        }),
        3 => Some(AuthoredFieldStorage::Repeated {
            byte_offset: reader.u64()?,
            element_stride: reader.u64()?,
        }),
        4 => Some(AuthoredFieldStorage::Direct {
            byte_offset: reader.u64()?,
        }),
        6 => {
            let count = usize::try_from(reader.u32()?).map_err(|_| CodecError::UnexpectedEnd)?;
            if count == 0 || count > reader.remaining() / 40 {
                return Err(CodecError::MalformedStructuralFoundation(
                    "fragmented field requires a complete recipe",
                ));
            }
            let mut fragments = Vec::with_capacity(count);
            for _ in 0..count {
                fragments.push(semantic_vocabulary::AuthoredBitFragment {
                    container_byte_offset: reader.u64()?,
                    container_width_bits: reader.u64()?,
                    destination_lsb: reader.u64()?,
                    source_lsb: reader.u64()?,
                    width: reader.u64()?,
                });
            }
            Some(AuthoredFieldStorage::Fragmented(
                semantic_vocabulary::AuthoredBitFragments::new(fragments),
            ))
        }
        tag => return Err(CodecError::InvalidTag("AuthoredFieldStorage", tag)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{
        AuthoredFieldStorage, AuthoredRecordStorage, IntegerSign, IntegerType, ScalarType,
        StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{BindingRelevance, StructuralFieldDeclaration, StructuralFieldType};

    #[test]
    fn authored_record_geometry_round_trips_without_widening_its_scalar() {
        let stored =
            ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).expect("signed byte"));
        let declaration = StructuralTypeDeclaration {
            quotient_carrier: None,
            no_code_affine_cleanup: true,
            descriptor_cleanup: true,
            scalar_restriction: None,
            id: StructuralTypeId::new(1).expect("record identity"),
            identity: terminal_psi::TerminalIdentityIndex(15),
            shape: StructuralTypeShape::Record {
                storage: StructuralRecordStorage::Authored(AuthoredRecordStorage {
                    byte_size: 1,
                    byte_alignment: 1,
                }),
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).expect("field identity"),
                    identity: terminal_psi::TerminalIdentityIndex(16),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Signed, 64).expect("logical integer"),
                    )),
                    storage: Some(AuthoredFieldStorage::Scalar {
                        byte_offset: 0,
                        stored_scalar: Some(stored),
                    }),
                }],
            },
        };
        let mut writer = Writer::default();
        encode_structural_type(&mut writer, &declaration).expect("encode record");
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(
            decode_structural_type(&mut reader).expect("decode record"),
            declaration
        );
        assert_eq!(reader.remaining(), 0);
    }

    #[test]
    fn authored_field_wire_preserves_absence_stride_and_fragmentation() {
        for storage in [
            None,
            Some(semantic_vocabulary::AuthoredFieldStorage::Repeated {
                byte_offset: 3,
                element_stride: 5,
            }),
            Some(semantic_vocabulary::AuthoredFieldStorage::Direct { byte_offset: 7 }),
            Some(semantic_vocabulary::AuthoredFieldStorage::Fragmented(
                semantic_vocabulary::AuthoredBitFragments::new([
                    semantic_vocabulary::AuthoredBitFragment {
                        container_byte_offset: 7,
                        container_width_bits: 16,
                        destination_lsb: 4,
                        source_lsb: 0,
                        width: 3,
                    },
                    semantic_vocabulary::AuthoredBitFragment {
                        container_byte_offset: 9,
                        container_width_bits: 8,
                        destination_lsb: 1,
                        source_lsb: 3,
                        width: 2,
                    },
                ]),
            )),
        ] {
            let mut writer = Writer::default();
            encode_authored_field_storage(&mut writer, storage.as_ref())
                .expect("encode field geometry");
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(
                decode_authored_field_storage(&mut reader).expect("decode field geometry"),
                storage
            );
            assert_eq!(reader.remaining(), 0);
        }
    }
}
