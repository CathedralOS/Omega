//! Canonical structural field rows, including exact inclusive integer restrictions.

use semantic_vocabulary::{
    IeeeFloatFormat, IntegerSign, IntegerType, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, ByteSequencePredicateSet, StructuralFieldDeclaration,
    StructuralFieldType,
};

use crate::installation_record::codec::structural::scalar::{
    access_tag, decode_access, decode_identity, encode_identity,
};
use crate::installation_record::{InstallationError, Reader, decode_boolean, push_u16, push_u64};

pub(crate) fn encode_structural_field(
    bytes: &mut Vec<u8>,
    field: &StructuralFieldDeclaration,
) -> Result<(), InstallationError> {
    push_u64(bytes, field.id.get());
    encode_identity(bytes, &field.identity)?;
    bytes.push(u8::from(field.relevance.is_erased()));
    match &field.field_type {
        StructuralFieldType::Atomic(_) => {
            bytes.push(10);
            let scalar = field
                .field_type
                .atomic_scalar_type()
                .ok_or(InstallationError::InvalidStructuralTypeShape)?;
            crate::installation_record::codec::unit::scalar::encode_scalar_type(bytes, scalar)?;
        }
        StructuralFieldType::BoundedInteger(bounds) => {
            bytes.push(8);
            crate::installation_record::codec::unit::scalar::encode_integer_type(
                bytes,
                bounds.integer_type(),
            )?;
            crate::installation_record::codec::unit::scalar::encode_integer_value(
                bytes,
                bounds.minimum(),
            );
            crate::installation_record::codec::unit::scalar::encode_integer_value(
                bytes,
                bounds.maximum(),
            );
        }
        StructuralFieldType::Scalar(ScalarType::Boolean) => {
            bytes.push(1);
            bytes.extend_from_slice(&[0; 2]);
        }
        StructuralFieldType::Scalar(ScalarType::Integer(integer)) => {
            bytes.push(2);
            bytes.push(u8::from(integer.is_address()));
            bytes.push(u8::from(matches!(integer.sign(), IntegerSign::Signed)));
            push_u16(bytes, integer.bits());
        }
        StructuralFieldType::Scalar(ScalarType::IeeeFloat(format)) => {
            bytes.push(7);
            bytes.push(match format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            });
            bytes.push(0);
        }
        StructuralFieldType::IeeeFloat(format) => {
            bytes.push(5);
            bytes.push(match format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            });
            bytes.push(0);
        }
        StructuralFieldType::PredicateByteSequence {
            carrier,
            predicates,
        } => {
            if predicates.is_empty() {
                return Err(InstallationError::InvalidStructuralTypeShape);
            }
            bytes.push(9);
            let (tag, access, capacity) = match carrier {
                ByteSequenceCarrier::BorrowedView { access } => (1, *access, None),
                ByteSequenceCarrier::BoundedOwned { capacity } => (2, None, Some(*capacity)),
                ByteSequenceCarrier::BoundedView { access, capacity } => {
                    (3, *access, Some(*capacity))
                }
            };
            bytes.push(tag);
            bytes.push(access.map_or(0, access_tag));
            bytes.push(predicates.bits());
            if let Some(capacity) = capacity {
                push_u64(bytes, capacity);
            }
        }
        StructuralFieldType::ByteSequence(carrier) => {
            bytes.push(6);
            bytes.push(match carrier {
                ByteSequenceCarrier::BorrowedView { .. } => 1,
                ByteSequenceCarrier::BoundedOwned { .. } => 2,
                ByteSequenceCarrier::BoundedView { .. } => 3,
            });
            bytes.push(0);
            if let ByteSequenceCarrier::BoundedOwned { capacity }
            | ByteSequenceCarrier::BoundedView { capacity, .. } = carrier
            {
                push_u64(bytes, *capacity);
            }
        }
        StructuralFieldType::Structural(structural_type) => {
            bytes.push(3);
            bytes.extend_from_slice(&[0; 2]);
            push_u64(bytes, structural_type.get());
        }
        StructuralFieldType::Erased { type_identity } => {
            bytes.push(4);
            bytes.extend_from_slice(&[0; 2]);
            encode_identity(bytes, type_identity)?;
        }
    }
    Ok(())
}

pub(crate) fn decode_structural_field(
    reader: &mut Reader<'_>,
) -> Result<StructuralFieldDeclaration, InstallationError> {
    let id = StructuralFieldId::new(reader.u64()?).ok_or(
        InstallationError::ZeroStructuralReturnIdentity("structural field"),
    )?;
    let identity = decode_identity(reader)?;
    let relevance = match reader.u8()? {
        0 => BindingRelevance::Relevant,
        1 => BindingRelevance::Erased,
        value => return Err(InstallationError::InvalidBoolean(value)),
    };
    let field_type = match reader.u8()? {
        10 => {
            let field = StructuralFieldType::Atomic(
                crate::installation_record::codec::unit::scalar::decode_scalar_type(reader)?,
            );
            field
                .atomic_scalar_type()
                .ok_or(InstallationError::InvalidStructuralTypeShape)?;
            field
        }
        9 => {
            let carrier_tag = reader.u8()?;
            let access_tag = reader.u8()?;
            let access = if access_tag == 0 {
                None
            } else {
                Some(decode_access(access_tag)?)
            };
            let predicates = ByteSequencePredicateSet::from_bits(reader.u8()?)
                .filter(|predicates| !predicates.is_empty())
                .ok_or(InstallationError::InvalidStructuralTypeShape)?;
            let carrier = match carrier_tag {
                1 => ByteSequenceCarrier::BorrowedView { access },
                2 if access.is_none() => ByteSequenceCarrier::BoundedOwned {
                    capacity: reader.u64()?,
                },
                3 => ByteSequenceCarrier::BoundedView {
                    access,
                    capacity: reader.u64()?,
                },
                _ => return Err(InstallationError::InvalidStructuralTypeShape),
            };
            StructuralFieldType::PredicateByteSequence {
                carrier,
                predicates,
            }
        }

        8 => {
            let sign = match reader.u8()? {
                1 => IntegerSign::Signed,
                2 => IntegerSign::Unsigned,
                _ => return Err(InstallationError::InvalidStructuralTypeShape),
            };
            if reader.u8()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            let integer = IntegerType::new(sign, reader.u16()?)
                .map_err(|_| InstallationError::InvalidStructuralTypeShape)?;
            let minimum =
                crate::installation_record::codec::unit::scalar::decode_integer_value(reader)?;
            let maximum =
                crate::installation_record::codec::unit::scalar::decode_integer_value(reader)?;
            StructuralFieldType::BoundedInteger(
                semantic_vocabulary::BoundedIntegerType::new(integer, minimum, maximum)
                    .map_err(|_| InstallationError::InvalidStructuralTypeShape)?,
            )
        }
        1 => {
            if reader.u16()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            StructuralFieldType::Scalar(ScalarType::Boolean)
        }
        2 => {
            let is_address = decode_boolean(reader.u8()?)?;
            let signed = decode_boolean(reader.u8()?)?;
            let bits = reader.u16()?;
            let integer = if is_address {
                if signed {
                    return Err(InstallationError::InvalidStructuralTypeShape);
                }
                IntegerType::address(bits)
            } else {
                IntegerType::new(
                    if signed {
                        IntegerSign::Signed
                    } else {
                        IntegerSign::Unsigned
                    },
                    bits,
                )
            }
            .map_err(|_| InstallationError::InvalidStructuralTypeShape)?;
            StructuralFieldType::Scalar(ScalarType::Integer(integer))
        }
        3 => {
            if reader.u16()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            StructuralFieldType::Structural(StructuralTypeId::new(reader.u64()?).ok_or(
                InstallationError::ZeroStructuralReturnIdentity("nested structural type"),
            )?)
        }
        4 => {
            if reader.u16()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            StructuralFieldType::Erased {
                type_identity: decode_identity(reader)?,
            }
        }
        5 => {
            let format = match reader.u8()? {
                1 => IeeeFloatFormat::Binary32,
                2 => IeeeFloatFormat::Binary64,
                _ => return Err(InstallationError::InvalidStructuralTypeShape),
            };
            if reader.u8()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            StructuralFieldType::IeeeFloat(format)
        }
        6 => {
            let carrier_tag = reader.u8()?;
            if reader.u8()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            let carrier = match carrier_tag {
                1 => ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
                2 => ByteSequenceCarrier::BoundedOwned {
                    capacity: reader.u64()?,
                },
                // This wire keeps the field's capacity but not its bound
                // access — the same convention the borrowed view row uses.
                3 => ByteSequenceCarrier::BoundedView {
                    access: None,
                    capacity: reader.u64()?,
                },
                _ => return Err(InstallationError::InvalidStructuralTypeShape),
            };
            StructuralFieldType::ByteSequence(carrier)
        }
        7 => {
            let format = match reader.u8()? {
                1 => IeeeFloatFormat::Binary32,
                2 => IeeeFloatFormat::Binary64,
                _ => return Err(InstallationError::InvalidStructuralTypeShape),
            };
            if reader.u8()? != 0 {
                return Err(InstallationError::NonzeroReservedField);
            }
            StructuralFieldType::Scalar(ScalarType::IeeeFloat(format))
        }
        tag => {
            return Err(InstallationError::InvalidStructuralFieldTypeTag(tag));
        }
    };
    Ok(StructuralFieldDeclaration {
        id,
        identity,
        relevance,
        field_type,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        BindingRelevance, InstallationError, IntegerSign, IntegerType, Reader,
        StructuralFieldDeclaration, StructuralFieldId, StructuralFieldType,
        decode_structural_field, encode_structural_field,
    };
    use semantic_vocabulary::{BoundedIntegerType, IntegerValue};

    #[test]
    fn installed_predicate_fields_preserve_carrier_access_and_reject_unknown_bits() {
        use terminal_psi::{ByteSequenceCarrier, ByteSequencePredicateSet, StructuralAccess};
        for access in [
            None,
            Some(StructuralAccess::SharedBorrow),
            Some(StructuralAccess::MutableBorrow),
            Some(StructuralAccess::WriteOnlyBorrow),
        ] {
            for carrier in [
                ByteSequenceCarrier::BorrowedView { access },
                ByteSequenceCarrier::BoundedView {
                    access,
                    capacity: 23,
                },
                ByteSequenceCarrier::BoundedOwned { capacity: 23 },
            ] {
                for bits in 1..=15 {
                    let field = StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "text".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::PredicateByteSequence {
                            carrier,
                            predicates: ByteSequencePredicateSet::from_bits(bits).unwrap(),
                        },
                    };
                    let mut bytes = Vec::new();
                    encode_structural_field(&mut bytes, &field).unwrap();
                    let mut reader = Reader::new(&bytes);
                    assert_eq!(decode_structural_field(&mut reader).unwrap(), field);
                    assert_eq!(reader.remaining(), 0);
                    let predicate_offset = bytes.len()
                        - if matches!(carrier, ByteSequenceCarrier::BorrowedView { .. }) {
                            1
                        } else {
                            9
                        };
                    for malformed in [0, 16, 255] {
                        let mut changed = bytes.clone();
                        changed[predicate_offset] = malformed;
                        assert!(decode_structural_field(&mut Reader::new(&changed)).is_err());
                    }
                }
            }
        }
    }

    #[test]
    fn installed_bounded_fields_preserve_full_width_endpoints_and_reject_reversed_bounds() {
        let field = StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).unwrap(),
            identity: "payload".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
                    IntegerValue::Unsigned(1 << 127),
                    IntegerValue::Unsigned(u128::MAX - 1),
                )
                .unwrap(),
            ),
        };
        let mut bytes = Vec::new();
        encode_structural_field(&mut bytes, &field).unwrap();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_structural_field(&mut reader).unwrap(), field);
        assert_eq!(reader.remaining(), 0);
        let minimum_payload = bytes.len() - 36;
        bytes[minimum_payload..minimum_payload + 16].copy_from_slice(&u128::MAX.to_le_bytes());
        assert_eq!(
            decode_structural_field(&mut Reader::new(&bytes)),
            Err(InstallationError::InvalidStructuralTypeShape)
        );
    }
}
