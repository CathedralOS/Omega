//! Canonical format-32 structural field rows.

use psi_core::{
    IeeeFloatFormat, IntegerSign, IntegerType, ScalarType, StructuralFieldId, StructuralTypeId,
};
use psi_terminal::{
    BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
};

use super::{
    Reader, TerminalInstallationError, decode_boolean, push_u16, push_u64,
    structural_scalar_codec::{decode_identity, encode_identity},
};

pub(super) fn encode_structural_field(
    bytes: &mut Vec<u8>,
    field: &StructuralFieldDeclaration,
) -> Result<(), TerminalInstallationError> {
    push_u64(bytes, field.id.get());
    encode_identity(bytes, &field.identity)?;
    bytes.push(u8::from(field.relevance.is_erased()));
    match &field.field_type {
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
        StructuralFieldType::IeeeFloat(format) => {
            bytes.push(5);
            bytes.push(match format {
                IeeeFloatFormat::Binary32 => 1,
                IeeeFloatFormat::Binary64 => 2,
            });
            bytes.push(0);
        }
        StructuralFieldType::ByteSequence(carrier) => {
            bytes.push(6);
            bytes.push(match carrier {
                ByteSequenceCarrier::BorrowedView => 1,
                ByteSequenceCarrier::BoundedOwned { .. } => 2,
            });
            bytes.push(0);
            if let ByteSequenceCarrier::BoundedOwned { capacity } = carrier {
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

pub(super) fn decode_structural_field(
    reader: &mut Reader<'_>,
) -> Result<StructuralFieldDeclaration, TerminalInstallationError> {
    let id = StructuralFieldId::new(reader.u64()?).ok_or(
        TerminalInstallationError::ZeroStructuralReturnIdentity("structural field"),
    )?;
    let identity = decode_identity(reader)?;
    let relevance = match reader.u8()? {
        0 => BindingRelevance::Relevant,
        1 => BindingRelevance::Erased,
        value => return Err(TerminalInstallationError::InvalidBoolean(value)),
    };
    let field_type = match reader.u8()? {
        1 => {
            if reader.u16()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            StructuralFieldType::Scalar(ScalarType::Boolean)
        }
        2 => {
            let is_address = decode_boolean(reader.u8()?)?;
            let signed = decode_boolean(reader.u8()?)?;
            let bits = reader.u16()?;
            let integer = if is_address {
                if signed {
                    return Err(TerminalInstallationError::InvalidStructuralTypeShape);
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
            .map_err(|_| TerminalInstallationError::InvalidStructuralTypeShape)?;
            StructuralFieldType::Scalar(ScalarType::Integer(integer))
        }
        3 => {
            if reader.u16()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            StructuralFieldType::Structural(StructuralTypeId::new(reader.u64()?).ok_or(
                TerminalInstallationError::ZeroStructuralReturnIdentity("nested structural type"),
            )?)
        }
        4 => {
            if reader.u16()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            StructuralFieldType::Erased {
                type_identity: decode_identity(reader)?,
            }
        }
        5 => {
            let format = match reader.u8()? {
                1 => IeeeFloatFormat::Binary32,
                2 => IeeeFloatFormat::Binary64,
                _ => return Err(TerminalInstallationError::InvalidStructuralTypeShape),
            };
            if reader.u8()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            StructuralFieldType::IeeeFloat(format)
        }
        6 => {
            let carrier_tag = reader.u8()?;
            if reader.u8()? != 0 {
                return Err(TerminalInstallationError::NonzeroReservedField);
            }
            let carrier = match carrier_tag {
                1 => ByteSequenceCarrier::BorrowedView,
                2 => ByteSequenceCarrier::BoundedOwned {
                    capacity: reader.u64()?,
                },
                _ => return Err(TerminalInstallationError::InvalidStructuralTypeShape),
            };
            StructuralFieldType::ByteSequence(carrier)
        }
        tag => {
            return Err(TerminalInstallationError::InvalidStructuralFieldTypeTag(
                tag,
            ));
        }
    };
    Ok(StructuralFieldDeclaration {
        id,
        identity,
        relevance,
        field_type,
    })
}
